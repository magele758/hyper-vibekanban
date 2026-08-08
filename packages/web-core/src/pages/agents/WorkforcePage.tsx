import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ArrowsClockwiseIcon,
  PlusIcon,
  RobotIcon,
  WrenchIcon,
} from '@phosphor-icons/react';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { configApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import type { OrgAgentEntry } from 'shared/remote-types';
import type { AvailableCodingAgent } from 'shared/types';
import { WorkforceCreatePanel } from './WorkforceCreatePanel';

/**
 * The workforce roster: every configured agent across the organization, plus
 * the coding agents installed on this machine that they can be pinned to.
 *
 * Configured agents live in remote (project-scoped rows); installed coding
 * agents are a property of the local host. Keeping both on one page makes the
 * relationship visible: a configured agent is a persona, and its executor is
 * the hand it works with.
 */
function WorkforceInner() {
  const { selectedOrgId } = useOrganizationStore();
  const { data: projects } = useOrganizationProjects(selectedOrgId);
  const navigation = useAppNavigation();

  const [roster, setRoster] = useState<OrgAgentEntry[]>([]);
  const [rosterLoading, setRosterLoading] = useState(true);
  const [rosterError, setRosterError] = useState<string | null>(null);

  const [installed, setInstalled] = useState<AvailableCodingAgent[]>([]);
  const [installedLoading, setInstalledLoading] = useState(true);

  const [creating, setCreating] = useState(false);

  const loadRoster = useCallback(async () => {
    if (!selectedOrgId) return;
    setRosterLoading(true);
    setRosterError(null);
    try {
      setRoster(await boardAgentsApi.listOrgAgents(selectedOrgId));
    } catch (err) {
      setRosterError(err instanceof Error ? err.message : String(err));
    } finally {
      setRosterLoading(false);
    }
  }, [selectedOrgId]);

  useEffect(() => {
    void loadRoster();
  }, [loadRoster]);

  // Installed coding agents come from the local host, so this reflects what can
  // actually run here rather than what the org has configured.
  useEffect(() => {
    setInstalledLoading(true);
    void configApi
      .listAvailableAgents()
      .then((res) => setInstalled(res.agents))
      .catch(() => setInstalled([]))
      .finally(() => setInstalledLoading(false));
  }, []);

  const byProject = useMemo(() => {
    const groups = new Map<string, OrgAgentEntry[]>();
    for (const entry of roster) {
      const list = groups.get(entry.project_id) ?? [];
      list.push(entry);
      groups.set(entry.project_id, list);
    }
    return [...groups.entries()];
  }, [roster]);

  const installedCount = installed.filter((item) => item.available).length;

  return (
    <div className="h-full overflow-y-auto px-6 py-6">
      <div className="mx-auto max-w-4xl">
        <header className="mb-6 flex items-start justify-between gap-4">
          <div>
            <h1 className="text-lg font-medium text-normal">员工</h1>
            <p className="mt-1 text-sm text-low">
              配置好的 Agent 是「人设」，本机的 coding agent 是他们干活用的
              「手」。两者都在这里。
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              aria-label="刷新员工列表"
              className="rounded-md border border-border p-2 text-low hover:bg-secondary hover:text-normal"
              onClick={() => void loadRoster()}
            >
              <ArrowsClockwiseIcon
                className={cn('size-4', rosterLoading && 'animate-spin')}
              />
            </button>
            <PrimaryButton
              onClick={() => setCreating((v) => !v)}
              disabled={projects.length === 0}
            >
              <PlusIcon className="mr-1 size-4" />
              新增员工
            </PrimaryButton>
          </div>
        </header>

        {creating && (
          <WorkforceCreatePanel
            projects={projects.map((p) => ({ id: p.id, name: p.name }))}
            installed={installed}
            existingAgents={roster}
            onCancel={() => setCreating(false)}
            onCreated={() => {
              setCreating(false);
              void loadRoster();
            }}
          />
        )}

        {rosterError && (
          <p className="mb-4 text-sm text-destructive">{rosterError}</p>
        )}

        {/* Configured agents, grouped by the project that owns them. */}
        <section className="mb-8">
          <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-low">
            配置的 Agent（{roster.length}）
          </h2>
          {rosterLoading && roster.length === 0 ? (
            <p className="text-sm text-low">加载中…</p>
          ) : roster.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border p-6 text-center">
              <RobotIcon className="mx-auto mb-2 size-6 text-low" />
              <p className="text-sm text-low">
                还没有配置任何 Agent。点「新增员工」创建第一个。
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {byProject.map(([projectId, entries]) => (
                <div key={projectId}>
                  <p className="mb-1 text-xs text-low">
                    {entries[0].project_name}
                  </p>
                  <ul className="divide-y divide-border overflow-hidden rounded-lg border border-border">
                    {entries.map((entry) => (
                      <AgentRow
                        key={entry.id}
                        entry={entry}
                        installed={installed}
                        reviewerName={
                          roster.find((a) => a.id === entry.reviewer_agent_id)
                            ?.name ?? null
                        }
                        onOpen={() =>
                          navigation.goToProjectAgent(
                            entry.project_id,
                            entry.id
                          )
                        }
                      />
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          )}
        </section>
        {/* Installed coding agents: a property of this machine, not the org. */}
        <section>
          <h2 className="mb-1 text-xs font-medium uppercase tracking-wide text-low">
            本机的 Coding Agent（{installedCount}/{installed.length} 可用）
          </h2>
          <p className="mb-2 text-xs text-low">
            这是本台机器上装了什么，与组织配置无关。未安装的也列出来，方便你知道
            还能装什么。
          </p>
          {installedLoading && installed.length === 0 ? (
            <p className="text-sm text-low">检测中…</p>
          ) : (
            <ul className="grid gap-2 sm:grid-cols-2">
              {installed.map((item) => (
                <li
                  key={item.executor}
                  className={cn(
                    'flex items-center gap-2 rounded-lg border border-border px-3 py-2',
                    !item.available && 'opacity-55'
                  )}
                >
                  <WrenchIcon className="size-4 shrink-0 text-low" />
                  <span className="min-w-0 flex-1 truncate text-sm text-normal">
                    {item.executor}
                  </span>
                  <span
                    className={cn(
                      'shrink-0 rounded px-1.5 py-0.5 text-[11px]',
                      item.available
                        ? 'bg-success/15 text-success'
                        : 'bg-secondary text-low'
                    )}
                  >
                    {item.available ? '可用' : '未安装'}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
/**
 * One agent in the roster. Surfaces the executor prominently because a persona
 * without a working hand is the failure mode this view exists to make obvious.
 */
function AgentRow({
  entry,
  installed,
  reviewerName,
  onOpen,
}: {
  entry: OrgAgentEntry;
  installed: AvailableCodingAgent[];
  reviewerName: string | null;
  onOpen: () => void;
}) {
  // Only judge availability once the host list has loaded, otherwise every
  // agent would briefly look broken.
  const executorMissing =
    entry.default_executor !== null &&
    installed.length > 0 &&
    !installed.some(
      (item) => item.executor === entry.default_executor && item.available
    );

  return (
    <li>
      <button
        type="button"
        onClick={onOpen}
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left hover:bg-secondary"
      >
        <RobotIcon className="size-5 shrink-0 text-low" />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm text-normal">
            {entry.name}
          </span>
          <span className="mt-0.5 block truncate text-xs text-low">
            {entry.default_executor ?? '本机默认 executor'}
            {' · '}
            {entry.chat_runtime}
            {reviewerName ? ` · 审查者 ${reviewerName}` : ''}
          </span>
        </span>
        {executorMissing && (
          <span className="shrink-0 rounded bg-warning/15 px-1.5 py-0.5 text-[11px] text-warning">
            本机缺少此 executor
          </span>
        )}
        <span className="shrink-0 text-xs text-low">{entry.status}</span>
      </button>
    </li>
  );
}

export function WorkforcePage() {
  const { isSignedIn } = useAuth();

  if (!isSignedIn) {
    return <LoginRequiredPrompt />;
  }

  return <WorkforceInner />;
}
