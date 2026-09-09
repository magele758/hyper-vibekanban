const LITE_BLOCKED_PATH_PREFIXES = [
  '/projects',
  '/agents',
  '/workforce',
  '/notifications',
  '/export',
  '/hosts',
  '/onboarding',
] as const;

export function isViteLiteMode(): boolean {
  return import.meta.env.VITE_VK_LITE === '1';
}

export function resolveLiteMode(serverLiteMode?: boolean | null): boolean {
  return serverLiteMode === true || isViteLiteMode();
}

export function isLiteAllowedPath(pathname: string): boolean {
  const path = pathname.split('?')[0] || '/';
  return !LITE_BLOCKED_PATH_PREFIXES.some(
    (prefix) => path === prefix || path.startsWith(`${prefix}/`)
  );
}

export type WorkspaceRepoLink = {
  workspace_id: string;
  repo_id: string;
};

export function groupWorkspaceIdsByRepo(
  links: WorkspaceRepoLink[]
): Map<string, string[]> {
  const byRepo = new Map<string, string[]>();
  for (const link of links) {
    const ids = byRepo.get(link.repo_id);
    if (ids) {
      ids.push(link.workspace_id);
    } else {
      byRepo.set(link.repo_id, [link.workspace_id]);
    }
  }
  return byRepo;
}

export function workspaceMatchesRepoFilter(
  workspaceId: string,
  selectedRepoIds: string[],
  workspaceIdsByRepo: Map<string, string[]>,
  noRepoId: string
): boolean {
  if (selectedRepoIds.length === 0) {
    return true;
  }

  const includeUnlinked = selectedRepoIds.includes(noRepoId);
  const realRepoIds = selectedRepoIds.filter((id) => id !== noRepoId);
  const linkedRepoIds = [...workspaceIdsByRepo.entries()]
    .filter(([, workspaceIds]) => workspaceIds.includes(workspaceId))
    .map(([repoId]) => repoId);

  if (linkedRepoIds.length === 0) {
    return includeUnlinked;
  }

  return realRepoIds.some((repoId) => linkedRepoIds.includes(repoId));
}
