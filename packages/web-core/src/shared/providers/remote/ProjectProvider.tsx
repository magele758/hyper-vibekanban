import {
  useMemo,
  useCallback,
  useEffect,
  useState,
  type ReactNode,
} from 'react';
import { useShape } from '@/shared/integrations/electric/hooks';
import {
  PROJECT_ISSUES_SHAPE,
  PROJECT_PROJECT_STATUSES_SHAPE,
  PROJECT_TAGS_SHAPE,
  PROJECT_AGENTS_SHAPE,
  PROJECT_AGENT_TASKS_SHAPE,
  PROJECT_AUTOPILOTS_SHAPE,
  PROJECT_SQUADS_SHAPE,
  PROJECT_SQUAD_MEMBERS_SHAPE,
  PROJECT_ISSUE_ASSIGNEES_SHAPE,
  PROJECT_ISSUE_FOLLOWERS_SHAPE,
  PROJECT_ISSUE_TAGS_SHAPE,
  PROJECT_ISSUE_RELATIONSHIPS_SHAPE,
  PROJECT_PULL_REQUESTS_SHAPE,
  PROJECT_PULL_REQUEST_ISSUES_SHAPE,
  PROJECT_WORKSPACES_SHAPE,
  ISSUE_MUTATION,
  PROJECT_STATUS_MUTATION,
  TAG_MUTATION,
  AGENT_MUTATION,
  ISSUE_ASSIGNEE_MUTATION,
  ISSUE_FOLLOWER_MUTATION,
  ISSUE_TAG_MUTATION,
  ISSUE_RELATIONSHIP_MUTATION,
  PULL_REQUEST_ISSUE_MUTATION,
  type Issue,
  type ProjectStatus,
  type Tag,
} from 'shared/remote-types';
import {
  ProjectContext,
  type ProjectContextValue,
} from '@/shared/hooks/useProjectContext';

interface ProjectProviderProps {
  projectId: string;
  children: ReactNode;
}

const CORE_SHAPE_READY_TIMEOUT_MS = 5_000;

