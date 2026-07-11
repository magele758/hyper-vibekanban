/**
 * Board Agent sidecar — Cursor SDK + OpenAI-compatible Pi/OpenCode adapters.
 *
 * Env:
 *   PORT                 default 13110
 *   VK_LOCAL_API_BASE    default http://127.0.0.1:13002
 *   VK_REMOTE_API_BASE   default http://127.0.0.1:13010
 *   VK_REMOTE_TOKEN      Bearer token for Remote (or pass Authorization header)
 */
import cors from 'cors';
import express from 'express';
import { z } from 'zod';
import { Agent } from '@cursor/sdk';
import {
  TOOL_SYSTEM_PROMPT,
  extractToolCall,
  executeTool,
  type ToolContext,
} from './tools.js';
import { openaiCompatibleChat, type ChatMessage } from './runtimes.js';

const PORT = Number(process.env.PORT ?? 13110);
const LOCAL_API = process.env.VK_LOCAL_API_BASE ?? 'http://127.0.0.1:13002';
const REMOTE_API = process.env.VK_REMOTE_API_BASE ?? 'http://127.0.0.1:13010';
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
  chat_runtime?: 'cursor' | 'pi' | 'opencode' | null;
  name?: string;
  instructions?: string;
};

type CopilotMessageRow = {
  role: string;
  content: string;
};

async function loadAgent(
  agentId: string,
  auth: string | undefined
): Promise<BoardAgent> {
  return remoteFetch<BoardAgent>(`/v1/agents/${agentId}`, auth);
}

function authHeader(req: express.Request): string | undefined {
  const h = req.header('authorization') ?? req.header('Authorization');
  if (h) return h;
  if (process.env.VK_REMOTE_TOKEN) {
    return `Bearer ${process.env.VK_REMOTE_TOKEN}`;
  }
  return undefined;
}

