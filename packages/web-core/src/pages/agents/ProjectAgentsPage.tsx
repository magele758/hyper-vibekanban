import { useCallback, useEffect, useMemo, useState } from 'react';
import { useParams, useNavigate } from '@tanstack/react-router';
import {
  PlusIcon,
  RobotIcon,
  ChatCircleIcon,
  TrashIcon,
  ClockIcon,
  UsersThreeIcon,
  WebhooksLogoIcon,
  PlayIcon,
  CopyIcon,
  ArrowsClockwiseIcon,
} from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { cn } from '@/shared/lib/utils';
import { getRemoteApiUrl } from '@/shared/lib/remoteApi';
import type {
  Autopilot,
  AutopilotRun,
  WebhookEndpoint,
} from 'shared/remote-types';

type Tab = 'agents' | 'autopilots' | 'squads' | 'webhooks';

// ── Agents tab ───────────────────────────────────────────────────────────────

function AgentsTab({ projectId }: { projectId: string }) {
  const navigate = useNavigate();
  const { agents, removeAgent } = useProjectContext();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [instructions, setInstructions] = useState('');
  const [chatRuntime, setChatRuntime] = useState<'cursor' | 'pi' | 'opencode'>(
    'cursor'
  );
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [modelName, setModelName] = useState('composer-2.5');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const sorted = useMemo(
    () => [...agents].sort((a, b) => a.name.localeCompare(b.name)),
    [agents]
  );

  const handleCreate = async () => {
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const agent = await boardAgentsApi.createAgent({
        project_id: projectId,
        name: name.trim(),
        instructions: instructions.trim(),
        default_executor: null,
        max_concurrent_tasks: 1,
        chat_runtime: chatRuntime,
        api_key: apiKey.trim() || undefined,
        base_url: baseUrl.trim() || undefined,
        model_name: modelName.trim() || undefined,
      });
      setCreating(false);
      setName('');
      setInstructions('');
      setChatRuntime('cursor');
      setApiKey('');
      setBaseUrl('');
      setModelName('composer-2.5');
      void navigate({
        to: '/projects/$projectId/agents/$agentId',
        params: { projectId, agentId: agent.id },
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (agentId: string, agentName: string) => {
    if (
      !window.confirm(
        `确定删除 Agent「${agentName}」？相关会话与 LLM 设置也会一并移除。`
      )
    ) {
      return;
    }
    setDeletingId(agentId);
    setError(null);
    try {
      await removeAgent(agentId).persisted;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-low">
          看板 Agent：可对话、可指派执行。对话 runtime 默认 Cursor SDK。
        </p>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="rounded-md px-3 py-1.5 text-sm text-low hover:text-normal"
            onClick={() =>
              void navigate({
                to: '/projects/$projectId/copilot',
                params: { projectId },
              })
            }
          >
            项目 Copilot
          </button>
          <PrimaryButton onClick={() => setCreating(true)}>
            <PlusIcon className="size-4" />
            新建 Agent
          </PrimaryButton>
        </div>
      </div>

      {creating && (
        <div className="mb-6 max-w-xl space-y-3 rounded-lg border border-border bg-secondary p-4">
          <h2 className="font-medium text-normal">创建 Agent</h2>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <textarea
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="Instructions / 系统提示"
            rows={3}
            value={instructions}
            onChange={(e) => setInstructions(e.target.value)}
          />
          <label className="block text-xs text-low">
            对话 Runtime
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={chatRuntime}
              onChange={(e) =>
                setChatRuntime(e.target.value as 'cursor' | 'pi' | 'opencode')
              }
            >
              <option value="cursor">Cursor SDK（默认，已接入）</option>
              <option value="pi">Pi（规划中）</option>
              <option value="opencode">OpenCode（规划中）</option>
            </select>
          </label>
          <div className="grid gap-2 sm:grid-cols-1">
            <label className="text-xs text-low">
              API Key
              <input
                className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                type="password"
                placeholder="key_..."
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                disabled={chatRuntime !== 'cursor'}
              />
            </label>
            <label className="text-xs text-low">
              Base URL（可选）
              <input
                className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                placeholder="默认留空"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </label>
            <label className="text-xs text-low">
              Model name
              <input
                className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                placeholder="composer-2.5"
                value={modelName}
                onChange={(e) => setModelName(e.target.value)}
              />
            </label>
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <div className="flex gap-2">
            <PrimaryButton disabled={busy} onClick={() => void handleCreate()}>
              {busy ? '创建中…' : '创建'}
            </PrimaryButton>
            <button
              type="button"
              className="rounded-md px-3 py-1.5 text-sm text-low"
              onClick={() => setCreating(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {error && !creating && (
        <p className="mb-4 text-sm text-destructive">{error}</p>
      )}

      {sorted.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <RobotIcon className="size-10" />
          <p>还没有 Agent。创建一个开始对话。</p>
        </div>
      ) : (
        <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {sorted.map((agent) => (
            <li key={agent.id} className="group relative">
              <button
                type="button"
                className={cn(
                  'flex w-full flex-col items-start gap-2 rounded-lg border border-border',
                  'bg-secondary p-4 pr-12 text-left hover:border-brand'
                )}
                onClick={() =>
                  void navigate({
                    to: '/projects/$projectId/agents/$agentId',
                    params: { projectId, agentId: agent.id },
                  })
                }
              >
                <div className="flex items-center gap-2">
                  <RobotIcon className="size-5 text-brand" />
                  <span className="font-medium text-normal">{agent.name}</span>
                </div>
                <p className="line-clamp-2 text-xs text-low">
                  {agent.instructions || '无 instructions'}
                </p>
                <span className="inline-flex items-center gap-1 text-xs text-low">
                  <ChatCircleIcon className="size-3.5" />
                  {agent.status} · {agent.chat_runtime ?? 'cursor'} · 点击进入
                </span>
              </button>
              <button
                type="button"
                title="删除 Agent"
                disabled={deletingId === agent.id}
                className={cn(
                  'absolute right-2 top-2 rounded-md p-1.5 text-low',
                  'hover:bg-destructive/10 hover:text-destructive',
                  'opacity-70 group-hover:opacity-100 disabled:opacity-40'
                )}
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  void handleDelete(agent.id, agent.name);
                }}
              >
                <TrashIcon className="size-4" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ── Autopilots tab ───────────────────────────────────────────────────────────

function AutopilotsTab({ projectId }: { projectId: string }) {
  const { agents, autopilots } = useProjectContext();
  const [runs, setRuns] = useState<Record<string, AutopilotRun[]>>({});
  const [showRuns, setShowRuns] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [agentId, setAgentId] = useState('');
  const [cron, setCron] = useState('0 9 * * 1-5');
  const [timezone, setTimezone] = useState('UTC');
  const [executionMode, setExecutionMode] = useState<
    'create_issue' | 'run_only'
  >('create_issue');
  const [concurrency, setConcurrency] = useState<'skip' | 'queue'>('skip');
  const [titleTemplate, setTitleTemplate] = useState(
    '{{autopilot_name}} - {{date}}'
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createAutopilot({
        project_id: projectId,
        name: name.trim(),
        agent_id: agentId || null,
        cron_expression: cron.trim(),
        timezone: timezone.trim(),
        execution_mode: executionMode,
        concurrency_policy: concurrency,
        issue_title_template: titleTemplate.trim(),
        enabled: true,
      });
      setCreating(false);
      setName('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (ap: Autopilot) => {
    try {
      await boardAgentsApi.updateAutopilot(ap.id, {
        enabled: !ap.enabled,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleTrigger = async (id: string) => {
    try {
      await boardAgentsApi.triggerAutopilot(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (id: string, apName: string) => {
    if (!window.confirm(`确定删除 Autopilot「${apName}」？`)) return;
    try {
      await boardAgentsApi.deleteAutopilot(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleShowRuns = async (id: string) => {
    if (showRuns === id) {
      setShowRuns(null);
      return;
    }
    setShowRuns(id);
    if (!runs[id]) {
      try {
        const list = await boardAgentsApi.listAutopilotRuns(id);
        setRuns((prev) => ({ ...prev, [id]: list }));
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    }
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-low">
          定时任务：按 Cron 表达式自动创建 Issue 或触发 Agent 执行。
        </p>
        <PrimaryButton onClick={() => setCreating(true)}>
          <PlusIcon className="size-4" />
          新建 Autopilot
        </PrimaryButton>
      </div>

      {error && <p className="mb-4 text-sm text-destructive">{error}</p>}

      {creating && (
        <div className="mb-6 max-w-xl space-y-3 rounded-lg border border-border bg-secondary p-4">
          <h2 className="font-medium text-normal">创建 Autopilot</h2>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <label className="block text-xs text-low">
            绑定 Agent（可选）
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={agentId}
              onChange={(e) => setAgentId(e.target.value)}
            >
              <option value="">（无）</option>
              {agents.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block text-xs text-low">
            Cron 表达式
            <input
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
              placeholder="0 9 * * 1-5"
              value={cron}
              onChange={(e) => setCron(e.target.value)}
            />
          </label>
          <label className="block text-xs text-low">
            时区
            <input
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              placeholder="UTC"
              value={timezone}
              onChange={(e) => setTimezone(e.target.value)}
            />
          </label>
          <label className="block text-xs text-low">
            执行模式
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={executionMode}
              onChange={(e) =>
                setExecutionMode(e.target.value as 'create_issue' | 'run_only')
              }
            >
              <option value="create_issue">创建 Issue（create_issue）</option>
              <option value="run_only">仅运行（run_only）</option>
            </select>
          </label>
          <label className="block text-xs text-low">
            并发策略
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={concurrency}
              onChange={(e) =>
                setConcurrency(e.target.value as 'skip' | 'queue')
              }
            >
              <option value="skip">跳过（skip）</option>
              <option value="queue">排队（queue）</option>
            </select>
          </label>
          <label className="block text-xs text-low">
            Issue 标题模板
            <input
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              placeholder={'{{autopilot_name}} - {{date}}'}
              value={titleTemplate}
              onChange={(e) => setTitleTemplate(e.target.value)}
            />
          </label>
          <div className="flex gap-2">
            <PrimaryButton disabled={busy} onClick={() => void handleCreate()}>
              {busy ? '创建中…' : '创建'}
            </PrimaryButton>
            <button
              type="button"
              className="rounded-md px-3 py-1.5 text-sm text-low"
              onClick={() => setCreating(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {autopilots.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <ClockIcon className="size-10" />
          <p>还没有 Autopilot。</p>
        </div>
      ) : (
        <div className="space-y-3">
          {autopilots.map((ap) => {
            const agentName = agents.find((a) => a.id === ap.agent_id)?.name;
            return (
              <div
                key={ap.id}
                className="rounded-lg border border-border bg-secondary p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-normal">{ap.name}</span>
                      <span
                        className={cn(
                          'rounded-full px-2 py-0.5 text-xs',
                          ap.enabled
                            ? 'bg-brand/15 text-normal'
                            : 'bg-secondary text-low'
                        )}
                      >
                        {ap.enabled ? '启用' : '停用'}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-low font-mono">
                      {ap.cron_expression} · {ap.timezone}
                    </p>
                    <p className="text-xs text-low">
                      {ap.execution_mode} · {ap.concurrency_policy}
                      {agentName ? ` · Agent: ${agentName}` : ''}
                    </p>
                    {ap.next_run_at && (
                      <p className="text-xs text-low">
                        下次运行：
                        {new Date(ap.next_run_at).toLocaleString()}
                      </p>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      title="立即触发"
                      className="rounded-md p-1.5 text-low hover:bg-brand/10 hover:text-brand"
                      onClick={() => void handleTrigger(ap.id)}
                    >
                      <PlayIcon className="size-4" />
                    </button>
                    <button
                      type="button"
                      title={ap.enabled ? '停用' : '启用'}
                      className="rounded-md px-2 py-1 text-xs text-low hover:bg-primary"
                      onClick={() => void handleToggle(ap)}
                    >
                      {ap.enabled ? '停用' : '启用'}
                    </button>
                    <button
                      type="button"
                      title="查看运行记录"
                      className={cn(
                        'rounded-md px-2 py-1 text-xs hover:bg-primary',
                        showRuns === ap.id ? 'text-brand' : 'text-low'
                      )}
                      onClick={() => void handleShowRuns(ap.id)}
                    >
                      记录
                    </button>
                    <button
                      type="button"
                      title="删除"
                      className="rounded-md p-1.5 text-low hover:bg-destructive/10 hover:text-destructive"
                      onClick={() => void handleDelete(ap.id, ap.name)}
                    >
                      <TrashIcon className="size-4" />
                    </button>
                  </div>
                </div>

                {showRuns === ap.id && (
                  <div className="mt-3 border-t border-border pt-3">
                    <p className="mb-2 text-xs font-medium text-low">运行记录</p>
                    {!runs[ap.id] ? (
                      <p className="text-xs text-low">加载中…</p>
                    ) : runs[ap.id].length === 0 ? (
                      <p className="text-xs text-low">暂无记录</p>
                    ) : (
                      <ul className="space-y-1">
                        {runs[ap.id].slice(0, 10).map((run) => (
                          <li
                            key={run.id}
                            className="flex items-center gap-2 text-xs"
                          >
                            <span
                              className={cn(
                                'rounded px-1.5 py-0.5 text-[10px]',
                                run.status === 'completed'
                                  ? 'bg-brand/15 text-normal'
                                  : run.status === 'failed'
                                    ? 'bg-destructive/15 text-destructive'
                                    : run.status === 'running'
                                      ? 'bg-warning/15 text-warning'
                                      : 'bg-secondary text-low'
                              )}
                            >
                              {run.status}
                            </span>
                            <span className="text-low">
                              {new Date(run.planned_at).toLocaleString()}
                            </span>
                            {run.error_message && (
                              <span className="truncate text-destructive">
                                {run.error_message}
                              </span>
                            )}
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ── Squads tab ───────────────────────────────────────────────────────────────

function SquadsTab({ projectId }: { projectId: string }) {
  const { agents, squads, squadMembers: allSquadMembers } = useProjectContext();
  const [showMembers, setShowMembers] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [leaderId, setLeaderId] = useState('');
  const [addingMemberSquadId, setAddingMemberSquadId] = useState<string | null>(
    null
  );
  const [newMemberAgentId, setNewMemberAgentId] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const membersFor = useCallback(
    (squadId: string) =>
      allSquadMembers.filter((m) => m.squad_id === squadId),
    [allSquadMembers]
  );

  const handleCreate = async () => {
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createSquad({
        project_id: projectId,
        name: name.trim(),
        leader_agent_id: leaderId || null,
      });
      setCreating(false);
      setName('');
      setLeaderId('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string, squadName: string) => {
    if (!window.confirm(`确定删除 Squad「${squadName}」？`)) return;
    try {
      await boardAgentsApi.deleteSquad(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleShowMembers = (id: string) => {
    setShowMembers((prev) => (prev === id ? null : id));
  };

  const handleAddMember = async (squadId: string) => {
    if (!newMemberAgentId) return;
    try {
      await boardAgentsApi.addSquadMember(squadId, {
        agent_id: newMemberAgentId,
      });
      setNewMemberAgentId('');
      setAddingMemberSquadId(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRemoveMember = async (squadId: string, memberId: string) => {
    try {
      await boardAgentsApi.removeSquadMember(squadId, memberId);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-low">
          Squad：Agent 团队，支持 Leader + 成员协作执行任务。
        </p>
        <PrimaryButton onClick={() => setCreating(true)}>
          <PlusIcon className="size-4" />
          新建 Squad
        </PrimaryButton>
      </div>

      {error && <p className="mb-4 text-sm text-destructive">{error}</p>}

      {creating && (
        <div className="mb-6 max-w-xl space-y-3 rounded-lg border border-border bg-secondary p-4">
          <h2 className="font-medium text-normal">创建 Squad</h2>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <label className="block text-xs text-low">
            Leader Agent（可选）
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={leaderId}
              onChange={(e) => setLeaderId(e.target.value)}
            >
              <option value="">（无）</option>
              {agents.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <div className="flex gap-2">
            <PrimaryButton disabled={busy} onClick={() => void handleCreate()}>
              {busy ? '创建中…' : '创建'}
            </PrimaryButton>
            <button
              type="button"
              className="rounded-md px-3 py-1.5 text-sm text-low"
              onClick={() => setCreating(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {squads.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <UsersThreeIcon className="size-10" />
          <p>还没有 Squad。</p>
        </div>
      ) : (
        <div className="space-y-3">
          {squads.map((squad) => {
            const leaderName = agents.find(
              (a) => a.id === squad.leader_agent_id
            )?.name;
            return (
              <div
                key={squad.id}
                className="rounded-lg border border-border bg-secondary p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <UsersThreeIcon className="size-4 text-brand" />
                      <span className="font-medium text-normal">
                        {squad.name}
                      </span>
                    </div>
                    {leaderName && (
                      <p className="mt-1 text-xs text-low">
                        Leader: {leaderName}
                      </p>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      className={cn(
                        'rounded-md px-2 py-1 text-xs hover:bg-primary',
                        showMembers === squad.id ? 'text-brand' : 'text-low'
                      )}
                      onClick={() => void handleShowMembers(squad.id)}
                    >
                      成员
                    </button>
                    <button
                      type="button"
                      className="rounded-md p-1.5 text-low hover:bg-destructive/10 hover:text-destructive"
                      onClick={() => void handleDelete(squad.id, squad.name)}
                    >
                      <TrashIcon className="size-4" />
                    </button>
                  </div>
                </div>

                {showMembers === squad.id && (
                  <div className="mt-3 border-t border-border pt-3">
                    <div className="mb-2 flex items-center justify-between">
                      <p className="text-xs font-medium text-low">成员列表</p>
                      <button
                        type="button"
                        className="text-xs text-brand hover:underline"
                        onClick={() => setAddingMemberSquadId(squad.id)}
                      >
                        + 添加
                      </button>
                    </div>
                    {addingMemberSquadId === squad.id && (
                      <div className="mb-2 flex gap-2">
                        <select
                          className="flex-1 rounded-md border border-border bg-primary px-2 py-1 text-xs"
                          value={newMemberAgentId}
                          onChange={(e) => setNewMemberAgentId(e.target.value)}
                        >
                          <option value="">选择 Agent</option>
                          {agents.map((a) => (
                            <option key={a.id} value={a.id}>
                              {a.name}
                            </option>
                          ))}
                        </select>
                        <button
                          type="button"
                          className="rounded-md bg-brand px-2 py-1 text-xs text-on-brand"
                          onClick={() => void handleAddMember(squad.id)}
                        >
                          添加
                        </button>
                        <button
                          type="button"
                          className="rounded-md px-2 py-1 text-xs text-low"
                          onClick={() => setAddingMemberSquadId(null)}
                        >
                          取消
                        </button>
                      </div>
                    )}
                    {(() => {
                      const members = membersFor(squad.id);
                      if (members.length === 0) {
                        return (
                          <p className="text-xs text-low">暂无成员</p>
                        );
                      }
                      return (
                      <ul className="space-y-1">
                        {members.map((m) => {
                          const agentName = agents.find(
                            (a) => a.id === m.agent_id
                          )?.name;
                          return (
                            <li
                              key={m.id}
                              className="flex items-center justify-between text-xs"
                            >
                              <span className="text-normal">
                                {agentName ?? m.agent_id ?? m.user_id ?? m.id}
                              </span>
                              <button
                                type="button"
                                className="text-low hover:text-destructive"
                                onClick={() =>
                                  void handleRemoveMember(squad.id, m.id)
                                }
                              >
                                移除
                              </button>
                            </li>
                          );
                        })}
                      </ul>
                      );
                    })()}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ── Webhooks tab ─────────────────────────────────────────────────────────────

function WebhooksTab({ projectId }: { projectId: string }) {
  const [endpoints, setEndpoints] = useState<WebhookEndpoint[]>([]);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [rotating, setRotating] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<Record<string, string>>({});

  const apiBase = getRemoteApiUrl();

  const load = useCallback(async () => {
    try {
      const list = await boardAgentsApi.listWebhookEndpoints(projectId);
      setEndpoints(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleCreate = async () => {
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createWebhookEndpoint({
        project_id: projectId,
        name: name.trim(),
      });
      setCreating(false);
      setName('');
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string, epName: string) => {
    if (!window.confirm(`确定删除 Webhook「${epName}」？`)) return;
    try {
      await boardAgentsApi.deleteWebhookEndpoint(id);
      setEndpoints((prev) => prev.filter((ep) => ep.id !== id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCopy = (token: string) => {
    const url = `${apiBase}/v1/webhooks/${token}`;
    void navigator.clipboard.writeText(url).then(() => {
      setCopied(token);
      setTimeout(() => setCopied(null), 2000);
    });
  };

  const handleRotate = async (id: string) => {
    if (
      !window.confirm(
        '旋转 Token 会使旧 URL 立即失效，确定继续？'
      )
    ) {
      return;
    }
    setRotating(id);
    setError(null);
    try {
      const updated = await boardAgentsApi.rotateWebhookToken(id);
      setEndpoints((prev) =>
        prev.map((ep) => (ep.id === id ? updated : ep))
      );
      setNewToken((prev) => ({ ...prev, [id]: updated.token }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRotating(null);
    }
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-low">
          Webhook：外部系统通过 POST 触发 Autopilot 执行。
        </p>
        <PrimaryButton onClick={() => setCreating(true)}>
          <PlusIcon className="size-4" />
          新建 Webhook
        </PrimaryButton>
      </div>

      {error && <p className="mb-4 text-sm text-destructive">{error}</p>}

      {creating && (
        <div className="mb-6 max-w-xl space-y-3 rounded-lg border border-border bg-secondary p-4">
          <h2 className="font-medium text-normal">创建 Webhook</h2>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <div className="flex gap-2">
            <PrimaryButton disabled={busy} onClick={() => void handleCreate()}>
              {busy ? '创建中…' : '创建'}
            </PrimaryButton>
            <button
              type="button"
              className="rounded-md px-3 py-1.5 text-sm text-low"
              onClick={() => setCreating(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {endpoints.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <WebhooksLogoIcon className="size-10" />
          <p>还没有 Webhook。</p>
        </div>
      ) : (
        <div className="space-y-3">
          {endpoints.map((ep) => {
            const url = `${apiBase}/v1/webhooks/${ep.token}`;
            return (
              <div
                key={ep.id}
                className="rounded-lg border border-border bg-secondary p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <WebhooksLogoIcon className="size-4 text-brand" />
                      <span className="font-medium text-normal">{ep.name}</span>
                      <span
                        className={cn(
                          'rounded-full px-2 py-0.5 text-xs',
                          ep.enabled
                            ? 'bg-brand/15 text-normal'
                            : 'bg-secondary text-low'
                        )}
                      >
                        {ep.enabled ? '启用' : '停用'}
                      </span>
                    </div>
                    <div className="mt-2 flex items-center gap-2">
                      <code className="max-w-sm truncate rounded bg-primary px-2 py-1 text-xs font-mono text-low">
                        {url}
                      </code>
                      <button
                        type="button"
                        title="复制 URL"
                        className="rounded p-1 text-low hover:text-normal"
                        onClick={() => handleCopy(ep.token)}
                      >
                        {copied === ep.token ? (
                          <span className="text-xs text-brand">已复制</span>
                        ) : (
                          <CopyIcon className="size-3.5" />
                        )}
                      </button>
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      title="旋转 Token"
                      disabled={rotating === ep.id}
                      className="rounded-md p-1.5 text-low hover:bg-warning/10 hover:text-warning disabled:opacity-40"
                      onClick={() => void handleRotate(ep.id)}
                    >
                      <ArrowsClockwiseIcon className="size-4" />
                    </button>
                    <button
                      type="button"
                      title="删除"
                      className="rounded-md p-1.5 text-low hover:bg-destructive/10 hover:text-destructive"
                      onClick={() => void handleDelete(ep.id, ep.name)}
                    >
                      <TrashIcon className="size-4" />
                    </button>
                  </div>
                </div>
                {newToken[ep.id] && (
                  <div className="mt-2 border-t border-border pt-2">
                    <p className="mb-1 text-xs text-low">新 Token（请立即复制，刷新后不再显示）：</p>
                    <div className="flex items-center gap-2">
                      <code className="max-w-sm truncate rounded bg-primary px-2 py-1 text-xs font-mono text-normal">
                        {`${apiBase}/v1/webhooks/${newToken[ep.id]}`}
                      </code>
                      <button
                        type="button"
                        className="rounded p-1 text-low hover:text-normal"
                        onClick={() => handleCopy(newToken[ep.id])}
                      >
                        {copied === newToken[ep.id] ? (
                          <span className="text-xs text-brand">已复制</span>
                        ) : (
                          <CopyIcon className="size-3.5" />
                        )}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ── Page shell ───────────────────────────────────────────────────────────────

const TABS: { id: Tab; label: string }[] = [
  { id: 'agents', label: 'Agents' },
  { id: 'autopilots', label: 'Autopilots' },
  { id: 'squads', label: 'Squads' },
  { id: 'webhooks', label: 'Webhooks' },
];

function AgentsPageInner() {
  const { projectId } = useParams({ strict: false }) as { projectId: string };
  const [tab, setTab] = useState<Tab>('agents');

  return (
    <div className="flex h-full min-h-0 flex-col bg-primary">
      <div className="flex items-center gap-4 border-b border-border px-6 py-4">
        <div>
          <h1 className="text-lg font-semibold text-normal">Agents</h1>
        </div>
        <nav className="ml-4 flex gap-1">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => setTab(t.id)}
              className={cn(
                'rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
                tab === t.id
                  ? 'bg-brand/15 text-normal'
                  : 'text-low hover:bg-primary hover:text-normal'
              )}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {tab === 'agents' && <AgentsTab projectId={projectId} />}
        {tab === 'autopilots' && <AutopilotsTab projectId={projectId} />}
        {tab === 'squads' && <SquadsTab projectId={projectId} />}
        {tab === 'webhooks' && <WebhooksTab projectId={projectId} />}
      </div>
    </div>
  );
}

export function ProjectAgentsPage() {
  const { isSignedIn } = useAuth();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const { projectId } = useParams({ strict: false }) as { projectId: string };

  if (!isSignedIn) {
    return <LoginRequiredPrompt />;
  }
  if (!selectedOrgId || !projectId) {
    return null;
  }

  return (
    <OrgProvider organizationId={selectedOrgId}>
      <ProjectProvider projectId={projectId}>
        <AgentsPageInner />
      </ProjectProvider>
    </OrgProvider>
  );
}
