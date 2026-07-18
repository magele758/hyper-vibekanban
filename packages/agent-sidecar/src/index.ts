/**
 * Board Agent sidecar — Cursor SDK + OpenAI-compatible Pi/OpenCode adapters.
 *
 * Env:
 *   PORT                 default 13110
 *   VK_LOCAL_API_BASE    default http://127.0.0.1:13002
 *   VK_REMOTE_API_BASE   default http://127.0.0.1:13010
 *   VK_REMOTE_TOKEN      Bearer token for Remote (or pass Authorization header)
 */
import cors from "cors";
import express from "express";
import { z } from "zod";
import { Agent, AgentNotFoundError, Cursor } from "@cursor/sdk";
import {
  TOOL_SYSTEM_PROMPT,
  extractToolCall,
  executeTool,
  type ToolContext,
} from "./tools.js";
import {
  listCursorSdkModels,
  listOpenAiCompatibleModels,
  openaiCompatibleChat,
  resolveCursorSdkModelId,
  type ChatMessage,
} from "./runtimes.js";

const PORT = Number(process.env.PORT ?? 13110);
const LOCAL_API = process.env.VK_LOCAL_API_BASE ?? "http://127.0.0.1:13002";
const REMOTE_API = process.env.VK_REMOTE_API_BASE ?? "http://127.0.0.1:13000";
const MAX_TOOL_ROUNDS = 3;

const ChatBody = z.object({
  project_id: z.string().uuid(),
  session_id: z.string().uuid(),
  agent_id: z.string().uuid().optional().nullable(),
  message: z.string().min(1),
  cwd: z.string().optional(),
});

type LlmSecret = {
  agent_id: string;
  api_key: string | null;
  base_url: string | null;
  model_name: string | null;
  working_directory: string | null;
};

type CopilotSession = {
  id: string;
  project_id: string;
  agent_id: string | null;
  external_agent_id: string | null;
  title: string | null;
};

type BoardAgent = {
  id: string;
  chat_runtime?: "cursor" | "pi" | "opencode" | null;
  name?: string;
  instructions?: string;
};

type CopilotMessageRow = {
  role: string;
  content: string;
};

async function loadAgent(
  agentId: string,
  auth: string | undefined,
): Promise<BoardAgent> {
  return remoteFetch<BoardAgent>(`/v1/agents/${agentId}`, auth);
}

function authHeader(req: express.Request): string | undefined {
  const h = req.header("authorization") ?? req.header("Authorization");
  if (h) return h;
  if (process.env.VK_REMOTE_TOKEN) {
    return `Bearer ${process.env.VK_REMOTE_TOKEN}`;
  }
  return undefined;
}

async function remoteFetch<T>(
  path: string,
  auth: string | undefined,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`${REMOTE_API}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(auth ? { Authorization: auth } : {}),
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Remote ${path} → ${res.status}: ${body}`);
  }
  return (await res.json()) as T;
}

async function loadLlmSecret(
  agentId: string,
  auth: string | undefined,
): Promise<LlmSecret> {
  return remoteFetch<LlmSecret>(
    `/v1/agents/${agentId}/llm_settings/secret`,
    auth,
  );
}

async function loadSession(
  sessionId: string,
  auth: string | undefined,
): Promise<CopilotSession> {
  return remoteFetch<CopilotSession>(`/v1/copilot_sessions/${sessionId}`, auth);
}

async function loadRecentMessages(
  sessionId: string,
  auth: string | undefined,
): Promise<CopilotMessageRow[]> {
  const data = await remoteFetch<{ copilot_messages: CopilotMessageRow[] }>(
    `/v1/copilot_messages?session_id=${sessionId}`,
    auth,
  );
  return data.copilot_messages ?? [];
}

async function persistMessage(
  sessionId: string,
  role: string,
  content: string,
  auth: string | undefined,
) {
  await remoteFetch(`/v1/copilot_messages`, auth, {
    method: "POST",
    body: JSON.stringify({ session_id: sessionId, role, content }),
  });
}

async function bindExternalAgent(
  sessionId: string,
  externalAgentId: string,
  auth: string | undefined,
) {
  await remoteFetch(`/v1/copilot_sessions/${sessionId}`, auth, {
    method: "PATCH",
    body: JSON.stringify({ external_agent_id: externalAgentId }),
  });
}

