export type AppDestination =
  | { kind: 'root' }
  | { kind: 'onboarding' }
  | { kind: 'onboarding-sign-in' }
  | { kind: 'agents' }
  | { kind: 'workspaces'; hostId?: string }
  | { kind: 'workspaces-create'; hostId?: string }
  | { kind: 'workspace'; workspaceId: string; hostId?: string }
  | { kind: 'workspace-vscode'; workspaceId: string; hostId?: string }
  | { kind: 'export' }
  | { kind: 'project'; projectId: string }
  | { kind: 'project-agents'; projectId: string }
  | { kind: 'project-agent'; projectId: string; agentId: string }
  | { kind: 'project-copilot'; projectId: string }
  | { kind: 'project-inbox'; projectId: string }
  | {
      kind: 'project-issue';
      projectId: string;
      issueId: string;
    }
  | {
      kind: 'project-issue-workspace';
      projectId: string;
      issueId: string;
      workspaceId: string;
      hostId?: string;
    }
  | {
      kind: 'project-issue-workspace-create';
      projectId: string;
      issueId: string;
      draftId: string;
      hostId?: string;
    }
  | {
      kind: 'project-workspace-create';
      projectId: string;
      draftId: string;
      hostId?: string;
    };

export type NavigationTransition = {
  replace?: boolean;
};

export interface AppNavigation {
  resolveFromPath(path: string): AppDestination | null;
  goToRoot(transition?: NavigationTransition): void;
  goToOnboarding(transition?: NavigationTransition): void;
  goToOnboardingSignIn(transition?: NavigationTransition): void;
  goToAgents(transition?: NavigationTransition): void;
  goToWorkspaces(transition?: NavigationTransition): void;
  goToWorkspacesCreate(
    transition?: NavigationTransition,
    hostId?: string | null
  ): void;
  goToWorkspace(workspaceId: string, transition?: NavigationTransition): void;
  goToWorkspaceVsCode(
    workspaceId: string,
    transition?: NavigationTransition
  ): void;
  goToExport(transition?: NavigationTransition): void;
  goToProject(projectId: string, transition?: NavigationTransition): void;
  goToProjectAgents(projectId: string, transition?: NavigationTransition): void;
  goToProjectAgent(
    projectId: string,
    agentId: string,
    transition?: NavigationTransition
  ): void;
  goToProjectCopilot(
    projectId: string,
    transition?: NavigationTransition
  ): void;
  goToProjectInbox(projectId: string, transition?: NavigationTransition): void;
  goToProjectIssue(
    projectId: string,
    issueId: string,
    transition?: NavigationTransition
  ): void;
  goToProjectIssueWorkspace(
    projectId: string,
    issueId: string,
    workspaceId: string,
    transition?: NavigationTransition
  ): void;
  goToProjectIssueWorkspaceCreate(
    projectId: string,
    issueId: string,
    draftId: string,
    transition?: NavigationTransition,
    hostId?: string | null
  ): void;
  goToProjectWorkspaceCreate(
    projectId: string,
    draftId: string,
    transition?: NavigationTransition,
    hostId?: string | null
  ): void;
}

type ProjectDestinationKind =
  | 'project'
  | 'project-agents'
  | 'project-agent'
  | 'project-copilot'
  | 'project-inbox'
  | 'project-issue'
  | 'project-issue-workspace'
  | 'project-issue-workspace-create'
  | 'project-workspace-create';

type WorkspaceDestinationKind =
  | 'workspaces'
  | 'workspaces-create'
  | 'workspace'
  | 'workspace-vscode';

export type ProjectDestination = Extract<
  AppDestination,
  { kind: ProjectDestinationKind }
>;

export type WorkspaceDestination = Extract<
  AppDestination,
  { kind: WorkspaceDestinationKind }
>;

export type KanbanSidebarMode =
  | 'closed'
  | 'issue'
  | 'issue-workspace'
  | 'workspace-create';

export interface KanbanRouteState {
  hostId: string | null;
  projectId: string | null;
  issueId: string | null;
  workspaceId: string | null;
  draftId: string | null;
  sidebarMode: KanbanSidebarMode | null;
  isCreateMode: boolean;
  isWorkspaceCreateMode: boolean;
  hasInvalidWorkspaceCreateDraftId: boolean;
  isPanelOpen: boolean;
}

