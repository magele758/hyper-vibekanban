import type { Workspace as RemoteWorkspace } from 'shared/remote-types';
import { workspacesApi } from '@/shared/lib/api';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import type { DeletableWorkspaceSummary } from '@vibe/ui/components/DeleteIssueDialog';

export interface LinkedWorkspaceClassification {
  /** Local workspaces (worktree/in-place) that can be cascade-deleted */
  deletableWorkspaces: DeletableWorkspaceSummary[];
  /** Number of linked console-mode workspaces, always exempted from deletion */
  exemptedConsoleWorkspaceCount: number;
}

/**
 * Resolves the local workspaces linked to the given issue(s) and classifies
 * them by whether they're safe to cascade-delete. Console-mode workspaces
 * run directly in the repo's own working tree (no dedicated directory) and
 * are always exempted.
 */
export async function classifyLinkedWorkspacesForIssues(
  remoteWorkspaces: RemoteWorkspace[],
  issueIds: string[]
): Promise<LinkedWorkspaceClassification> {
  const localWorkspaceIds = Array.from(
    new Set(
      remoteWorkspaces
        .filter((w) => issueIds.includes(w.issue_id ?? ''))
        .map((w) => w.local_workspace_id)
        .filter((id): id is string => !!id)
    )
  );

  const deletableWorkspaces: DeletableWorkspaceSummary[] = [];
  let exemptedConsoleWorkspaceCount = 0;

  if (localWorkspaceIds.length > 0) {
    const localWorkspaces = await Promise.all(
      localWorkspaceIds.map((id) => workspacesApi.get(id).catch(() => null))
    );
    for (const workspace of localWorkspaces) {
      if (!workspace) continue;
      if (workspace.kind === 'console') {
        exemptedConsoleWorkspaceCount += 1;
      } else {
        deletableWorkspaces.push({
          id: workspace.id,
          name: workspace.name,
          branch: workspace.branch,
        });
      }
    }
  }

  return { deletableWorkspaces, exemptedConsoleWorkspaceCount };
}

/**
 * Deletes the given local workspaces (and their local directories) on a
 * best-effort basis. Failures (e.g. a still-running workspace) are surfaced
 * to the user without blocking the caller's own deletion flow.
 */
export async function deleteWorkspacesBestEffort(
  workspaces: DeletableWorkspaceSummary[]
): Promise<void> {
  const failures: string[] = [];
  for (const workspace of workspaces) {
    try {
      await workspacesApi.delete(workspace.id, false);
    } catch {
      failures.push(workspace.name || workspace.branch);
    }
  }
  if (failures.length > 0) {
    await ConfirmDialog.show({
      title: 'Some Workspaces Could Not Be Deleted',
      message: `Failed to delete: ${failures.join(', ')}. They may still be running.`,
      confirmText: 'OK',
      showCancelButton: false,
    });
  }
}