function extractAssistantText(events: unknown[]): string {
  const parts: string[] = [];
  for (const ev of events) {
    if (!ev || typeof ev !== "object") continue;
    const e = ev as Record<string, unknown>;
    const type = String(e.type ?? "");
    if (
      (type === "assistant" || type === "assistant_message") &&
      typeof e.message === "object" &&
      e.message
    ) {
      const msg = e.message as {
        content?: Array<{ type?: string; text?: string }>;
      };
      for (const block of msg.content ?? []) {
        if ((block.type === "text" || !block.type) && block.text) {
          parts.push(block.text);
        }
      }
    }
    if (
      (type === "text_delta" || type === "text-delta") &&
      typeof e.delta === "string"
    ) {
      parts.push(e.delta);
    }
    if (type === "text-delta" && typeof e.text === "string") {
      parts.push(e.text);
    }
    if (typeof e.text === "string" && type === "message_update") {
      parts.push(e.text);
    }
  }
  return parts.join("");
}

const app = express();
app.use(
  cors({
    origin: true,
    credentials: false,
    preflightContinue: false,
  }),
);
app.use((req, res, next) => {
  if (req.headers["access-control-request-private-network"]) {
    res.setHeader("Access-Control-Allow-Private-Network", "true");
  }
  next();
});
app.use(express.json({ limit: "2mb" }));

function assistantTextFromEvent(event: Record<string, unknown>): string {
  const msg = event.message as
    | { content?: Array<{ type?: string; text?: string }> }
    | undefined;
  if (!msg?.content) return "";
  return msg.content
    .filter((b) => (b.type === "text" || !b.type) && b.text)
    .map((b) => b.text as string)
    .join("");
}

function buildSystemPrompt(agent?: BoardAgent | null): string {
  const role = agent?.instructions?.trim()
    ? `\n\nAgent role instructions:\n${agent.instructions.trim()}`
    : "";
  return `${TOOL_SYSTEM_PROMPT}${role}`;
}

function isCursorAgentNotFound(err: unknown): boolean {
  if (err instanceof AgentNotFoundError) return true;
  const code =
    err && typeof err === "object" && "code" in err
      ? String((err as { code?: unknown }).code ?? "")
      : "";
  if (code === "agent_not_found") return true;
  const msg = err instanceof Error ? err.message : String(err ?? "");
  return /agent[- ].*not found/i.test(msg) || /not found.*agent/i.test(msg);
}

/** Format DB transcript for a freshly created Cursor agent (no SDK checkpoint). */
function buildCursorRecreatePrompt(
  systemPrompt: string,
  history: CopilotMessageRow[],
): string {
  const recent = history
    .filter((m) => m.role === "user" || m.role === "assistant")
    .slice(-20);
  const transcript = recent
    .map((m) => {
      const label = m.role === "user" ? "User" : "Assistant";
      return `${label}:\n${m.content}`;
    })
    .join("\n\n");
  return `${systemPrompt}\n\n---\nConversation so far (continue this multi-turn chat):\n${transcript}`;
}

async function runToolLoop(params: {
  projectId: string;
  auth: string;
  send: (payload: unknown) => void;
  initialReply: string;
  continueChat: (followUp: string) => Promise<string>;
}): Promise<string> {
  let reply = params.initialReply;
  for (let round = 0; round < MAX_TOOL_ROUNDS; round++) {
    const call = extractToolCall(reply);
    if (!call) break;
    const toolCtx: ToolContext = {
      projectId: params.projectId,
      auth: params.auth,
      remoteApi: REMOTE_API,
      send: params.send,
    };
    const result = await executeTool(call.tool, call.args, toolCtx);
    const followUp = `Tool \`${call.tool}\` result:\n${JSON.stringify(result, null, 2)}\n\nContinue helping the user. If you need another tool, emit another vk-tool block; otherwise reply normally.`;
    reply = await params.continueChat(followUp);
  }
  return reply;
}

app.get("/health", (_req, res) => {
  res.json({
    ok: true,
    service: "agent-sidecar",
    runtimes: ["cursor", "pi", "opencode"],
    local_api: LOCAL_API,
    remote_api: REMOTE_API,
    default_cwd: process.cwd(),
  });
});

app.get("/cwd", (_req, res) => {
  res.json({ default_cwd: process.cwd() });
});

/**
 * Stream a chat turn.
 * SSE events: {type:'delta', text}, {type:'tool_start'|'tool_result'}, {type:'done'}, {type:'error'}
 */
