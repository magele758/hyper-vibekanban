/** Live vk-start stack defaults (scripts/vk-ports.sh). */
export const env = {
  baseUrl: process.env.VK_E2E_BASE_URL ?? 'http://localhost:13001',
  localApi: process.env.VK_E2E_API_BASE ?? 'http://127.0.0.1:13002',
  remoteApi: process.env.VK_E2E_REMOTE_BASE ?? 'http://127.0.0.1:13000',
  relayApi: process.env.VK_E2E_RELAY_BASE ?? 'http://127.0.0.1:18082',
};

export async function fetchJson(
  url: string,
  init?: RequestInit
): Promise<{ ok: boolean; status: number; body: unknown }> {
  const res = await fetch(url, init);
  let body: unknown = null;
  const text = await res.text();
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = text;
  }
  return { ok: res.ok, status: res.status, body };
}
