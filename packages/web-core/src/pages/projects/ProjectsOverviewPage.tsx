import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import {
  FolderSimpleIcon,
  GitBranchIcon,
  MagnifyingGlassIcon,
  PlusIcon,
  SquaresFourIcon,
} from '@phosphor-icons/react';
import type { Project } from 'shared/remote-types';
import type { OrganizationWithRole, Repo } from 'shared/types';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import {
  CreateRemoteProjectDialog,
  type CreateRemoteProjectResult,
} from '@/shared/dialogs/org/CreateRemoteProjectDialog';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useAllOrganizationProjects } from '@/shared/hooks/useAllOrganizationProjects';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { getProjectRepoDefaults } from '@/shared/hooks/useProjectRepoDefaults';
import { useUserContext } from '@/shared/hooks/useUserContext';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { repoApi } from '@/shared/lib/api';
import { formatRelativeTime } from '@/shared/lib/date';
import { sortProjectsByOrder } from '@/shared/lib/projectOrder';
import { cn } from '@/shared/lib/utils';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';

type SortMode = 'last-updated' | 'name' | 'workspaces';

export type ProjectOverviewRepo = {
  repoId: string;
  name: string;
  path: string | null;
  targetBranch: string;
};

export type ProjectOverviewCardModel = {
  project: Project;
  organizationName: string;
  workspaceCount: number;
  repos: ProjectOverviewRepo[];
};

type ProjectsOverviewPageProps = {
  organizationName: string | null;
  cards: ProjectOverviewCardModel[];
  isLoading: boolean;
  search: string;
  sortMode: SortMode;
  showRepoDetails: boolean;
  topContent?: ReactNode;
  onSearchChange: (value: string) => void;
  onSortModeChange: (value: SortMode) => void;
  onCreateProject: () => void;
  onOpenProject: (project: Project) => void;
};

function basename(path: string): string {
  const trimmed = path.replace(/\/+$/, '');
  const parts = trimmed.split('/');
  return parts[parts.length - 1] || path;
}