app.post("/copilot/chat", async (req, res) => {
  const parsed = ChatBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.flatten() });
    return;
  }

  const auth = authHeader(req);
  if (!auth) {
    res.status(401).json({
      error: "missing_auth",
      message: "Pass Authorization: Bearer <token> or set VK_REMOTE_TOKEN",
    });
    return;
  }

  const { project_id, session_id, agent_id, message, cwd } = parsed.data;
  console.log(
    `[agent-sidecar] chat session=${session_id} agent=${agent_id ?? "-"} msg_len=${message.length}`,
  );

  res.setHeader("Content-Type", "text/event-stream");
  res.setHeader("Cache-Control", "no-cache");
  res.setHeader("Connection", "keep-alive");
  res.setHeader("X-Accel-Buffering", "no");
  res.flushHeaders?.();

  const send = (payload: unknown) => {
    res.write(`data: ${JSON.stringify(payload)}\n\n`);
  };

  try {
    const session = await loadSession(session_id, auth);
    if (session.project_id !== project_id) {
      throw new Error("session project mismatch");
    }

    const effectiveAgentId = agent_id ?? session.agent_id;
    let boardAgent: BoardAgent | null = null;
    let runtime: "cursor" | "pi" | "opencode" = "cursor";
    if (effectiveAgentId) {
      boardAgent = await loadAgent(effectiveAgentId, auth);
      runtime = boardAgent.chat_runtime ?? "cursor";
    }

    let apiKey = process.env.CURSOR_API_KEY ?? "";
    let modelId = runtime === "cursor" ? "composer-2.5" : "gpt-4.1-mini";
    let baseUrl: string | null = null;
    let savedWorkingDirectory: string | null = null;

    if (effectiveAgentId) {
      const llm = await loadLlmSecret(effectiveAgentId, auth);
      if (llm.api_key) apiKey = llm.api_key;
      if (llm.model_name) modelId = llm.model_name;
      if (llm.base_url) baseUrl = llm.base_url;
      if (llm.working_directory) savedWorkingDirectory = llm.working_directory;
    } else {
      // 全局指挥台（agent_id: null）用环境变量配模型服务。
      // 一旦设了 VK_COPILOT_BASE_URL，下方 useOpenAiCompatible 自动为 true，
      // 走 OpenAI 兼容的 /chat/completions，支持任意兼容网关。
      if (process.env.VK_COPILOT_API_KEY)
        apiKey = process.env.VK_COPILOT_API_KEY;
      if (process.env.VK_COPILOT_BASE_URL)
        baseUrl = process.env.VK_COPILOT_BASE_URL;
      if (process.env.VK_COPILOT_MODEL) modelId = process.env.VK_COPILOT_MODEL;
    }

    const requestCwd = cwd?.trim() || "";
    const savedCwd = savedWorkingDirectory?.trim() || "";
    const effectiveCwd = requestCwd || savedCwd || process.cwd();
    const cwdSource: "request" | "saved" | "default" = requestCwd
      ? "request"
      : savedCwd
        ? "saved"
        : "default";

    if (!apiKey) {
      throw new Error(
        runtime === "cursor"
          ? "No Cursor API key. Set agent LLM settings (api_key) or CURSOR_API_KEY."
          : `No API key for runtime "${runtime}". Set agent LLM settings api_key (+ base_url).`,
      );
    }

    // Cursor + custom base_url is treated as OpenAI-compatible (same as /models).
    // Official Cursor User API Key path only when base_url is empty.
    const useOpenAiCompatible =
      runtime === "pi" || runtime === "opencode" || !!baseUrl?.trim();

    if ((runtime === "pi" || runtime === "opencode") && !baseUrl) {
      throw new Error(
        `Runtime "${runtime}" requires agent LLM base_url (OpenAI-compatible endpoint).`,
      );
    }

    await persistMessage(session_id, "user", message, auth);
    send({
      type: "status",
      message: "running",
      runtime,
      cwd: effectiveCwd,
      cwd_source: cwdSource,
      transport: useOpenAiCompatible ? "openai-compatible" : "cursor-sdk",
    });

    const systemPrompt = buildSystemPrompt(boardAgent);
    let finalReply = "";
    let externalAgentId = session.external_agent_id;

    if (useOpenAiCompatible) {
      if (!baseUrl?.trim()) {
        throw new Error(
          "OpenAI-compatible chat requires agent LLM base_url.",
        );
      }
      const history = await loadRecentMessages(session_id, auth);
      const historyTurns = history
        .filter((m) => m.role === "user" || m.role === "assistant")
        .slice(-20);
      console.log(
        `[agent-sidecar] multi-turn openai-compatible session=${session_id} history=${historyTurns.length} model=${modelId}`,
      );
      send({
        type: "status",
        message: "running",
        runtime,
        transport: "openai-compatible",
        history_turns: historyTurns.length,
        cwd: effectiveCwd,
        cwd_source: cwdSource,
      });
      const messages: ChatMessage[] = [
        { role: "system", content: systemPrompt },
        ...historyTurns.map((m) => ({
          role: m.role as "user" | "assistant",
          content: m.content,
        })),
      ];
      // history already includes the just-persisted user message

      const runOnce = async (msgs: ChatMessage[]) =>
        openaiCompatibleChat({
          apiKey,
          baseUrl: baseUrl!,
          model: modelId,
          messages: msgs,
          onDelta: (text) => send({ type: "delta", text }),
        });

      let reply = await runOnce(messages);
      reply = await runToolLoop({
        projectId: project_id,
        auth,
        send,
        initialReply: reply,
        continueChat: async (followUp) => {
          messages.push({ role: "assistant", content: reply });
          messages.push({ role: "user", content: followUp });
          const next = await runOnce(messages);
          reply = next;
          return next;
        },
      });
      finalReply = reply.trim() || "(Agent finished with no text output.)";
    } else {
      // Cursor SDK path — model ids must come from Cursor.models.list(), not CLI.
      let cursorModelId = modelId;
      if (!baseUrl) {
        try {
          const catalog = await Cursor.models.list({ apiKey });
          cursorModelId = resolveCursorSdkModelId(modelId, catalog);
          if (cursorModelId !== modelId) {
            console.warn(
              `[agent-sidecar] normalized Cursor model "${modelId}" → "${cursorModelId}"`,
            );
          }
        } catch (err) {
          console.warn(
            "[agent-sidecar] Cursor.models.list failed; using saved model id:",
            err instanceof Error ? err.message : err,
          );
        }
      }

      const prevBackend = process.env.CURSOR_BACKEND_URL;
      if (baseUrl) {
        process.env.CURSOR_BACKEND_URL = baseUrl;
      }

      const createCursorAgent = async () =>
        Agent.create({
          apiKey,
          model: { id: cursorModelId },
          local: { cwd: effectiveCwd },
          name: effectiveAgentId
            ? `vk-agent-${effectiveAgentId.slice(0, 8)}`
            : `vk-copilot-${project_id.slice(0, 8)}`,
        });

      // Cursor local agents live in an on-disk store keyed by cwd. Resume can
      // fail after store wipe, cwd change, or expired agent id — recreate and
      // re-prime from DB history so multi-turn still works.
      let agent;
      let needsHistoryPrime = !session.external_agent_id;
      if (session.external_agent_id) {
        try {
          agent = await Agent.resume(session.external_agent_id, {
            apiKey,
            model: { id: cursorModelId },
            local: { cwd: effectiveCwd },
          });
        } catch (err) {
          if (!isCursorAgentNotFound(err)) throw err;
          console.warn(
            `[agent-sidecar] Agent.resume(${session.external_agent_id}) failed (${
              err instanceof Error ? err.message : String(err)
            }); recreating agent with DB history`,
          );
          agent = await createCursorAgent();
          needsHistoryPrime = true;
          if (agent.agentId) {
            await bindExternalAgent(session_id, agent.agentId, auth);
            externalAgentId = agent.agentId;
          }
        }
      } else {
        agent = await createCursorAgent();
        if (agent.agentId) {
          await bindExternalAgent(session_id, agent.agentId, auth);
          externalAgentId = agent.agentId;
        }
      }

      const sendCursor = async (prompt: string) => {
        const run = await agent.send(prompt);
        const collected: unknown[] = [];
        for await (const event of run.stream()) {
          collected.push(event);
          const e = event as unknown as Record<string, unknown>;
          const type = String(e.type ?? "");
          if (
            (type === "text_delta" || type === "text-delta") &&
            typeof e.delta === "string"
          ) {
            send({ type: "delta", text: e.delta });
          } else if (type === "text-delta" && typeof e.text === "string") {
            send({ type: "delta", text: e.text });
          } else if (type === "assistant" || type === "assistant_message") {
            const text = assistantTextFromEvent(e);
            if (text) send({ type: "delta", text });
            send({ type: "event", event });
          } else if (
            type === "message_update" ||
            type === "tool_call" ||
            type === "tool-call"
          ) {
            send({ type: "event", event });
          }
        }
        const result = await run.wait();
        const resultText = (result as { result?: string })?.result;
        return (
          (resultText && resultText.trim()
            ? resultText
            : extractAssistantText(collected)) || ""
        );
      };

      let primed: string;
      if (!needsHistoryPrime) {
        // Successful resume — SDK checkpoint already has prior turns.
        primed = message;
      } else if (session.external_agent_id) {
        // Recreated after stale external_agent_id — inject recent transcript.
        const history = await loadRecentMessages(session_id, auth);
        primed = buildCursorRecreatePrompt(systemPrompt, history);
      } else {
        // Brand-new Cursor agent.
        primed = `${systemPrompt}\n\n---\nUser:\n${message}`;
      }
      let reply = await sendCursor(primed);
      reply = await runToolLoop({
        projectId: project_id,
        auth,
        send,
        initialReply: reply,
        continueChat: sendCursor,
      });
      finalReply = reply.trim() || "(Agent finished with no text output.)";
      if (agent.agentId && agent.agentId !== externalAgentId) {
        await bindExternalAgent(session_id, agent.agentId, auth);
        externalAgentId = agent.agentId;
      } else {
        externalAgentId = agent.agentId ?? externalAgentId;
      }

      if (baseUrl) {
        if (prevBackend === undefined) delete process.env.CURSOR_BACKEND_URL;
        else process.env.CURSOR_BACKEND_URL = prevBackend;
      }
    }

    // Persist before `done` so the next turn's history load always includes
    // this assistant reply (UI unblocks only after done).
    await persistMessage(session_id, "assistant", finalReply, auth);
    send({
      type: "done",
      reply: finalReply,
      external_agent_id: externalAgentId,
      runtime,
      cwd: effectiveCwd,
      cwd_source: cwdSource,
    });
  } catch (err) {
    const messageText = err instanceof Error ? err.message : String(err);
    console.error("[agent-sidecar] chat error:", messageText);
    send({ type: "error", message: messageText });
  } finally {
    res.end();
  }
});