async function remoteFetch<T>(
  path: string,
  auth: string | undefined,
  init?: RequestInit
): Promise<T> {
  const res = await fetch(`${REMOTE_API}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
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
  auth: string | undefined
): Promise<LlmSecret> {
  return remoteFetch<LlmSecret>(
    `/v1/agents/${agentId}/llm_settings/secret`,
    auth
  );
}

async function loadSession(
  sessionId: string,
  auth: string | undefined
): Promise<CopilotSession> {
  return remoteFetch<CopilotSession>(`/v1/copilot_sessions/${sessionId}`, auth);
}

async function loadRecentMessages(
  sessionId: string,
  auth: string | undefined
): Promise<CopilotMessageRow[]> {
  const data = await remoteFetch<{ copilot_messages: CopilotMessageRow[] }>(
    `/v1/copilot_messages?session_id=${sessionId}`,
    auth
  );
  return data.copilot_messages ?? [];
}

async function persistMessage(
  sessionId: string,
  role: string,
  content: string,
  auth: string | undefined
) {
  await remoteFetch(`/v1/copilot_messages`, auth, {
    method: 'POST',
    body: JSON.stringify({ session_id: sessionId, role, content }),
  });
}

async function bindExternalAgent(
  sessionId: string,
  externalAgentId: string,
  auth: string | undefined
) {
  await remoteFetch(`/v1/copilot_sessions/${sessionId}`, auth, {
    method: 'PATCH',
    body: JSON.stringify({ external_agent_id: externalAgentId }),
  });
}

function extractAssistantText(events: unknown[]): string {
  const parts: string[] = [];
  for (const ev of events) {
    if (!ev || typeof ev !== 'object') continue;
    const e = ev as Record<string, unknown>;
    const type = String(e.type ?? '');
    if (
      (type === 'assistant' || type === 'assistant_message') &&
      typeof e.message === 'object' &&
      e.message
    ) {
      const msg = e.message as {
        content?: Array<{ type?: string; text?: string }>;
      };
      for (const block of msg.content ?? []) {
        if ((block.type === 'text' || !block.type) && block.text) {
          parts.push(block.text);
        }
      }
    }
    if (
      (type === 'text_delta' || type === 'text-delta') &&
      typeof e.delta === 'string'
    ) {
      parts.push(e.delta);
    }
    if (type === 'text-delta' && typeof e.text === 'string') {
      parts.push(e.text);
    }
    if (typeof e.text === 'string' && type === 'message_update') {
      parts.push(e.text);
    }
  }
  return parts.join('');
}

const app = express();
app.use(
  cors({
    origin: true,
    credentials: false,
    preflightContinue: false,
  })
);
app.use((req, res, next) => {
  if (req.headers['access-control-request-private-network']) {
    res.setHeader('Access-Control-Allow-Private-Network', 'true');
  }
  next();
});
app.use(express.json({ limit: '2mb' }));

function assistantTextFromEvent(event: Record<string, unknown>): string {
  const msg = event.message as
    | { content?: Array<{ type?: string; text?: string }> }
    | undefined;
  if (!msg?.content) return '';
  return msg.content
    .filter((b) => (b.type === 'text' || !b.type) && b.text)
    .map((b) => b.text as string)
    .join('');
}

function buildSystemPrompt(agent?: BoardAgent | null): string {
  const role = agent?.instructions?.trim()
    ? `\n\nAgent role instructions:\n${agent.instructions.trim()}`
    : '';
  return `${TOOL_SYSTEM_PROMPT}${role}`;
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

app.get('/health', (_req, res) => {
  res.json({
    ok: true,
    service: 'agent-sidecar',
    runtimes: ['cursor', 'pi', 'opencode'],
    local_api: LOCAL_API,
    remote_api: REMOTE_API,
  });
});

/**
 * Stream a chat turn.
 * SSE events: {type:'delta', text}, {type:'tool_start'|'tool_result'}, {type:'done'}, {type:'error'}
 */
app.post('/copilot/chat', async (req, res) => {
  const parsed = ChatBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.flatten() });
    return;
  }

  const auth = authHeader(req);
  if (!auth) {
    res.status(401).json({
      error: 'missing_auth',
      message: 'Pass Authorization: Bearer <token> or set VK_REMOTE_TOKEN',
    });
    return;
  }

  const { project_id, session_id, agent_id, message, cwd } = parsed.data;
  console.log(
    `[agent-sidecar] chat session=${session_id} agent=${agent_id ?? '-'} msg_len=${message.length}`
  );

  res.setHeader('Content-Type', 'text/event-stream');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  res.setHeader('X-Accel-Buffering', 'no');
  res.flushHeaders?.();

  const send = (payload: unknown) => {
    res.write(`data: ${JSON.stringify(payload)}\n\n`);
  };

  try {
    const session = await loadSession(session_id, auth);
    if (session.project_id !== project_id) {
      throw new Error('session project mismatch');
    }

    const effectiveAgentId = agent_id ?? session.agent_id;
    let boardAgent: BoardAgent | null = null;
    let runtime: 'cursor' | 'pi' | 'opencode' = 'cursor';
    if (effectiveAgentId) {
      boardAgent = await loadAgent(effectiveAgentId, auth);
      runtime = boardAgent.chat_runtime ?? 'cursor';
    }

    let apiKey = process.env.CURSOR_API_KEY ?? '';
    let modelId =
      runtime === 'cursor' ? 'composer-2.5' : 'gpt-4.1-mini';
    let baseUrl: string | null = null;

    if (effectiveAgentId) {
      const llm = await loadLlmSecret(effectiveAgentId, auth);
      if (llm.api_key) apiKey = llm.api_key;
      if (llm.model_name) modelId = llm.model_name;
      if (llm.base_url) baseUrl = llm.base_url;
    }

    if (!apiKey) {
      throw new Error(
        runtime === 'cursor'
          ? 'No Cursor API key. Set agent LLM settings (api_key) or CURSOR_API_KEY.'
          : `No API key for runtime "${runtime}". Set agent LLM settings api_key (+ base_url).`
      );
    }

    if ((runtime === 'pi' || runtime === 'opencode') && !baseUrl) {
      throw new Error(
        `Runtime "${runtime}" requires agent LLM base_url (OpenAI-compatible endpoint).`
      );
    }

    await persistMessage(session_id, 'user', message, auth);
    send({ type: 'status', message: 'running', runtime });

    const systemPrompt = buildSystemPrompt(boardAgent);
    let finalReply = '';
    let externalAgentId = session.external_agent_id;

    if (runtime === 'pi' || runtime === 'opencode') {
      const history = await loadRecentMessages(session_id, auth);
      const messages: ChatMessage[] = [
        { role: 'system', content: systemPrompt },
        ...history
          .filter((m) => m.role === 'user' || m.role === 'assistant')
          .slice(-20)
          .map((m) => ({
            role: m.role as 'user' | 'assistant',
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
          onDelta: (text) => send({ type: 'delta', text }),
        });

      let reply = await runOnce(messages);
      reply = await runToolLoop({
        projectId: project_id,
        auth,
        send,
        initialReply: reply,
        continueChat: async (followUp) => {
          messages.push({ role: 'assistant', content: reply });
          messages.push({ role: 'user', content: followUp });
          const next = await runOnce(messages);
          reply = next;
          return next;
        },
      });
      finalReply = reply.trim() || '(Agent finished with no text output.)';
    } else {
      // Cursor SDK path
      const prevBackend = process.env.CURSOR_BACKEND_URL;
      if (baseUrl) {
        process.env.CURSOR_BACKEND_URL = baseUrl;
      }

      let agent;
      if (session.external_agent_id) {
        agent = await Agent.resume(session.external_agent_id, {
          apiKey,
          model: { id: modelId },
          local: { cwd: cwd ?? process.cwd() },
        });
      } else {
        agent = await Agent.create({
          apiKey,
          model: { id: modelId },
          local: { cwd: cwd ?? process.cwd() },
          name: effectiveAgentId
            ? `vk-agent-${effectiveAgentId.slice(0, 8)}`
            : `vk-copilot-${project_id.slice(0, 8)}`,
        });
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
          const type = String(e.type ?? '');
          if (
            (type === 'text_delta' || type === 'text-delta') &&
            typeof e.delta === 'string'
          ) {
            send({ type: 'delta', text: e.delta });
          } else if (type === 'text-delta' && typeof e.text === 'string') {
            send({ type: 'delta', text: e.text });
          } else if (type === 'assistant' || type === 'assistant_message') {
            const text = assistantTextFromEvent(e);
            if (text) send({ type: 'delta', text });
            send({ type: 'event', event });
          } else if (
            type === 'message_update' ||
            type === 'tool_call' ||
            type === 'tool-call'
          ) {
            send({ type: 'event', event });
          }
        }
        const result = await run.wait();
        const resultText = (result as { result?: string })?.result;
        return (
          (resultText && resultText.trim()
            ? resultText
            : extractAssistantText(collected)) || ''
        );
      };

      // First turn includes tool system prompt as prefix for new sessions
      const primed = session.external_agent_id
        ? message
        : `${systemPrompt}\n\n---\nUser:\n${message}`;
      let reply = await sendCursor(primed);
      reply = await runToolLoop({
        projectId: project_id,
        auth,
        send,
        initialReply: reply,
        continueChat: sendCursor,
      });
      finalReply = reply.trim() || '(Agent finished with no text output.)';
      externalAgentId = agent.agentId ?? session.external_agent_id;

      if (baseUrl) {
        if (prevBackend === undefined) delete process.env.CURSOR_BACKEND_URL;
        else process.env.CURSOR_BACKEND_URL = prevBackend;
      }
    }

    await persistMessage(session_id, 'assistant', finalReply, auth);
    send({
      type: 'done',
      reply: finalReply,
      external_agent_id: externalAgentId,
      runtime,
    });
  } catch (err) {
    const messageText = err instanceof Error ? err.message : String(err);
    console.error('[agent-sidecar] chat error:', messageText);
    send({ type: 'error', message: messageText });
  } finally {
    res.end();
  }
});

app.listen(PORT, () => {
  console.log(
    `[agent-sidecar] board chat on http://127.0.0.1:${PORT} (cursor/pi/opencode)`
  );
});