export function ProjectsOverviewPage({
  organizationName,
  cards,
  isLoading,
  search,
  sortMode,
  showRepoDetails,
  topContent,
  onSearchChange,
  onSortModeChange,
  onCreateProject,
  onOpenProject,
}: ProjectsOverviewPageProps) {
  return (
    <div className="h-full overflow-auto bg-primary">
      <div className="mx-auto w-full max-w-6xl px-base py-base sm:px-double sm:py-double">
        {topContent}

        <header className="space-y-half">
          <div className="flex items-center gap-half text-low">
            <SquaresFourIcon className="size-icon-base" weight="bold" />
            <span className="text-sm">Overview</span>
          </div>
          <h1 className="text-2xl font-semibold text-high">Projects</h1>
          <p className="text-sm text-low">
            {organizationName ?? 'All organizations'}
            {' · '}
            {cards.length} {cards.length === 1 ? 'project' : 'projects'}
          </p>
        </header>

        <div className="mt-double flex flex-col gap-base sm:flex-row sm:items-center">
          <label className="relative min-w-0 flex-1">
            <MagnifyingGlassIcon className="pointer-events-none absolute left-base top-1/2 size-icon-base -translate-y-1/2 text-low" />
            <input
              type="search"
              value={search}
              onChange={(event) => onSearchChange(event.target.value)}
              placeholder="Search projects"
              className="w-full rounded border border-border bg-secondary py-half pl-8 pr-base text-base text-normal placeholder:text-low focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </label>
          <div className="flex shrink-0 items-center gap-half">
            <label className="flex items-center gap-half text-sm text-low">
              <span className="uppercase tracking-wide">Sort</span>
              <select
                value={sortMode}
                onChange={(event) =>
                  onSortModeChange(event.target.value as SortMode)
                }
                className="rounded border border-border bg-secondary px-base py-half text-sm text-normal focus:outline-none focus:ring-1 focus:ring-brand"
              >
                <option value="last-updated">Last updated</option>
                <option value="name">Name</option>
                <option value="workspaces">Workspaces</option>
              </select>
            </label>
            <button
              type="button"
              onClick={onCreateProject}
              className="inline-flex items-center gap-half rounded border border-brand/50 bg-secondary px-base py-half text-sm font-medium text-brand hover:border-brand hover:bg-panel"
            >
              <PlusIcon className="size-icon-sm" weight="bold" />
              New
            </button>
          </div>
        </div>

        {isLoading ? (
          <div className="mt-double grid gap-base sm:grid-cols-2 lg:grid-cols-3">
            {Array.from({ length: 6 }).map((_, index) => (
              <div
                key={index}
                className="h-40 animate-pulse rounded-md border border-border bg-secondary"
              />
            ))}
          </div>
        ) : cards.length === 0 ? (
          <section className="mt-double rounded-sm border border-border bg-secondary p-base sm:p-double">
            <h2 className="text-base font-medium text-high">No projects yet</h2>
            <p className="mt-half text-sm text-low">
              Create a project to open a kanban board and attach repos.
            </p>
            <button
              type="button"
              onClick={onCreateProject}
              className="mt-base inline-flex items-center gap-half rounded border border-brand/50 px-base py-half text-sm font-medium text-brand hover:border-brand"
            >
              <PlusIcon className="size-icon-sm" weight="bold" />
              New project
            </button>
          </section>
        ) : (
          <ul className="mt-double grid gap-base sm:grid-cols-2 lg:grid-cols-3">
            {cards.map((card) => (
              <li key={card.project.id}>
                <ProjectOverviewCard
                  card={card}
                  showRepoDetails={showRepoDetails}
                  onOpen={() => onOpenProject(card.project)}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function ProjectOverviewCard({
  card,
  showRepoDetails,
  onOpen,
}: {
  card: ProjectOverviewCardModel;
  showRepoDetails: boolean;
  onOpen: () => void;
}) {
  const { project, organizationName, workspaceCount, repos } = card;
  const primaryRepo = repos[0] ?? null;
  const extraRepoCount = Math.max(repos.length - 1, 0);

  return (
    <button
      type="button"
      onClick={onOpen}
      className={cn(
        'group flex h-full w-full flex-col rounded-md border border-border bg-secondary p-base text-left',
        'transition-colors hover:border-high/20 hover:bg-panel',
        'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand'
      )}
    >
      <div className="flex items-start gap-base">
        <span
          className="mt-0.5 h-2.5 w-2.5 shrink-0 rounded-full"
          style={{ backgroundColor: `hsl(${project.color})` }}
          aria-hidden
        />
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-lg font-medium text-high">
            {project.name}
          </h2>
          <p className="mt-half truncate text-sm text-low">
            {organizationName}
          </p>
        </div>
      </div>

      <div className="mt-base flex items-center gap-half text-sm text-normal">
        <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-success" />
        <span className="truncate">
          {workspaceCount > 0
            ? `${workspaceCount} active workspace${workspaceCount === 1 ? '' : 's'}`
            : 'No workspaces yet'}
        </span>
      </div>

      {showRepoDetails ? (
        <div className="mt-base space-y-half">
          {primaryRepo ? (
            <>
              <div className="flex items-center gap-half text-sm text-normal">
                <FolderSimpleIcon className="size-icon-sm shrink-0 text-low" />
                <span
                  className="truncate"
                  title={primaryRepo.path ?? undefined}
                >
                  {primaryRepo.name}
                  {extraRepoCount > 0 ? ` +${extraRepoCount}` : ''}
                </span>
              </div>
              <div className="flex items-center gap-half text-sm text-low">
                <GitBranchIcon className="size-icon-sm shrink-0" />
                <span className="truncate font-ibm-plex-mono">
                  {primaryRepo.targetBranch}
                </span>
              </div>
            </>
          ) : (
            <p className="text-sm text-low">No default repos configured</p>
          )}
        </div>
      ) : (
        <p className="mt-base text-sm text-low">
          Open to manage board, agents, and workspaces
        </p>
      )}

      <div className="mt-auto flex items-center justify-between gap-base pt-base text-sm text-low">
        <span>
          {repos.length > 0
            ? `${repos.length} repo${repos.length === 1 ? '' : 's'}`
            : `${workspaceCount} workspace${workspaceCount === 1 ? '' : 's'}`}
        </span>
        <span>Updated {formatRelativeTime(project.updated_at)}</span>
      </div>
    </button>
  );
}

type ProjectsOverviewPageContainerProps = {
  topContent?: ReactNode;
  /** When set, only show projects for this organization. */
  organizationId?: string | null;
};

export function ProjectsOverviewPageContainer({
  topContent,
  organizationId,
}: ProjectsOverviewPageContainerProps = {}) {
  const { isLoaded, isSignedIn } = useAuth();
  const runtime = useAppRuntime();
  const appNavigation = useAppNavigation();
  const { workspaces } = useUserContext();
  const { data: orgsData, isLoading: orgsLoading } = useUserOrganizations();
  const { data: allProjects = [], isLoading: projectsLoading } =
    useAllOrganizationProjects({ enabled: isSignedIn });
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const setSelectedOrgId = useOrganizationStore((s) => s.setSelectedOrgId);

  const [search, setSearch] = useState('');
  const [sortMode, setSortMode] = useState<SortMode>('last-updated');
  const [repoDefaultsByProject, setRepoDefaultsByProject] = useState<
    Map<string, ProjectOverviewRepo[]>
  >(new Map());

  const organizations = useMemo(
    () => orgsData?.organizations ?? [],
    [orgsData?.organizations]
  );

  const orgById = useMemo(() => {
    const map = new Map<string, OrganizationWithRole>();
    for (const org of organizations) {
      map.set(org.id, org);
    }
    return map;
  }, [organizations]);

  const scopedOrgId = organizationId ?? selectedOrgId;
  const scopedOrgName = scopedOrgId
    ? (orgById.get(scopedOrgId)?.name ?? null)
    : null;

  const workspaceCountByProject = useMemo(() => {
    const map = new Map<string, number>();
    for (const workspace of workspaces) {
      if (workspace.archived) continue;
      map.set(workspace.project_id, (map.get(workspace.project_id) ?? 0) + 1);
    }
    return map;
  }, [workspaces]);

  const projects = useMemo(() => {
    const filtered = scopedOrgId
      ? allProjects.filter((project) => project.organization_id === scopedOrgId)
      : allProjects;
    return sortProjectsByOrder(filtered);
  }, [allProjects, scopedOrgId]);

  useEffect(() => {
    if (!isSignedIn || runtime !== 'local' || projects.length === 0) {
      setRepoDefaultsByProject(new Map());
      return;
    }

    let cancelled = false;

    const load = async () => {
      try {
        const repos = await repoApi.list();
        if (cancelled) return;
        const repoMap = new Map<string, Repo>(
          repos.map((repo) => [repo.id, repo])
        );

        const entries = await Promise.all(
          projects.map(async (project) => {
            const defaults = await getProjectRepoDefaults(project.id);
            const overviewRepos: ProjectOverviewRepo[] = (defaults ?? []).map(
              (draft) => {
                const repo = repoMap.get(draft.repo_id);
                return {
                  repoId: draft.repo_id,
                  name:
                    repo?.display_name || repo?.name || basename(draft.repo_id),
                  path: repo?.path ?? null,
                  targetBranch: draft.target_branch,
                };
              }
            );
            return [project.id, overviewRepos] as const;
          })
        );

        if (!cancelled) {
          setRepoDefaultsByProject(new Map(entries));
        }
      } catch (error) {
        console.error('[ProjectsOverview] Failed to load repo defaults', error);
        if (!cancelled) {
          setRepoDefaultsByProject(new Map());
        }
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [isSignedIn, projects, runtime]);

  const cards = useMemo(() => {
    const query = search.trim().toLowerCase();
    const models: ProjectOverviewCardModel[] = projects.map((project) => ({
      project,
      organizationName:
        orgById.get(project.organization_id)?.name ?? 'Organization',
      workspaceCount: workspaceCountByProject.get(project.id) ?? 0,
      repos: repoDefaultsByProject.get(project.id) ?? [],
    }));

    const filtered = query
      ? models.filter((card) => {
          const haystack = [
            card.project.name,
            card.organizationName,
            ...card.repos.map((repo) => repo.name),
            ...card.repos.map((repo) => repo.targetBranch),
            ...card.repos.map((repo) => repo.path ?? ''),
          ]
            .join(' ')
            .toLowerCase();
          return haystack.includes(query);
        })
      : models;

    const sorted = [...filtered];
    switch (sortMode) {
      case 'name':
        sorted.sort((a, b) => a.project.name.localeCompare(b.project.name));
        break;
      case 'workspaces':
        sorted.sort((a, b) => b.workspaceCount - a.workspaceCount);
        break;
      case 'last-updated':
        sorted.sort(
          (a, b) =>
            new Date(b.project.updated_at).getTime() -
            new Date(a.project.updated_at).getTime()
        );
        break;
      default: {
        const _exhaustive: never = sortMode;
        return _exhaustive;
      }
    }
    return sorted;
  }, [
    orgById,
    projects,
    repoDefaultsByProject,
    search,
    sortMode,
    workspaceCountByProject,
  ]);

  const handleCreateProject = useCallback(async () => {
    const orgId = scopedOrgId ?? organizations[0]?.id;
    if (!orgId) return;

    try {
      const result: CreateRemoteProjectResult =
        await CreateRemoteProjectDialog.show({ organizationId: orgId });
      if (result.action === 'created' && result.project) {
        setSelectedOrgId(result.project.organization_id);
        appNavigation.goToProject(result.project.id);
      }
    } catch {
      // Dialog cancelled
    }
  }, [appNavigation, organizations, scopedOrgId, setSelectedOrgId]);

  const handleOpenProject = useCallback(
    (project: Project) => {
      setSelectedOrgId(project.organization_id);
      appNavigation.goToProject(project.id);
    },
    [appNavigation, setSelectedOrgId]
  );

  if (!isLoaded) {
    return null;
  }

  if (!isSignedIn) {
    return (
      <div className="flex h-full items-center justify-center p-double">
        <LoginRequiredPrompt />
      </div>
    );
  }

  return (
    <ProjectsOverviewPage
      organizationName={scopedOrgName}
      cards={cards}
      isLoading={orgsLoading || projectsLoading}
      search={search}
      sortMode={sortMode}
      showRepoDetails={runtime === 'local'}
      topContent={topContent}
      onSearchChange={setSearch}
      onSortModeChange={setSortMode}
      onCreateProject={() => {
        void handleCreateProject();
      }}
      onOpenProject={handleOpenProject}
    />
  );
}
