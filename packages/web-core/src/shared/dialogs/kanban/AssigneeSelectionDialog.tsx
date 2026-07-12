import { useCallback, useMemo, useRef, useEffect, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import type { Project, Squad } from 'shared/remote-types';
import type { OrganizationMemberWithProfile } from 'shared/types';
import { defineModal } from '@/shared/lib/modals';
import { CommandDialog } from '@vibe/ui/components/Command';
import {
  MultiSelectCommandBar,
  type MultiSelectOption,
} from '@vibe/ui/components/MultiSelectCommandBar';
import { UserAvatar } from '@vibe/ui/components/UserAvatar';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { getExecutionHostId } from '@/shared/lib/executionHostContext';
import {
  getDestinationHostId,
  getProjectDestination,
} from '@/shared/lib/routes/appNavigation';
import {
  buildKanbanIssueComposerKey,
  patchKanbanIssueComposer,
  useKanbanIssueComposer,
} from '@/shared/stores/useKanbanIssueComposerStore';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
export interface AssigneeSelectionDialogProps {
  projectId: string;
  issueIds: string[];
  isCreateMode?: boolean;
  /** Initial assignee IDs for create mode (used instead of URL params when provided) */
  createModeAssigneeIds?: string[];
  /** Callback for create-mode assignee changes (bypasses URL params when provided) */
  onCreateModeAssigneesChange?: (assigneeIds: string[]) => void;
  /** Optional additional options for create-mode selection (e.g. "Me", "Unassigned"). */
  additionalOptions?: MultiSelectOption<string>[];
}

const getUserDisplayName = (user: OrganizationMemberWithProfile): string => {
  return (
    [user.first_name, user.last_name].filter(Boolean).join(' ') ||
    user.username ||
    'User'
  );
};

/** Inner component that uses contexts to render the selection UI */
function AssigneeSelectionContent({
  projectId,
  issueIds,
  isCreateMode,
  createModeAssigneeIds,
  onCreateModeAssigneesChange,
  additionalOptions,
}: {
  projectId: string;
  issueIds: string[];
  isCreateMode: boolean;
  createModeAssigneeIds?: string[];
  onCreateModeAssigneesChange?: (assigneeIds: string[]) => void;
  additionalOptions?: MultiSelectOption<string>[];
}) {
  const { t } = useTranslation('common');
  const modal = useModal();
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const hasCreateCallback = onCreateModeAssigneesChange != null;
  const destination = useCurrentAppDestination();
  const projectDestination = useMemo(
    () => getProjectDestination(destination),
    [destination]
  );
  const resolvedProjectId = projectId || projectDestination?.projectId || null;

  // Load squads for this project
  const [squads, setSquads] = useState<Squad[]>([]);
  useEffect(() => {
    if (!resolvedProjectId) return;
    boardAgentsApi
      .listSquads(resolvedProjectId)
      .then(setSquads)
      .catch(() => undefined);
  }, [resolvedProjectId]);
  const issueComposerKey = useMemo(() => {
    if (!resolvedProjectId) return null;
    const hostId =
      getDestinationHostId(projectDestination) ?? getExecutionHostId();
    return buildKanbanIssueComposerKey(hostId, resolvedProjectId);
  }, [resolvedProjectId, projectDestination]);
  const issueComposer = useKanbanIssueComposer(issueComposerKey);

  // Get users from OrgContext - use membersWithProfilesById for OrganizationMemberWithProfile
  const { membersWithProfilesById } = useOrgContext();
  const users = useMemo(
    () => [...membersWithProfilesById.values()],
    [membersWithProfilesById]
  );

  // Get issue assignees, agents and mutation functions from ProjectContext
  const { issueAssignees, agents, insertIssueAssignee, removeIssueAssignee } =
    useProjectContext();

  // Local state for create mode when using callback pattern
  const [localCreateAssignees, setLocalCreateAssignees] = useState<string[]>(
    createModeAssigneeIds ?? []
  );

  // Keep local create-mode state aligned with incoming source-of-truth values.
  // This avoids stale selections when the draft is reset outside the dialog.
  useEffect(() => {
    if (!hasCreateCallback) return;
    setLocalCreateAssignees(createModeAssigneeIds ?? []);
  }, [hasCreateCallback, createModeAssigneeIds, modal.visible]);

  // Fallback: get/set create mode defaults from shared in-memory state.
  const issueComposerAssigneeIds = issueComposer?.draft.assigneeIds ?? [];

  const setIssueComposerAssigneeIds = useCallback(
    (assigneeIds: string[]) => {
      if (!issueComposerKey) return;
      patchKanbanIssueComposer(issueComposerKey, { assigneeIds });
    },
    [issueComposerKey]
  );

  // Derive selected assignee IDs based on mode and callback availability
  const selectedIds = useMemo(() => {
    if (isCreateMode) {
      return hasCreateCallback
        ? localCreateAssignees
        : issueComposerAssigneeIds;
    }
    return issueAssignees
      .filter((a) => issueIds.includes(a.issue_id))
      .flatMap((a) => {
        if (a.user_id) return [a.user_id];
        if (a.agent_id) return [`agent:${a.agent_id}`];
        if (a.squad_id) return [`squad:${a.squad_id}`];
        return [];
      });
  }, [
    isCreateMode,
    issueIds,
    issueAssignees,
    hasCreateCallback,
    localCreateAssignees,
    issueComposerAssigneeIds,
  ]);

  const [search, setSearch] = useState('');

  // Capture focus when dialog opens and reset search
  useEffect(() => {
    if (modal.visible) {
      previousFocusRef.current = document.activeElement as HTMLElement;
      setSearch('');
    }
  }, [modal.visible]);

  const options: MultiSelectOption<string>[] = useMemo(() => {
    const squadOptions = squads.map((squad) => ({
      value: `squad:${squad.id}`,
      label: `👥 ${squad.name}`,
      searchValue: `squad ${squad.name}`,
      renderOption: () => (
        <div className="flex items-center gap-base">
          <div className="flex h-5 w-5 items-center justify-center rounded-full bg-secondary text-[10px]">
            S
          </div>
          <span>{squad.name}</span>
        </div>
      ),
    }));

    const agentOptions = agents.map((agent) => ({
      value: `agent:${agent.id}`,
      label: `🤖 ${agent.name}`,
      searchValue: `agent ${agent.name} ${agent.instructions}`,
      renderOption: () => (
        <div className="flex items-center gap-base">
          <div className="flex h-5 w-5 items-center justify-center rounded-full bg-secondary text-[10px]">
            A
          </div>
          <span>{agent.name}</span>
        </div>
      ),
    }));

    const userOptions = users.map((user) => ({
      value: user.user_id,
      label: getUserDisplayName(user),
      searchValue: `${user.user_id} ${getUserDisplayName(user)} ${user.email ?? ''}`,
      renderOption: () => (
        <div className="flex items-center gap-base">
          <UserAvatar user={user} className="h-5 w-5 text-[10px]" />
          <span>{getUserDisplayName(user)}</span>
        </div>
      ),
    }));

    if (!isCreateMode || !additionalOptions || additionalOptions.length === 0) {
      return [...squadOptions, ...agentOptions, ...userOptions];
    }

    return [
      ...additionalOptions,
      ...squadOptions,
      ...agentOptions,
      ...userOptions,
    ];
  }, [users, agents, squads, isCreateMode, additionalOptions]);

  const handleToggle = useCallback(
    (selectedValue: string) => {
      const isSelected = selectedIds.includes(selectedValue);
      const isAgent = selectedValue.startsWith('agent:');
      const isSquad = selectedValue.startsWith('squad:');
      const agentId = isAgent ? selectedValue.slice('agent:'.length) : null;
      const squadId = isSquad ? selectedValue.slice('squad:'.length) : null;
      const userId = !isAgent && !isSquad ? selectedValue : null;

      if (isCreateMode) {
        const newIds = isSelected
          ? selectedIds.filter((id: string) => id !== selectedValue)
          : [...selectedIds, selectedValue];
        if (onCreateModeAssigneesChange) {
          setLocalCreateAssignees(newIds);
          onCreateModeAssigneesChange(newIds);
        } else {
          setIssueComposerAssigneeIds(newIds);
        }
      } else {
        for (const issueId of issueIds) {
          if (isSelected) {
            const record = issueAssignees.find(
              (a) =>
                a.issue_id === issueId &&
                (agentId
                  ? a.agent_id === agentId
                  : squadId
                    ? a.squad_id === squadId
                    : a.user_id === userId)
            );
            if (record) {
              removeIssueAssignee(record.id);
            }
          } else if (agentId) {
            insertIssueAssignee({
              issue_id: issueId,
              agent_id: agentId,
            });
          } else if (squadId) {
            insertIssueAssignee({
              issue_id: issueId,
              squad_id: squadId,
            });
          } else if (userId) {
            insertIssueAssignee({
              issue_id: issueId,
              user_id: userId,
            });
          }
        }
      }

      setSearch('');
    },
    [
      isCreateMode,
      selectedIds,
      issueIds,
      issueAssignees,
      onCreateModeAssigneesChange,
      setIssueComposerAssigneeIds,
      insertIssueAssignee,
      removeIssueAssignee,
    ]
  );

  const handleClose = useCallback(() => {
    modal.hide();
  }, [modal]);

  // Restore focus when dialog closes
  const handleCloseAutoFocus = useCallback((event: Event) => {
    event.preventDefault();
    previousFocusRef.current?.focus();
  }, []);

  return (
    <CommandDialog
      open={modal.visible}
      onOpenChange={(open) => !open && modal.hide()}
      onCloseAutoFocus={handleCloseAutoFocus}
    >
      <MultiSelectCommandBar
        title={t('kanban.selectAssignees', 'Select assignees...')}
        options={options}
        selectedValues={selectedIds}
        onToggle={handleToggle}
        onClose={handleClose}
        search={search}
        onSearchChange={setSearch}
      />
    </CommandDialog>
  );
}

/** Wrapper that provides OrgContext and ProjectContext */
function AssigneeSelectionWithContext({
  projectId,
  issueIds,
  isCreateMode = false,
  createModeAssigneeIds,
  onCreateModeAssigneesChange,
  additionalOptions,
}: AssigneeSelectionDialogProps) {
  const destination = useCurrentAppDestination();
  const projectDestination = useMemo(
    () => getProjectDestination(destination),
    [destination]
  );
  const resolvedProjectId = projectId || projectDestination?.projectId;
  // Get organization ID from store (set when navigating to project)
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);

  // Fallback: try to find org from projects if not in store
  const { data: projects = [] } = useOrganizationProjects(selectedOrgId);
  const project = projects.find((p: Project) => p.id === resolvedProjectId);
  const organizationId = project?.organization_id ?? selectedOrgId;

  // If we don't have the required IDs, render nothing
  if (!organizationId || !resolvedProjectId) {
    return null;
  }

  return (
    <OrgProvider organizationId={organizationId}>
      <ProjectProvider projectId={resolvedProjectId}>
        <AssigneeSelectionContent
          projectId={resolvedProjectId}
          issueIds={issueIds}
          isCreateMode={isCreateMode}
          createModeAssigneeIds={createModeAssigneeIds}
          onCreateModeAssigneesChange={onCreateModeAssigneesChange}
          additionalOptions={additionalOptions}
        />
      </ProjectProvider>
    </OrgProvider>
  );
}

const AssigneeSelectionDialogImpl = create<AssigneeSelectionDialogProps>(
  ({
    projectId,
    issueIds,
    isCreateMode,
    createModeAssigneeIds,
    onCreateModeAssigneesChange,
    additionalOptions,
  }) => {
    return (
      <AssigneeSelectionWithContext
        projectId={projectId}
        issueIds={issueIds}
        isCreateMode={isCreateMode}
        createModeAssigneeIds={createModeAssigneeIds}
        onCreateModeAssigneesChange={onCreateModeAssigneesChange}
        additionalOptions={additionalOptions}
      />
    );
  }
);

export const AssigneeSelectionDialog = defineModal<
  AssigneeSelectionDialogProps,
  void
>(AssigneeSelectionDialogImpl);
