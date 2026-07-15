import { router } from "@remote/app/router";
import type { FileRouteTypes } from "@remote/routeTree.gen";
import {
  type AppDestination,
  type AppNavigation,
  type NavigationTransition,
} from "@/shared/lib/routes/appNavigation";
import { getRelayHostFallback } from "@/shared/lib/relayHostFallback";

type RemoteRouteId = FileRouteTypes["id"];

function getPathParam(
  routeParams: Record<string, string>,
  key: string,
): string | null {
  const value = routeParams[key];
  return value ? value : null;
}

export function resolveRemoteDestinationFromPath(
  path: string,
): AppDestination | null {
  const { pathname } = new URL(path, "http://localhost");
  const { foundRoute, routeParams } = router.getMatchedRoutes(pathname);

  if (!foundRoute) {
    return null;
  }

  switch (foundRoute.id as RemoteRouteId) {
    case "/":
      return { kind: "root" };
    case "/export":
      return { kind: "export" };
    case "/hosts/$hostId/workspaces": {
      const hostId = getPathParam(routeParams, "hostId");
      return hostId ? { kind: "workspaces", hostId } : null;
    }
    case "/hosts/$hostId/workspaces_/create": {
      const hostId = getPathParam(routeParams, "hostId");
      return hostId ? { kind: "workspaces-create", hostId } : null;
    }
    case "/hosts/$hostId/workspaces_/$workspaceId": {
      const hostId = getPathParam(routeParams, "hostId");
      const workspaceId = getPathParam(routeParams, "workspaceId");
      return hostId && workspaceId
        ? { kind: "workspace", hostId, workspaceId }
        : null;
    }
    case "/hosts/$hostId/workspaces/$workspaceId/vscode": {
      const hostId = getPathParam(routeParams, "hostId");
      const workspaceId = getPathParam(routeParams, "workspaceId");
      return hostId && workspaceId
        ? { kind: "workspace-vscode", hostId, workspaceId }
        : null;
    }
    case "/projects/$projectId": {
      const projectId = getPathParam(routeParams, "projectId");
      return projectId ? { kind: "project", projectId } : null;
    }
    case "/projects/$projectId_/agents": {
      const projectId = getPathParam(routeParams, "projectId");
      return projectId ? { kind: "project-agents", projectId } : null;
    }
    case "/projects/$projectId_/agents_/$agentId": {
      const projectId = getPathParam(routeParams, "projectId");
      const agentId = getPathParam(routeParams, "agentId");
      return projectId && agentId
        ? { kind: "project-agent", projectId, agentId }
        : null;
    }
    case "/projects/$projectId_/copilot": {
      const projectId = getPathParam(routeParams, "projectId");
      return projectId ? { kind: "project-copilot", projectId } : null;
    }
    case "/projects/$projectId_/inbox": {
      const projectId = getPathParam(routeParams, "projectId");
      return projectId ? { kind: "project-inbox", projectId } : null;
    }
    case "/projects/$projectId_/issues/$issueId": {
      const projectId = getPathParam(routeParams, "projectId");
      const issueId = getPathParam(routeParams, "issueId");
      return projectId && issueId
        ? { kind: "project-issue", projectId, issueId }
        : null;
    }
    case "/projects/$projectId_/issues/$issueId_/hosts/$hostId/workspaces/$workspaceId": {
      const projectId = getPathParam(routeParams, "projectId");
      const issueId = getPathParam(routeParams, "issueId");
      const hostId = getPathParam(routeParams, "hostId");
      const workspaceId = getPathParam(routeParams, "workspaceId");
      return projectId && issueId && hostId && workspaceId
        ? {
            kind: "project-issue-workspace",
            projectId,
            issueId,
            hostId,
            workspaceId,
          }
        : null;
    }
    case "/projects/$projectId_/issues/$issueId_/hosts/$hostId/workspaces/create/$draftId": {
      const projectId = getPathParam(routeParams, "projectId");
      const issueId = getPathParam(routeParams, "issueId");
      const hostId = getPathParam(routeParams, "hostId");
      const draftId = getPathParam(routeParams, "draftId");
      return projectId && issueId && hostId && draftId
        ? {
            kind: "project-issue-workspace-create",
            projectId,
            issueId,
            hostId,
            draftId,
          }
        : null;
    }
    case "/projects/$projectId_/hosts/$hostId/workspaces/create/$draftId": {
      const projectId = getPathParam(routeParams, "projectId");
      const hostId = getPathParam(routeParams, "hostId");
      const draftId = getPathParam(routeParams, "draftId");
      return projectId && hostId && draftId
        ? {
            kind: "project-workspace-create",
            projectId,
            hostId,
            draftId,
          }
        : null;
    }
    default:
      return null;
  }
}