export function ProjectProvider({ projectId, children }: ProjectProviderProps) {
  const params = useMemo(() => ({ project_id: projectId }), [projectId]);
  const enabled = Boolean(projectId);

  // Shape subscriptions (with mutations where needed)
  const issuesResult = useShape(PROJECT_ISSUES_SHAPE, params, {
    enabled,
    mutation: ISSUE_MUTATION,
    readyTimeoutMs: CORE_SHAPE_READY_TIMEOUT_MS,
  });
  const statusesResult = useShape(PROJECT_PROJECT_STATUSES_SHAPE, params, {
    enabled,
    mutation: PROJECT_STATUS_MUTATION,
    readyTimeoutMs: CORE_SHAPE_READY_TIMEOUT_MS,
  });
  const workspacesResult = useShape(PROJECT_WORKSPACES_SHAPE, params, {
    enabled,
    readyTimeoutMs: CORE_SHAPE_READY_TIMEOUT_MS,
  });

  const coreReady =
    enabled && !issuesResult.isLoading && !statusesResult.isLoading;

  const [hydrateSecondary, setHydrateSecondary] = useState(false);
  useEffect(() => {
    setHydrateSecondary(false);
  }, [projectId]);
  useEffect(() => {
    if (!coreReady) return;
    const timer = globalThis.setTimeout(() => setHydrateSecondary(true), 300);
    return () => globalThis.clearTimeout(timer);
  }, [coreReady]);

  const secondaryEnabled = coreReady && hydrateSecondary;

  const tagsResult = useShape(PROJECT_TAGS_SHAPE, params, {
    enabled: secondaryEnabled,
    mutation: TAG_MUTATION,
  });
  // Agents + assignees must be available immediately — deferred secondary
  // hydration can make insertIssueAssignee silently no-op (same class of bug
  // as agents create "假成功").
  const agentsResult = useShape(PROJECT_AGENTS_SHAPE, params, {
    enabled,
    mutation: AGENT_MUTATION,
  });
  const agentTasksResult = useShape(PROJECT_AGENT_TASKS_SHAPE, params, {
    enabled,
  });
  const autopilotsResult = useShape(PROJECT_AUTOPILOTS_SHAPE, params, {
    enabled: secondaryEnabled,
  });
  const squadsResult = useShape(PROJECT_SQUADS_SHAPE, params, {
    enabled: secondaryEnabled,
  });
  const squadMembersResult = useShape(PROJECT_SQUAD_MEMBERS_SHAPE, params, {
    enabled: secondaryEnabled,
  });
  const issueAssigneesResult = useShape(PROJECT_ISSUE_ASSIGNEES_SHAPE, params, {
    enabled,
    mutation: ISSUE_ASSIGNEE_MUTATION,
  });
  const issueFollowersResult = useShape(PROJECT_ISSUE_FOLLOWERS_SHAPE, params, {
    enabled: secondaryEnabled,
    mutation: ISSUE_FOLLOWER_MUTATION,
  });
  const issueTagsResult = useShape(PROJECT_ISSUE_TAGS_SHAPE, params, {
    enabled: secondaryEnabled,
    mutation: ISSUE_TAG_MUTATION,
  });
  const issueRelationshipsResult = useShape(
    PROJECT_ISSUE_RELATIONSHIPS_SHAPE,
    params,
    { enabled: secondaryEnabled, mutation: ISSUE_RELATIONSHIP_MUTATION }
  );
  const pullRequestsResult = useShape(PROJECT_PULL_REQUESTS_SHAPE, params, {
    enabled: secondaryEnabled,
  });
  const pullRequestIssuesResult = useShape(
    PROJECT_PULL_REQUEST_ISSUES_SHAPE,
    params,
    { enabled: secondaryEnabled, mutation: PULL_REQUEST_ISSUE_MUTATION }
  );

  const isWorkspacesLoading = enabled && workspacesResult.isLoading;

  // Board readiness depends on core kanban data only.
  // Other project-scoped shapes hydrate opportunistically after render.
  const isLoading = issuesResult.isLoading || statusesResult.isLoading;

  // First error found
  const error =
    issuesResult.error ||
    statusesResult.error ||
    tagsResult.error ||
    issueAssigneesResult.error ||
    issueFollowersResult.error ||
    issueTagsResult.error ||
    issueRelationshipsResult.error ||
    pullRequestsResult.error ||
    pullRequestIssuesResult.error ||
    workspacesResult.error ||
    null;

  // Combined retry
  const retry = useCallback(() => {
    issuesResult.retry();
    statusesResult.retry();
    tagsResult.retry();
    issueAssigneesResult.retry();
    issueFollowersResult.retry();
    issueTagsResult.retry();
    issueRelationshipsResult.retry();
    pullRequestsResult.retry();
    pullRequestIssuesResult.retry();
    workspacesResult.retry();
  }, [
    issuesResult,
    statusesResult,
    tagsResult,
    issueAssigneesResult,
    issueFollowersResult,
    issueTagsResult,
    issueRelationshipsResult,
    pullRequestsResult,
    pullRequestIssuesResult,
    workspacesResult,
  ]);

  // Computed Maps for O(1) lookup
  const issuesById = useMemo(() => {
    const map = new Map<string, Issue>();
    for (const issue of issuesResult.data) {
      map.set(issue.id, issue);
    }
    return map;
  }, [issuesResult.data]);

  const statusesById = useMemo(() => {
    const map = new Map<string, ProjectStatus>();
    for (const status of statusesResult.data) {
      map.set(status.id, status);
    }
    return map;
  }, [statusesResult.data]);

  const tagsById = useMemo(() => {
    const map = new Map<string, Tag>();
    for (const tag of tagsResult.data) {
      map.set(tag.id, tag);
    }
    return map;
  }, [tagsResult.data]);

  const issuesByStatusId = useMemo(() => {
    const map = new Map<string, Issue[]>();
    for (const issue of issuesResult.data) {
      const list = map.get(issue.status_id);
      if (list) list.push(issue);
      else map.set(issue.status_id, [issue]);
    }
    return map;
  }, [issuesResult.data]);

  const assigneesByIssueId = useMemo(() => {
    const map = new Map<string, typeof issueAssigneesResult.data>();
    for (const row of issueAssigneesResult.data) {
      const list = map.get(row.issue_id);
      if (list) list.push(row);
      else map.set(row.issue_id, [row]);
    }
    return map;
  }, [issueAssigneesResult.data]);

  const followersByIssueId = useMemo(() => {
    const map = new Map<string, typeof issueFollowersResult.data>();
    for (const row of issueFollowersResult.data) {
      const list = map.get(row.issue_id);
      if (list) list.push(row);
      else map.set(row.issue_id, [row]);
    }
    return map;
  }, [issueFollowersResult.data]);

  const tagsByIssueId = useMemo(() => {
    const map = new Map<string, typeof issueTagsResult.data>();
    for (const row of issueTagsResult.data) {
      const list = map.get(row.issue_id);
      if (list) list.push(row);
      else map.set(row.issue_id, [row]);
    }
    return map;
  }, [issueTagsResult.data]);

  const relationshipsByIssueId = useMemo(() => {
    const map = new Map<string, typeof issueRelationshipsResult.data>();
    for (const row of issueRelationshipsResult.data) {
      for (const issueId of [row.issue_id, row.related_issue_id]) {
        const list = map.get(issueId);
        if (list) {
          if (!list.includes(row)) list.push(row);
        } else {
          map.set(issueId, [row]);
        }
      }
    }
    return map;
  }, [issueRelationshipsResult.data]);

  const pullRequestIdsByIssueId = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const link of pullRequestIssuesResult.data) {
      const list = map.get(link.issue_id);
      if (list) list.push(link.pull_request_id);
      else map.set(link.issue_id, [link.pull_request_id]);
    }
    return map;
  }, [pullRequestIssuesResult.data]);

  const pullRequestsById = useMemo(() => {
    const map = new Map(
      pullRequestsResult.data.map((pr) => [pr.id, pr] as const)
    );
    return map;
  }, [pullRequestsResult.data]);

  const workspacesByIssueId = useMemo(() => {
    const map = new Map<string, typeof workspacesResult.data>();
    for (const workspace of workspacesResult.data) {
      if (!workspace.issue_id) continue;
      const list = map.get(workspace.issue_id);
      if (list) list.push(workspace);
      else map.set(workspace.issue_id, [workspace]);
    }
    return map;
  }, [workspacesResult.data]);

  // Lookup helpers
  const getIssue = useCallback(
    (issueId: string) => issuesById.get(issueId),
    [issuesById]
  );

  const getIssuesForStatus = useCallback(
    (statusId: string) => issuesByStatusId.get(statusId) ?? [],
    [issuesByStatusId]
  );

  const getAssigneesForIssue = useCallback(
    (issueId: string) => assigneesByIssueId.get(issueId) ?? [],
    [assigneesByIssueId]
  );

  const getFollowersForIssue = useCallback(
    (issueId: string) => followersByIssueId.get(issueId) ?? [],
    [followersByIssueId]
  );

  const getTagsForIssue = useCallback(
    (issueId: string) => tagsByIssueId.get(issueId) ?? [],
    [tagsByIssueId]
  );

  const getTagObjectsForIssue = useCallback(
    (issueId: string) => {
      const issueTags = tagsByIssueId.get(issueId) ?? [];
      return issueTags
        .map((it) => tagsById.get(it.tag_id))
        .filter((t): t is Tag => t !== undefined);
    },
    [tagsByIssueId, tagsById]
  );

  const getRelationshipsForIssue = useCallback(
    (issueId: string) => relationshipsByIssueId.get(issueId) ?? [],
    [relationshipsByIssueId]
  );

  const getStatus = useCallback(
    (statusId: string) => statusesById.get(statusId),
    [statusesById]
  );

  const getTag = useCallback(
    (tagId: string) => tagsById.get(tagId),
    [tagsById]
  );

  const getPullRequestsForIssue = useCallback(
    (issueId: string) => {
      const prIds = pullRequestIdsByIssueId.get(issueId) ?? [];
      return prIds
        .map((id) => pullRequestsById.get(id))
        .filter((pr): pr is NonNullable<typeof pr> => pr !== undefined);
    },
    [pullRequestIdsByIssueId, pullRequestsById]
  );

  const getWorkspacesForIssue = useCallback(
    (issueId: string) => workspacesByIssueId.get(issueId) ?? [],
    [workspacesByIssueId]
  );

  const value = useMemo<ProjectContextValue>(
    () => ({
      projectId,

      // Data
      issues: issuesResult.data,
      statuses: statusesResult.data,
      tags: tagsResult.data,
      agents: agentsResult.data,
      agentTasks: agentTasksResult.data,
      autopilots: autopilotsResult.data,
      squads: squadsResult.data,
      squadMembers: squadMembersResult.data,
      issueAssignees: issueAssigneesResult.data,
      issueFollowers: issueFollowersResult.data,
      issueTags: issueTagsResult.data,
      issueRelationships: issueRelationshipsResult.data,
      pullRequests: pullRequestsResult.data,
      pullRequestIssues: pullRequestIssuesResult.data,
      workspaces: workspacesResult.data,

      // Loading/error
      isLoading,
      isWorkspacesLoading,
      error,
      retry,

      // Issue mutations
      insertIssue: issuesResult.insert,
      updateIssue: issuesResult.update,
      removeIssue: issuesResult.remove,

      // Status mutations
      insertStatus: statusesResult.insert,
      updateStatus: statusesResult.update,
      removeStatus: statusesResult.remove,

      // Tag mutations
      insertTag: tagsResult.insert,
      updateTag: tagsResult.update,
      removeTag: tagsResult.remove,

      // Agent mutations
      insertAgent: agentsResult.insert,
      updateAgent: agentsResult.update,
      removeAgent: agentsResult.remove,

      // IssueAssignee mutations
      insertIssueAssignee: issueAssigneesResult.insert,
      removeIssueAssignee: issueAssigneesResult.remove,

      // IssueFollower mutations
      insertIssueFollower: issueFollowersResult.insert,
      removeIssueFollower: issueFollowersResult.remove,

      // IssueTag mutations
      insertIssueTag: issueTagsResult.insert,
      removeIssueTag: issueTagsResult.remove,

      // IssueRelationship mutations
      insertIssueRelationship: issueRelationshipsResult.insert,
      removeIssueRelationship: issueRelationshipsResult.remove,

      // PullRequestIssue mutations
      insertPullRequestIssue: pullRequestIssuesResult.insert,
      removePullRequestIssue: pullRequestIssuesResult.remove,

      // Lookup helpers
      getIssue,
      getIssuesForStatus,
      getAssigneesForIssue,
      getFollowersForIssue,
      getTagsForIssue,
      getTagObjectsForIssue,
      getRelationshipsForIssue,
      getStatus,
      getTag,
      getPullRequestsForIssue,
      getWorkspacesForIssue,

      // Computed aggregations
      issuesById,
      statusesById,
      tagsById,
    }),
    [
      projectId,
      issuesResult,
      statusesResult,
      tagsResult,
      agentsResult,
      agentTasksResult,
      autopilotsResult,
      squadsResult,
      squadMembersResult,
      issueAssigneesResult,
      issueFollowersResult,
      issueTagsResult,
      issueRelationshipsResult,
      pullRequestsResult,
      pullRequestIssuesResult,
      workspacesResult,
      isLoading,
      isWorkspacesLoading,
      error,
      retry,
      getIssue,
      getIssuesForStatus,
      getAssigneesForIssue,
      getFollowersForIssue,
      getTagsForIssue,
      getTagObjectsForIssue,
      getRelationshipsForIssue,
      getStatus,
      getTag,
      getPullRequestsForIssue,
      getWorkspacesForIssue,
      issuesById,
      statusesById,
      tagsById,
    ]
  );

  return (
    <ProjectContext.Provider value={value}>{children}</ProjectContext.Provider>
  );
}
