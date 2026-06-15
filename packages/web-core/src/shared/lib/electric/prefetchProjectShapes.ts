import {
  ISSUE_MUTATION,
  PROJECT_ISSUES_SHAPE,
  PROJECT_PROJECT_STATUSES_SHAPE,
  PROJECT_STATUS_MUTATION,
  PROJECT_WORKSPACES_SHAPE,
} from 'shared/remote-types';
import { createShapeCollection } from '@/shared/lib/electric/collections';

/**
 * Warm Electric collections for a project before navigation so the board can
 * render from cache instead of waiting for cold shape sync after route change.
 */
export function prefetchProjectKanbanShapes(projectId: string): void {
  if (!projectId) return;

  const params = { project_id: projectId };
  createShapeCollection(
    PROJECT_ISSUES_SHAPE,
    params,
    undefined,
    ISSUE_MUTATION
  );
  createShapeCollection(
    PROJECT_PROJECT_STATUSES_SHAPE,
    params,
    undefined,
    PROJECT_STATUS_MUTATION
  );
  createShapeCollection(PROJECT_WORKSPACES_SHAPE, params);
}

const HOVER_PREFETCH_DELAY_MS = 120;
let hoverPrefetchTimer: ReturnType<typeof setTimeout> | null = null;
let pendingHoverProjectId: string | null = null;

/**
 * Debounced prefetch for hover intent. Avoids opening shape connections for
 * every project the cursor sweeps across; only warms once the pointer rests
 * briefly on a project. Underlying collections are cached, so a project that
 * has already been warmed is a no-op.
 */
export function prefetchProjectKanbanShapesOnHover(projectId: string): void {
  if (!projectId || projectId === pendingHoverProjectId) return;

  pendingHoverProjectId = projectId;
  if (hoverPrefetchTimer) {
    clearTimeout(hoverPrefetchTimer);
  }
  hoverPrefetchTimer = setTimeout(() => {
    hoverPrefetchTimer = null;
    pendingHoverProjectId = null;
    prefetchProjectKanbanShapes(projectId);
  }, HOVER_PREFETCH_DELAY_MS);
}