function destinationToRemoteTarget(
  destination: AppDestination,
  options: { currentHostId: string | null },
) {
  const destinationHostId =
    "hostId" in destination ? (destination.hostId ?? null) : null;
  const effectiveHostId = destinationHostId ?? options.currentHostId;

  switch (destination.kind) {
    case "root":
      return { to: "/" } as const;
    case "onboarding":
      return { to: "/" } as const;
    case "onboarding-sign-in":
      return { to: "/" } as const;
    case "workspaces":
      if (effectiveHostId) {
        return {
          to: "/hosts/$hostId/workspaces",
          params: { hostId: effectiveHostId },
        } as const;
      }
      return { to: "/" } as const;
    case "workspaces-create":
      if (effectiveHostId) {
        return {
          to: "/hosts/$hostId/workspaces/create",
          params: { hostId: effectiveHostId },
        } as const;
      }
      return { to: "/" } as const;
    case "workspace":
      if (effectiveHostId) {
        return {
          to: "/hosts/$hostId/workspaces/$workspaceId",
          params: {
            hostId: effectiveHostId,
            workspaceId: destination.workspaceId,
          },
        } as const;
      }
      return { to: "/" } as const;
    case "workspace-vscode":
      if (effectiveHostId) {
        return {
          to: "/hosts/$hostId/workspaces/$workspaceId/vscode",
          params: {
            hostId: effectiveHostId,
            workspaceId: destination.workspaceId,
          },
        } as const;
      }
      return { to: "/" } as const;
    case "export":
      return { to: "/export" } as const;
    case "project":
      return {
        to: "/projects/$projectId",
        params: { projectId: destination.projectId },
      } as const;
    case "project-agents":
      return {
        to: "/projects/$projectId/agents",
        params: { projectId: destination.projectId },
      } as const;
    case "project-agent":
      return {
        to: "/projects/$projectId/agents/$agentId",
        params: {
          projectId: destination.projectId,
          agentId: destination.agentId,
        },
      } as const;
    case "project-copilot":
      return {
        to: "/projects/$projectId/copilot",
        params: { projectId: destination.projectId },
      } as const;
    case "project-inbox":
      return {
        to: "/projects/$projectId/inbox",
        params: { projectId: destination.projectId },
      } as const;
    case "project-issue":
      return {
        to: "/projects/$projectId/issues/$issueId",
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
        },
      } as const;
    case "project-issue-workspace":
      if (!effectiveHostId) {
        return { to: "/" } as const;
      }
      return {
        to: "/projects/$projectId/issues/$issueId/hosts/$hostId/workspaces/$workspaceId",
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
          hostId: effectiveHostId,
          workspaceId: destination.workspaceId,
        },
      } as const;
    case "project-issue-workspace-create":
      if (!effectiveHostId) {
        return { to: "/" } as const;
      }
      return {
        to: "/projects/$projectId/issues/$issueId/hosts/$hostId/workspaces/create/$draftId",
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
          hostId: effectiveHostId,
          draftId: destination.draftId,
        },
      } as const;
    case "project-workspace-create":
      if (!effectiveHostId) {
        return { to: "/" } as const;
      }
      return {
        to: "/projects/$projectId/hosts/$hostId/workspaces/create/$draftId",
        params: {
          projectId: destination.projectId,
          hostId: effectiveHostId,
          draftId: destination.draftId,
        },
      } as const;
  }
}

