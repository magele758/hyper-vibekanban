/**
 * Alternate board chat runtimes (Pi / OpenCode) via OpenAI-compatible chat completions.
 * Coding executors remain separate — these are dialogue/orchestration only.
 */

export type ChatMessage = { role: 'system' | 'user' | 'assistant'; content: string };

export type RuntimeChatParams = {
  apiKey: string;
  baseUrl: string;
  model: string;
  messages: ChatMessage[];
  onDelta?: (text: string) => void;
};

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/$/, '');
}

/**
 * Stream (or fall back to non-stream) chat completions against an OpenAI-compatible endpoint.
 * Used for both `pi` and `opencode` board chat runtimes in MVP.
 */
export async function openaiCompatibleChat(
  params: RuntimeChatParams
): Promise<string> {
  const base = normalizeBaseUrl(params.baseUrl);
  const url = `${base}/chat/completions`;
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
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
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
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
        `OpenAI-compatible chat failed: ${res.status} / ${fallback.status} ${await fallback.text()}`
      );
    }
    const json = (await fallback.json()) as {
      choices?: Array<{ message?: { content?: string } }>;
    };
    const text = json.choices?.[0]?.message?.content ?? '';
    if (text) params.onDelta?.(text);
    return text;
  }

  if (!res.body) {
    throw new Error('OpenAI-compatible chat returned empty body');
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let reply = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() ?? '';
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed.startsWith('data:')) continue;
      const data = trimmed.slice(5).trim();
      if (data === '[DONE]') continue;
      try {
        const parsed = JSON.parse(data) as {
          choices?: Array<{ delta?: { content?: string } }>;
        };
        const delta = parsed.choices?.[0]?.delta?.content;
        if (delta) {
          reply += delta;
          params.onDelta?.(delta);
        }
      } catch {
        // ignore partial JSON
      }
    }
  }

  return reply;
}
