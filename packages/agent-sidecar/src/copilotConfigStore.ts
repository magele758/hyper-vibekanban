/**
 * Local sidecar store for global-copilot (agent_id: null) model config, keyed
 * by project_id. api_key stays on this machine and is never returned by GET.
 *
 * Default path is under VK_STATE_DIR (~/.vk-kanban), not the repo cwd — the
 * file contains secrets and must not show up in git status.
 *
 * ponytail: single JSON file, whole-map read/write. Fine for a handful of
 * projects on a single sidecar instance; move to the remote DB if it ever needs
 * multi-instance or per-user scoping.
 */
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";

export type CopilotModelConfig = {
  base_url?: string;
  api_key?: string;
  model?: string;
};

function resolveFile(): string {
  if (process.env.VK_COPILOT_CONFIG_FILE) {
    return process.env.VK_COPILOT_CONFIG_FILE;
  }
  const stateDir =
    process.env.VK_STATE_DIR ?? path.join(os.homedir(), ".vk-kanban");
  return path.join(stateDir, "copilot-model-config.json");
}

const FILE = resolveFile();

type Store = Record<string, CopilotModelConfig>;

async function readAll(): Promise<Store> {
  try {
    return JSON.parse(await fs.readFile(FILE, "utf8")) as Store;
  } catch {
    return {};
  }
}

export async function getConfig(
  projectId: string,
): Promise<CopilotModelConfig | null> {
  const all = await readAll();
  return all[projectId] ?? null;
}

/** Merge patch: only overwrites provided keys. Empty string clears a key. */
export async function upsertConfig(
  projectId: string,
  patch: CopilotModelConfig,
): Promise<CopilotModelConfig> {
  const all = await readAll();
  const next: CopilotModelConfig = { ...all[projectId] };
  for (const k of ["base_url", "api_key", "model"] as const) {
    const v = patch[k];
    if (v === undefined) continue;
    if (v.trim() === "") delete next[k];
    else next[k] = v.trim();
  }
  all[projectId] = next;
  await fs.mkdir(path.dirname(FILE), { recursive: true });
  await fs.writeFile(FILE, JSON.stringify(all, null, 2), {
    encoding: "utf8",
    mode: 0o600,
  });
  await fs.chmod(FILE, 0o600);
  return next;
}
