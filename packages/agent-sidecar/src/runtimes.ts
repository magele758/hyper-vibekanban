/**
 * Alternate board chat runtimes (Pi / OpenCode) via OpenAI-compatible chat completions.
 * Coding executors remain separate — these are dialogue/orchestration only.
 */

export type ChatMessage = {
  role: "system" | "user" | "assistant";
  content: string;
};

export type RuntimeChatParams = {
  apiKey: string;
  baseUrl: string;
  model: string;
  messages: ChatMessage[];
  onDelta?: (text: string) => void;
};

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/$/, "");
}

/**
 * Stream (or fall back to non-stream) chat completions against an OpenAI-compatible endpoint.
 * Used for both `pi` and `opencode` board chat runtimes in MVP.
 */
export async function openaiCompatibleChat(
  params: RuntimeChatParams,
): Promise<string> {
  const base = normalizeBaseUrl(params.baseUrl);
  const url = `${base}/chat/completions`;
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${params.apiKey}`,
    },
    body: JSON.stringify({
      model: params.model,
      messages: params.messages,
      stream: true,
    }),
  });

  if (!res.ok) {
    // Retry without stream for gateways that reject streaming.
    const fallback = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${params.apiKey}`,
      },
      body: JSON.stringify({
        model: params.model,
        messages: params.messages,
        stream: false,
      }),
    });
    if (!fallback.ok) {
      throw new Error(
        `OpenAI-compatible chat failed: ${res.status} / ${fallback.status} ${await fallback.text()}`,
      );
    }
    const json = (await fallback.json()) as {
      choices?: Array<{ message?: { content?: string } }>;
    };
    const text = json.choices?.[0]?.message?.content ?? "";
    if (text) params.onDelta?.(text);
    return text;
  }

  if (!res.body) {
    throw new Error("OpenAI-compatible chat returned empty body");
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let reply = "";
  let streamFinished = false;

  try {
    while (!streamFinished) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed.startsWith("data:")) continue;
        const data = trimmed.slice(5).trim();
        // Some gateways keep the TCP connection open after the last token.
        // Stop as soon as we see the terminal marker / finish_reason.
        if (data === "[DONE]") {
          streamFinished = true;
          break;
        }
        try {
          const parsed = JSON.parse(data) as {
            choices?: Array<{
              delta?: { content?: string };
              finish_reason?: string | null;
            }>;
          };
          const choice = parsed.choices?.[0];
          const delta = choice?.delta?.content;
          if (delta) {
            reply += delta;
            params.onDelta?.(delta);
          }
          if (choice?.finish_reason) {
            streamFinished = true;
            break;
          }
        } catch {
          // ignore partial JSON
        }
      }
    }
  } finally {
    try {
      await reader.cancel();
    } catch {
      // ignore
    }
  }

  return reply;
}

export type ListedModel = { id: string; name?: string };

/**
 * List models from an OpenAI-compatible `GET {base}/models` endpoint.
 */
export async function listOpenAiCompatibleModels(params: {
  apiKey: string;
  baseUrl: string;
}): Promise<ListedModel[]> {
  const base = normalizeBaseUrl(params.baseUrl);
  const url = `${base}/models`;
  const res = await fetch(url, {
    method: "GET",
    headers: {
      Authorization: `Bearer ${params.apiKey}`,
    },
  });
  if (!res.ok) {
    throw new Error(`Failed to list models: ${res.status} ${await res.text()}`);
  }
  const json = (await res.json()) as {
    data?: Array<{ id?: string; name?: string }>;
  };
  const rows = Array.isArray(json.data) ? json.data : [];
  const seen = new Set<string>();
  const models: ListedModel[] = [];
  for (const row of rows) {
    const id = typeof row.id === "string" ? row.id.trim() : "";
    if (!id || seen.has(id)) continue;
    seen.add(id);
    models.push({
      id,
      name: typeof row.name === "string" ? row.name : undefined,
    });
  }
  models.sort((a, b) => a.id.localeCompare(b.id, undefined, { numeric: true }));
  return models;
}

type CursorSdkModelItem = {
  id: string;
  displayName?: string;
  aliases?: string[];
};

/**
 * List models accepted by `@cursor/sdk` via `Cursor.models.list()`.
 * These IDs differ from Cursor CLI `--list-models` (which may include
 * variant suffixes like `grok-4.5-fast-high`).
 */
export async function listCursorSdkModels(params: {
  apiKey: string;
  list: (options: { apiKey: string }) => Promise<CursorSdkModelItem[]>;
}): Promise<ListedModel[]> {
  const items = await params.list({ apiKey: params.apiKey });
  const seen = new Set<string>();
  const models: ListedModel[] = [];
  for (const item of items) {
    const id = typeof item.id === "string" ? item.id.trim() : "";
    if (!id || seen.has(id)) continue;
    seen.add(id);
    models.push({
      id,
      name: typeof item.displayName === "string" ? item.displayName : undefined,
    });
  }
  models.sort((a, b) => a.id.localeCompare(b.id, undefined, { numeric: true }));
  return models;
}

/**
 * Map a user/CLI model id onto an SDK-valid id when possible (exact match or alias).
 */
export function resolveCursorSdkModelId(
  requested: string,
  items: CursorSdkModelItem[],
): string {
  const want = requested.trim();
  if (!want) return want;
  if (items.some((item) => item.id === want)) return want;
  for (const item of items) {
    if (item.aliases?.includes(want)) return item.id;
  }
  // Common CLI variant suffix: `<id>-fast-high` / `<id>-fast` etc. → base id
  for (const item of items) {
    if (want.startsWith(`${item.id}-`)) return item.id;
  }
  return want;
}