export function createRemoteHostAppNavigation(hostId: string): AppNavigation {
  const navigateTo = (
    destination: AppDestination,
    transition?: NavigationTransition,
  ) => {
    void router.navigate({
      ...destinationToRemoteTarget(destination, {
        currentHostId: hostId,
      }),
      ...(transition?.replace !== undefined
        ? { replace: transition.replace }
        : {}),
    });
  };

  const navigation: AppNavigation = {
    resolveFromPath: (path) => resolveRemoteDestinationFromPath(path),
    goToRoot: (transition) => navigateTo({ kind: "root" }, transition),
    goToOnboarding: (transition) =>
      navigateTo({ kind: "onboarding" }, transition),
    goToOnboardingSignIn: (transition) =>
      navigateTo({ kind: "onboarding-sign-in" }, transition),
    goToWorkspaces: (transition) =>
      navigateTo({ kind: "workspaces", hostId }, transition),
    goToWorkspacesCreate: (transition, _hostId) =>
      navigateTo({ kind: "workspaces-create", hostId }, transition),
    goToWorkspace: (workspaceId, transition) =>
      navigateTo({ kind: "workspace", hostId, workspaceId }, transition),
    goToWorkspaceVsCode: (workspaceId, transition) =>
      navigateTo({ kind: "workspace-vscode", hostId, workspaceId }, transition),
    goToExport: (transition) => navigateTo({ kind: "export" }, transition),
    goToProject: (projectId, transition) =>
      navigateTo({ kind: "project", projectId }, transition),
    goToProjectAgents: (projectId, transition) =>
      navigateTo({ kind: "project-agents", projectId }, transition),
    goToProjectAgent: (projectId, agentId, transition) =>
      navigateTo({ kind: "project-agent", projectId, agentId }, transition),
    goToProjectCopilot: (projectId, transition) =>
      navigateTo({ kind: "project-copilot", projectId }, transition),
    goToProjectInbox: (projectId, transition) =>
      navigateTo({ kind: "project-inbox", projectId }, transition),
    goToProjectIssue: (projectId, issueId, transition) =>
      navigateTo({ kind: "project-issue", projectId, issueId }, transition),
    goToProjectIssueWorkspace: (projectId, issueId, workspaceId, transition) =>
      navigateTo(
        {
          kind: "project-issue-workspace",
          hostId,
          projectId,
          issueId,
          workspaceId,
        },
        transition,
      ),
    goToProjectIssueWorkspaceCreate: (
      projectId,
      issueId,
      draftId,
      transition,
      _hostId,
    ) =>
      navigateTo(
        {
          kind: "project-issue-workspace-create",
          hostId,
          projectId,
          issueId,
          draftId,
        },
        transition,
      ),
    goToProjectWorkspaceCreate: (projectId, draftId, transition, _hostId) =>
      navigateTo(
        { kind: "project-workspace-create", hostId, projectId, draftId },
        transition,
      ),
  };

  return navigation;
}

function createRemoteFallbackAppNavigation(): AppNavigation {
  const navigateTo = (
    destination: AppDestination,
    transition?: NavigationTransition,
  ) => {
    void router.navigate({
      ...destinationToRemoteTarget(destination, {
        currentHostId: getRelayHostFallback(),
      }),
      ...(transition?.replace !== undefined
        ? { replace: transition.replace }
        : {}),
    });
  };

  const navigation: AppNavigation = {
    resolveFromPath: (path) => resolveRemoteDestinationFromPath(path),
    goToRoot: (transition) => navigateTo({ kind: "root" }, transition),
    goToOnboarding: (transition) =>
      navigateTo({ kind: "onboarding" }, transition),
    goToOnboardingSignIn: (transition) =>
      navigateTo({ kind: "onboarding-sign-in" }, transition),
    goToWorkspaces: (transition) =>
      navigateTo({ kind: "workspaces" }, transition),
    goToWorkspacesCreate: (transition, _hostId) =>
      navigateTo({ kind: "workspaces-create" }, transition),
    goToWorkspace: (workspaceId, transition) =>
      navigateTo({ kind: "workspace", workspaceId }, transition),
    goToWorkspaceVsCode: (workspaceId, transition) =>
      navigateTo({ kind: "workspace-vscode", workspaceId }, transition),
    goToExport: (transition) => navigateTo({ kind: "export" }, transition),
    goToProject: (projectId, transition) =>
      navigateTo({ kind: "project", projectId }, transition),
    goToProjectAgents: (projectId, transition) =>
      navigateTo({ kind: "project-agents", projectId }, transition),
    goToProjectAgent: (projectId, agentId, transition) =>
      navigateTo({ kind: "project-agent", projectId, agentId }, transition),
    goToProjectCopilot: (projectId, transition) =>
      navigateTo({ kind: "project-copilot", projectId }, transition),
    goToProjectInbox: (projectId, transition) =>
      navigateTo({ kind: "project-inbox", projectId }, transition),
    goToProjectIssue: (projectId, issueId, transition) =>
      navigateTo({ kind: "project-issue", projectId, issueId }, transition),
    goToProjectIssueWorkspace: (projectId, issueId, workspaceId, transition) =>
      navigateTo(
        { kind: "project-issue-workspace", projectId, issueId, workspaceId },
        transition,
      ),
    goToProjectIssueWorkspaceCreate: (
      projectId,
      issueId,
      draftId,
      transition,
      _hostId,
    ) =>
      navigateTo(
        { kind: "project-issue-workspace-create", projectId, issueId, draftId },
        transition,
      ),
    goToProjectWorkspaceCreate: (projectId, draftId, transition, _hostId) =>
      navigateTo(
        { kind: "project-workspace-create", projectId, draftId },
        transition,
      ),
  };

  return navigation;
}

export const remoteFallbackAppNavigation = createRemoteFallbackAppNavigation();