const ListModelsBody = z.object({
  api_key: z.string().optional().nullable(),
  base_url: z.string().optional().nullable(),
  agent_id: z.string().uuid().optional().nullable(),
});

/**
 * List models for board chat.
 * - With base_url: OpenAI-compatible `GET {base}/models` (Pi / OpenCode / custom Cursor gateway).
 * - Without base_url: Cursor SDK `Cursor.models.list()` (official Cursor; not CLI --list-models).
 */
app.post("/models", async (req, res) => {
  try {
    const body = ListModelsBody.parse(req.body ?? {});
    const auth = authHeader(req);

    let apiKey = body.api_key?.trim() || "";
    // null/"" from client means "no base_url" (Cursor SDK listing).
    // undefined means omitted — may inherit from agent secrets.
    let baseUrl = typeof body.base_url === "string" ? body.base_url.trim() : "";

    if (body.agent_id) {
      const llm = await loadLlmSecret(body.agent_id, auth);
      if (!apiKey && llm.api_key) apiKey = llm.api_key;
      if (body.base_url === undefined && llm.base_url) {
        baseUrl = llm.base_url;
      }
    }

    if (!apiKey) {
      apiKey = process.env.CURSOR_API_KEY?.trim() || "";
    }

    if (!apiKey) {
      res.status(400).json({
        error: "api_key required (or agent_id with saved key)",
      });
      return;
    }

    const models = baseUrl
      ? await listOpenAiCompatibleModels({ apiKey, baseUrl })
      : await listCursorSdkModels({
          apiKey,
          list: (opts) => Cursor.models.list(opts),
        });
    res.json({
      models,
      source: baseUrl ? "openai-compatible" : "cursor-sdk",
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error("[agent-sidecar] /models error:", message);
    res.status(500).json({ error: message });
  }
});

app.listen(PORT, () => {
  console.log(
    `[agent-sidecar] board chat on http://127.0.0.1:${PORT} (cursor/pi/opencode)`,
  );
});