export function getDestinationHostId(
  destination: AppDestination | null
): string | null {
  if (!destination || !('hostId' in destination)) {
    return null;
  }

  return destination.hostId ?? null;
}

export function isProjectDestination(
  destination: AppDestination | null
): destination is ProjectDestination {
  if (!destination) {
    return false;
  }

  switch (destination.kind) {
    case 'project':
    case 'project-agents':
    case 'project-agent':
    case 'project-copilot':
    case 'project-inbox':
    case 'project-issue':
    case 'project-issue-workspace':
    case 'project-issue-workspace-create':
    case 'project-workspace-create':
      return true;
    default:
      return false;
  }
}

export function isWorkspacesDestination(
  destination: AppDestination | null
): destination is WorkspaceDestination {
  if (!destination) {
    return false;
  }

  switch (destination.kind) {
    case 'workspaces':
    case 'workspaces-create':
    case 'workspace':
    case 'workspace-vscode':
      return true;
    default:
      return false;
  }
}

export function isLocalWorkspacesDestination(
  destination: AppDestination | null
): destination is WorkspaceDestination {
  return (
    isWorkspacesDestination(destination) &&
    getDestinationHostId(destination) === null
  );
}

export function isRemoteWorkspacesDestination(
  destination: AppDestination | null
): destination is WorkspaceDestination {
  return (
    isWorkspacesDestination(destination) &&
    getDestinationHostId(destination) !== null
  );
}

export function getProjectDestination(
  destination: AppDestination | null
): ProjectDestination | null {
  return isProjectDestination(destination) ? destination : null;
}

function isValidUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    value
  );
}

export function resolveKanbanRouteState(
  destination: AppDestination | null
): KanbanRouteState {
  const projectDestination = getProjectDestination(destination);
  const projectId = projectDestination?.projectId ?? null;
  const hostId = getDestinationHostId(projectDestination);

  const issueId = (() => {
    if (!projectDestination) {
      return null;
    }

    switch (projectDestination.kind) {
      case 'project-issue':
      case 'project-issue-workspace':
      case 'project-issue-workspace-create':
        return projectDestination.issueId;
      default:
        return null;
    }
  })();

  const workspaceId =
    projectDestination?.kind === 'project-issue-workspace'
      ? projectDestination.workspaceId
      : null;

  const rawDraftId =
    projectDestination?.kind === 'project-issue-workspace-create' ||
    projectDestination?.kind === 'project-workspace-create'
      ? projectDestination.draftId
      : null;
  const draftId = rawDraftId && isValidUuid(rawDraftId) ? rawDraftId : null;

  const hasInvalidWorkspaceCreateDraftId =
    (projectDestination?.kind === 'project-issue-workspace-create' ||
      projectDestination?.kind === 'project-workspace-create') &&
    rawDraftId !== null &&
    !draftId;

  const isWorkspaceCreateMode =
    (projectDestination?.kind === 'project-issue-workspace-create' ||
      projectDestination?.kind === 'project-workspace-create') &&
    draftId !== null;

  const sidebarMode = (() => {
    if (!projectDestination) {
      return null;
    }

    switch (projectDestination.kind) {
      case 'project':
      case 'project-agents':
      case 'project-agent':
      case 'project-copilot':
      case 'project-inbox':
        return 'closed';
      case 'project-issue':
        return 'issue';
      case 'project-issue-workspace':
        return 'issue-workspace';
      case 'project-issue-workspace-create':
      case 'project-workspace-create':
        return 'workspace-create';
    }
  })();

  const isFullPageProjectView =
    !!projectDestination &&
    (projectDestination.kind === 'project' ||
      projectDestination.kind === 'project-agents' ||
      projectDestination.kind === 'project-agent' ||
      projectDestination.kind === 'project-copilot' ||
      projectDestination.kind === 'project-inbox');

  return {
    hostId,
    projectId,
    issueId,
    workspaceId,
    draftId,
    sidebarMode,
    // Issue-create mode is route-independent and derived from composer state.
    isCreateMode: false,
    isWorkspaceCreateMode,
    hasInvalidWorkspaceCreateDraftId,
    isPanelOpen: !!projectDestination && !isFullPageProjectView,
  };
}
