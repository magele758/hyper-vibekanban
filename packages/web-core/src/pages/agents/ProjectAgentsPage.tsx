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
import type { FeishuBotBinding } from '@/shared/lib/boardAgentsApi';
import { cn } from '@/shared/lib/utils';
import { getRemoteApiUrl } from '@/shared/lib/remoteApi';
import type {
  Autopilot,
  AutopilotRun,
  UpdateAutopilotRequest,
  WebhookEndpoint,
} from 'shared/remote-types';
import { FolderPickerDialog } from '@/shared/dialogs/shared/FolderPickerDialog';
import { AgentModelNameField } from './AgentModelNameField';
import {
  AutopilotScheduleField,
  getLocalTimezone,
} from './AutopilotScheduleField';
import {
  SquadPipelineEditor,
  squadToDraft,
  type SquadEditorDraft,
} from './SquadPipelineEditor';
import { SquadChatCreatePanel } from './SquadChatCreatePanel';

type Tab = 'agents' | 'autopilots' | 'squads' | 'webhooks' | 'feishu';

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
  const [workingDirectory, setWorkingDirectory] = useState('');
  const [defaultCwd, setDefaultCwd] = useState<string | null>(null);
  const [cwdLoading, setCwdLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  useEffect(() => {
    setCwdLoading(true);
    void boardAgentsApi
      .getDefaultCwd()
      .then((cwd) => setDefaultCwd(cwd || null))
      .catch(() => setDefaultCwd(null))
      .finally(() => setCwdLoading(false));
  }, []);

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
        working_directory: workingDirectory.trim() || undefined,
      });
      setCreating(false);
      setName('');
      setInstructions('');
      setChatRuntime('cursor');
      setApiKey('');
      setBaseUrl('');
      setModelName('composer-2.5');
      setWorkingDirectory('');
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
              {chatRuntime === 'cursor'
                ? 'Cursor User API Key'
                : 'API Key（OpenAI 兼容）'}
              <input
                className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                type="password"
                placeholder={
                  chatRuntime === 'cursor'
                    ? 'key_...'
                    : '对应 runtime 的 API Key'
                }
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
              />
            </label>
            <label className="text-xs text-low">
              Base URL
              {chatRuntime !== 'cursor' ? '（Pi/OpenCode 必填）' : '（可选）'}
              <input
                className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                placeholder={
                  chatRuntime === 'cursor'
                    ? '默认留空（官方 Cursor）'
                    : 'https://api.openai.com/v1 或自建网关'
                }
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </label>
            <label className="text-xs text-low">
              Model name
              <AgentModelNameField
                value={modelName}
                onChange={setModelName}
                chatRuntime={chatRuntime}
                apiKey={apiKey}
                baseUrl={baseUrl}
                placeholder={
                  chatRuntime === 'cursor' ? 'composer-2.5' : 'gpt-4.1-mini'
                }
              />
            </label>
            <label className="text-xs text-low">
              工作目录（可选）
              <div className="mt-1 flex gap-2">
                <input
                  className="min-w-0 flex-1 rounded-md border border-border bg-primary px-3 py-2 text-sm"
                  placeholder={
                    defaultCwd
                      ? `留空 → ${defaultCwd}`
                      : cwdLoading
                        ? '正在读取 sidecar 默认目录…'
                        : '留空则用 sidecar 进程目录'
                  }
                  value={workingDirectory}
                  onChange={(e) => setWorkingDirectory(e.target.value)}
                />
                <button
                  type="button"
                  className="shrink-0 rounded-md border border-border px-3 py-2 text-sm text-low hover:bg-primary hover:text-normal"
                  onClick={() => {
                    void FolderPickerDialog.show({
                      value: workingDirectory || defaultCwd || undefined,
                      title: '选择 Agent 工作目录',
                      description:
                        'Cursor SDK 会在此目录读写文件。留空则使用 sidecar 默认目录。',
                    }).then((selected) => {
                      if (selected) setWorkingDirectory(selected);
                    });
                  }}
                >
                  选择…
                </button>
              </div>
              <p className="mt-1 text-[11px] text-low">
                未指定时实际目录：
                {cwdLoading
                  ? '（读取中…）'
                  : (defaultCwd ?? '（无法读取 sidecar 默认目录）')}
              </p>
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
  const { agents, autopilots, squads } = useProjectContext();
  const [runs, setRuns] = useState<Record<string, AutopilotRun[]>>({});
  const [showRuns, setShowRuns] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [agentId, setAgentId] = useState('');
  const [squadId, setSquadId] = useState('');
  const [cron, setCron] = useState('0 9 * * 1-5');
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
    if (!agentId && !squadId) {
      setError('请选择 Agent 或 Squad');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createAutopilot({
        project_id: projectId,
        name: name.trim(),
        agent_id: squadId ? undefined : agentId || undefined,
        squad_id: squadId || undefined,
        cron_expression: cron.trim(),
        timezone: getLocalTimezone(),
        execution_mode: executionMode,
        concurrency_policy: concurrency,
        issue_title_template: titleTemplate.trim(),
        enabled: true,
      });
      setCreating(false);
      setName('');
      setAgentId('');
      setSquadId('');
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
      } as UpdateAutopilotRequest);
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
            绑定 Agent（单 Agent 模式）
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={agentId}
              disabled={!!squadId}
              onChange={(e) => {
                setAgentId(e.target.value);
                if (e.target.value) setSquadId('');
              }}
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
            或绑定 Squad（运行其流水线 / Loop）
            <select
              className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
              value={squadId}
              disabled={!!agentId}
              onChange={(e) => {
                setSquadId(e.target.value);
                if (e.target.value) setAgentId('');
              }}
            >
              <option value="">（无）</option>
              {squads.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          <AutopilotScheduleField value={cron} onChange={setCron} />
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
            const squadName = squads.find((s) => s.id === ap.squad_id)?.name;
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
                      {squadName
                        ? ` · Squad: ${squadName}`
                        : agentName
                          ? ` · Agent: ${agentName}`
                          : ''}
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
                    <p className="mb-2 text-xs font-medium text-low">
                      运行记录
                    </p>
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
  const {
    agents,
    issues,
    squads,
    squadMembers: allSquadMembers,
  } = useProjectContext();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<SquadEditorDraft | null>(null);
  const [chatOpen, setChatOpen] = useState(false);
  const [showMembers, setShowMembers] = useState<string | null>(null);
  const [addingMemberSquadId, setAddingMemberSquadId] = useState<string | null>(
    null
  );
  const [newMemberAgentId, setNewMemberAgentId] = useState('');
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [runMsg, setRunMsg] = useState<string | null>(null);

  const membersFor = useCallback(
    (squadId: string) => allSquadMembers.filter((m) => m.squad_id === squadId),
    [allSquadMembers]
  );

  const openCreate = () => {
    setCreating(true);
    setEditingId(null);
    setChatOpen(false);
    setDraft({
      name: '',
      leader_agent_id: null,
      target_type: 'issue_and_path',
      issue_id: null,
      working_directory: null,
      pipeline: { nodes: [], edges: [] },
      on_assign: 'leader_only',
    });
    setError(null);
    setRunMsg(null);
  };

  const openChatCreate = () => {
    setCreating(true);
    setEditingId(null);
    setChatOpen(true);
    setDraft({
      name: '',
      leader_agent_id: null,
      target_type: 'issue_and_path',
      issue_id: null,
      working_directory: null,
      pipeline: { nodes: [], edges: [] },
      on_assign: 'leader_only',
    });
    setError(null);
    setRunMsg(null);
  };

  const openEdit = (squadId: string) => {
    const squad = squads.find((s) => s.id === squadId);
    if (!squad) return;
    setCreating(false);
    setEditingId(squadId);
    setChatOpen(false);
    setDraft(squadToDraft(squad));
    setError(null);
    setRunMsg(null);
  };

  const closeEditor = () => {
    setCreating(false);
    setEditingId(null);
    setDraft(null);
    setChatOpen(false);
  };

  const handleSave = async () => {
    if (!draft || !draft.name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      if (creating) {
        await boardAgentsApi.createSquad({
          project_id: projectId,
          name: draft.name.trim(),
          leader_agent_id: draft.leader_agent_id ?? undefined,
          target_type: draft.target_type,
          issue_id: draft.issue_id ?? undefined,
          working_directory: draft.working_directory ?? undefined,
          pipeline: draft.pipeline,
          on_assign: draft.on_assign,
        });
      } else if (editingId) {
        await boardAgentsApi.updateSquad(editingId, {
          name: draft.name.trim(),
          leader_agent_id: draft.leader_agent_id,
          target_type: draft.target_type,
          issue_id: draft.issue_id,
          working_directory: draft.working_directory,
          pipeline: draft.pipeline,
          on_assign: draft.on_assign,
        } as Parameters<typeof boardAgentsApi.updateSquad>[1]);
      }
      closeEditor();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleRun = async (squadId: string) => {
    setRunning(true);
    setError(null);
    setRunMsg(null);
    try {
      const squad =
        editingId === squadId && draft
          ? null
          : squads.find((s) => s.id === squadId);
      const pipeline =
        editingId === squadId && draft
          ? draft.pipeline
          : (squad?.pipeline ?? { nodes: [], edges: [] });
      const entryNodes = pipeline.nodes.filter(
        (n) => n.entry_label?.trim() || n.type === 'wait_approval'
      );
      const choices = [
        { id: '', label: '从头（拓扑根）' },
        ...pipeline.nodes.map((n) => ({
          id: n.id,
          label:
            n.entry_label?.trim() ||
            n.label?.trim() ||
            `${n.type ?? 'agent'}:${n.id.slice(0, 8)}`,
        })),
      ];
      // Prefer prompting when there are mid-entry labels; otherwise still allow pick.
      const picked =
        entryNodes.length > 0 || pipeline.nodes.length > 1
          ? window.prompt(
              `从哪一步开始？输入编号 0-${choices.length - 1}\n` +
                choices.map((c, i) => `${i}. ${c.label}`).join('\n'),
              '0'
            )
          : '0';
      if (picked == null) {
        setRunning(false);
        return;
      }
      const idx = Number(picked);
      const startFrom =
        Number.isFinite(idx) && idx > 0 && idx < choices.length
          ? choices[idx].id
          : undefined;

      if (editingId === squadId && draft) {
        await boardAgentsApi.updateSquad(squadId, {
          name: draft.name.trim(),
          leader_agent_id: draft.leader_agent_id,
          target_type: draft.target_type,
          issue_id: draft.issue_id,
          working_directory: draft.working_directory,
          pipeline: draft.pipeline,
          on_assign: draft.on_assign,
        } as Parameters<typeof boardAgentsApi.updateSquad>[1]);
      }
      const result = await boardAgentsApi.runSquad(squadId, {
        issue_id:
          editingId === squadId && draft
            ? (draft.issue_id ?? undefined)
            : (squad?.issue_id ?? undefined),
        working_directory:
          editingId === squadId && draft
            ? (draft.working_directory ?? undefined)
            : (squad?.working_directory ?? undefined),
        start_from_node_id: startFrom || undefined,
      });
      const status = result.status ?? 'completed';
      setRunMsg(
        status === 'waiting_approval'
          ? `已暂停待确认（run ${result.run_id?.slice(0, 8) ?? '?'}…）。请到 Inbox / Issue 批准。`
          : `已完成/入队 ${result.agent_task_ids.length} 个任务（Issue ${result.issue_id.slice(0, 8)}…，目标 ${result.target_type}${
              result.working_directory ? ` @ ${result.working_directory}` : ''
            }${startFrom ? `，从 ${startFrom}` : ''}）`
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  };

  const handleDelete = async (id: string, squadName: string) => {
    if (!window.confirm(`确定删除 Squad「${squadName}」？`)) return;
    try {
      await boardAgentsApi.deleteSquad(id);
      if (editingId === id) closeEditor();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
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

  const targetLabel = (s: (typeof squads)[0]) => {
    const t = s.target_type ?? 'path';
    if (t === 'issue') return 'Issue';
    if (t === 'issue_and_path') return 'Issue+目录';
    return '目录';
  };

  return (
    <div className="p-6">
      <div className="mb-4 flex items-start justify-between gap-4">
        <div className="max-w-2xl space-y-1">
          <p className="text-sm text-low">
            Squad = Agent 团队 +{' '}
            <span className="text-normal">可编辑流水线（DAG）</span>
            。可用「对话创建」用自然语言生成流水线，再在画布微调；也可手动编排后「运行一次」或交给
            Autopilot。
          </p>
        </div>
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <button
            type="button"
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm text-low hover:bg-secondary"
            disabled={busy}
            onClick={() => {
              void (async () => {
                setBusy(true);
                setError(null);
                setRunMsg(null);
                try {
                  const r =
                    await boardAgentsApi.installFeatureCloseout(projectId);
                  setRunMsg(
                    `已安装 Feature Closeout（Squad ${r.squad.name}${
                      r.created_agent_names.length
                        ? `，新建 Agent：${r.created_agent_names.join('、')}`
                        : ''
                    }）。可指派到 Issue，或「从某步运行」。`
                  );
                } catch (e) {
                  setError(e instanceof Error ? e.message : String(e));
                } finally {
                  setBusy(false);
                }
              })();
            }}
          >
            安装 Closeout 模板
          </button>
          <button
            type="button"
            className="inline-flex items-center gap-1.5 rounded-md border border-brand/40 bg-brand/10 px-3 py-1.5 text-sm text-brand hover:bg-brand/15"
            onClick={openChatCreate}
          >
            <ChatCircleIcon className="size-4" />
            对话创建
          </button>
          <PrimaryButton onClick={openCreate}>
            <PlusIcon className="size-4" />
            新建 Squad
          </PrimaryButton>
        </div>
      </div>

      {error && <p className="mb-4 text-sm text-destructive">{error}</p>}
      {runMsg && <p className="mb-4 text-sm text-brand">{runMsg}</p>}

      {draft && (creating || editingId) && (
        <div className="mb-6 space-y-3">
          <div className="flex items-center justify-between gap-2">
            <h2 className="font-medium text-normal">
              {creating ? '创建 Squad' : '编辑流水线'}
            </h2>
            <button
              type="button"
              className="text-xs text-brand hover:underline"
              onClick={() => setChatOpen((v) => !v)}
            >
              {chatOpen ? '收起对话' : '对话改流水线'}
            </button>
          </div>
          {chatOpen && (
            <SquadChatCreatePanel
              projectId={projectId}
              agents={agents}
              issues={issues}
              draft={draft}
              compact={!creating || draft.pipeline.nodes.length > 0}
              onApply={(next) => {
                setDraft(next);
              }}
              onClose={() => setChatOpen(false)}
            />
          )}
          <SquadPipelineEditor
            agents={agents}
            issues={issues}
            draft={draft}
            onChange={setDraft}
            onSave={() => void handleSave()}
            onRun={editingId ? () => void handleRun(editingId) : undefined}
            onCancel={closeEditor}
            busy={busy}
            running={running}
            saveLabel={creating ? '创建' : '保存流水线'}
          />
        </div>
      )}

      {squads.length === 0 && !creating ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <UsersThreeIcon className="size-10" />
          <p className="font-medium text-normal">还没有 Squad</p>
          <p className="max-w-md text-center text-sm">
            用「对话创建」描述工作流即可生成 DAG，或手动新建后在画布编排。配置
            Issue / 目录目标后可「运行一次」或交给 Autopilot。
          </p>
          <button
            type="button"
            className="mt-2 inline-flex items-center gap-1.5 rounded-md border border-brand/40 bg-brand/10 px-3 py-1.5 text-sm text-brand hover:bg-brand/15"
            onClick={openChatCreate}
          >
            <ChatCircleIcon className="size-4" />
            对话创建
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          {squads.map((squad) => {
            const leaderName = agents.find(
              (a) => a.id === squad.leader_agent_id
            )?.name;
            const stepCount = squad.pipeline?.nodes?.length ?? 0;
            const issueTitle = squad.issue_id
              ? issues.find((i) => i.id === squad.issue_id)?.title
              : null;
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
                      <span className="rounded bg-primary px-1.5 py-0.5 text-[10px] text-low">
                        {targetLabel(squad)}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-low">
                      {stepCount} 步
                      {leaderName ? ` · Leader: ${leaderName}` : ''}
                      {issueTitle ? ` · Issue: ${issueTitle}` : ''}
                      {squad.working_directory
                        ? ` · ${squad.working_directory}`
                        : ''}
                    </p>
                  </div>
                  <div className="flex shrink-0 flex-wrap items-center gap-1">
                    <button
                      type="button"
                      className="rounded-md px-2 py-1 text-xs text-brand hover:bg-brand/10"
                      onClick={() => openEdit(squad.id)}
                    >
                      编辑流水线
                    </button>
                    <button
                      type="button"
                      className="rounded-md px-2 py-1 text-xs text-low hover:bg-primary"
                      disabled={running}
                      onClick={() => void handleRun(squad.id)}
                    >
                      <PlayIcon className="mr-0.5 inline size-3" />
                      运行一次
                    </button>
                    <button
                      type="button"
                      className={cn(
                        'rounded-md px-2 py-1 text-xs hover:bg-primary',
                        showMembers === squad.id ? 'text-brand' : 'text-low'
                      )}
                      onClick={() =>
                        setShowMembers((prev) =>
                          prev === squad.id ? null : squad.id
                        )
                      }
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
                        return <p className="text-xs text-low">暂无成员</p>;
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
    const url = `${apiBase}/v1/hooks/${token}`;
    void navigator.clipboard.writeText(url).then(() => {
      setCopied(token);
      setTimeout(() => setCopied(null), 2000);
    });
  };

  const handleRotate = async (id: string) => {
    if (!window.confirm('旋转 Token 会使旧 URL 立即失效，确定继续？')) {
      return;
    }
    setRotating(id);
    setError(null);
    try {
      const updated = await boardAgentsApi.rotateWebhookToken(id);
      setEndpoints((prev) => prev.map((ep) => (ep.id === id ? updated : ep)));
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
            const url = `${apiBase}/v1/hooks/${ep.token}`;
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
                    <p className="mb-1 text-xs text-low">
                      新 Token（请立即复制，刷新后不再显示）：
                    </p>
                    <div className="flex items-center gap-2">
                      <code className="max-w-sm truncate rounded bg-primary px-2 py-1 text-xs font-mono text-normal">
                        {`${apiBase}/v1/hooks/${newToken[ep.id]}`}
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

// ── Feishu tab ───────────────────────────────────────────────────────────────

function FeishuTab({ projectId }: { projectId: string }) {
  const { agents } = useProjectContext();
  const [bindings, setBindings] = useState<FeishuBotBinding[]>([]);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('飞书机器人');
  const [agentId, setAgentId] = useState('');
  const [appId, setAppId] = useState('');
  const [appSecret, setAppSecret] = useState('');
  const [encryptKey, setEncryptKey] = useState('');
  const [verificationToken, setVerificationToken] = useState('');
  const [replyOnComplete, setReplyOnComplete] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const apiBase = getRemoteApiUrl();

  const load = useCallback(async () => {
    try {
      const list = await boardAgentsApi.listFeishuBindings(projectId);
      setBindings(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!agentId && agents.length > 0) {
      setAgentId(agents[0].id);
    }
  }, [agents, agentId]);

  const handleCreate = async () => {
    if (!agentId || !appId.trim() || !appSecret.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createFeishuBinding({
        project_id: projectId,
        agent_id: agentId,
        name: name.trim() || '飞书机器人',
        app_id: appId.trim(),
        app_secret: appSecret,
        encrypt_key: encryptKey.trim() || undefined,
        verification_token: verificationToken.trim() || undefined,
        reply_on_complete: replyOnComplete,
      });
      setCreating(false);
      setAppId('');
      setAppSecret('');
      setEncryptKey('');
      setVerificationToken('');
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string, bindingName: string) => {
    if (!window.confirm(`确定删除飞书绑定「${bindingName}」？`)) return;
    try {
      await boardAgentsApi.deleteFeishuBinding(id);
      setBindings((prev) => prev.filter((b) => b.id !== id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleToggle = async (b: FeishuBotBinding) => {
    try {
      const updated = await boardAgentsApi.updateFeishuBinding(b.id, {
        enabled: !b.enabled,
      });
      setBindings((prev) => prev.map((x) => (x.id === b.id ? updated : x)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCopy = (token: string) => {
    const url = `${apiBase}/v1/feishu/events/${token}`;
    void navigator.clipboard.writeText(url).then(() => {
      setCopied(token);
      setTimeout(() => setCopied(null), 2000);
    });
  };

  const handleRotate = async (id: string) => {
    if (!window.confirm('旋转回调 Token 会使旧 URL 立即失效，确定继续？')) {
      return;
    }
    try {
      const updated = await boardAgentsApi.rotateFeishuCallbackToken(id);
      setBindings((prev) => prev.map((b) => (b.id === id ? updated : b)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const agentName = (id: string) =>
    agents.find((a) => a.id === id)?.name ?? id.slice(0, 8);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between gap-4">
        <p className="text-sm text-low">
          绑定飞书机器人：在飞书里发消息 → 创建 Issue 并入队 Agent
          任务；完成后可回复飞书。
        </p>
        <PrimaryButton
          onClick={() => setCreating(true)}
          disabled={agents.length === 0}
        >
          <PlusIcon className="size-4" />
          绑定飞书
        </PrimaryButton>
      </div>

      {agents.length === 0 && (
        <p className="mb-4 text-sm text-low">
          请先创建一个 Agent，再绑定飞书。
        </p>
      )}

      {error && <p className="mb-4 text-sm text-destructive">{error}</p>}

      {creating && (
        <div className="mb-6 max-w-xl space-y-3 rounded-lg border border-border bg-secondary p-4">
          <h2 className="font-medium text-normal">绑定飞书机器人</h2>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <select
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
            value={agentId}
            onChange={(e) => setAgentId(e.target.value)}
          >
            {agents.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
            placeholder="App ID"
            value={appId}
            onChange={(e) => setAppId(e.target.value)}
          />
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
            placeholder="App Secret"
            type="password"
            value={appSecret}
            onChange={(e) => setAppSecret(e.target.value)}
          />
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
            placeholder="Encrypt Key（可选）"
            value={encryptKey}
            onChange={(e) => setEncryptKey(e.target.value)}
          />
          <input
            className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
            placeholder="Verification Token（可选）"
            value={verificationToken}
            onChange={(e) => setVerificationToken(e.target.value)}
          />
          <label className="flex items-center gap-2 text-sm text-normal">
            <input
              type="checkbox"
              checked={replyOnComplete}
              onChange={(e) => setReplyOnComplete(e.target.checked)}
            />
            任务完成后回复飞书
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

      {bindings.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 py-20 text-low">
          <ChatCircleIcon className="size-10" />
          <p>还没有飞书绑定。</p>
        </div>
      ) : (
        <div className="space-y-3">
          {bindings.map((b) => {
            const url = `${apiBase}/v1/feishu/events/${b.callback_token}`;
            return (
              <div
                key={b.id}
                className="rounded-lg border border-border bg-secondary p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <ChatCircleIcon className="size-4 text-brand" />
                      <span className="font-medium text-normal">{b.name}</span>
                      <span className="text-xs text-low">
                        → {agentName(b.agent_id)}
                      </span>
                      <span
                        className={cn(
                          'rounded-full px-2 py-0.5 text-xs',
                          b.enabled
                            ? 'bg-brand/15 text-normal'
                            : 'bg-secondary text-low'
                        )}
                      >
                        {b.enabled ? '启用' : '停用'}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-low font-mono">
                      App ID: {b.app_id}
                      {b.has_encrypt_key ? ' · Encrypt Key ✓' : ''}
                      {b.has_verification_token ? ' · Token ✓' : ''}
                    </p>
                    <div className="mt-2 flex items-center gap-2">
                      <code className="max-w-md truncate rounded bg-primary px-2 py-1 text-xs font-mono text-low">
                        {url}
                      </code>
                      <button
                        type="button"
                        title="复制回调 URL"
                        className="rounded p-1 text-low hover:text-normal"
                        onClick={() => handleCopy(b.callback_token)}
                      >
                        {copied === b.callback_token ? (
                          <span className="text-xs text-brand">已复制</span>
                        ) : (
                          <CopyIcon className="size-3.5" />
                        )}
                      </button>
                    </div>
                    <p className="mt-2 text-xs text-low">
                      将此 URL 填到飞书开放平台 → 事件订阅 → 请求地址；订阅{' '}
                      <code className="font-mono">im.message.receive_v1</code>
                      。群聊需 @机器人。
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      title={b.enabled ? '停用' : '启用'}
                      className="rounded-md px-2 py-1 text-xs text-low hover:bg-primary"
                      onClick={() => void handleToggle(b)}
                    >
                      {b.enabled ? '停用' : '启用'}
                    </button>
                    <button
                      type="button"
                      title="旋转 Token"
                      className="rounded-md p-1.5 text-low hover:bg-warning/10 hover:text-warning"
                      onClick={() => void handleRotate(b.id)}
                    >
                      <ArrowsClockwiseIcon className="size-4" />
                    </button>
                    <button
                      type="button"
                      title="删除"
                      className="rounded-md p-1.5 text-low hover:bg-destructive/10 hover:text-destructive"
                      onClick={() => void handleDelete(b.id, b.name)}
                    >
                      <TrashIcon className="size-4" />
                    </button>
                  </div>
                </div>
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
  { id: 'feishu', label: '飞书' },
];

function AgentsPageInner() {
  const { projectId } = useParams({ strict: false }) as { projectId: string };
  const [tab, setTab] = useState<Tab>('agents');

  return (
    <div className="flex h-full min-h-0 flex-col bg-primary">
      <div className="flex flex-col gap-3 border-b border-border px-4 py-3 sm:flex-row sm:items-center sm:gap-4 sm:px-6 sm:py-4">
        <div>
          <h1 className="text-lg font-semibold text-normal">Agents</h1>
        </div>
        <nav className="-mx-1 flex gap-1 overflow-x-auto px-1 sm:ml-4">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => setTab(t.id)}
              className={cn(
                'shrink-0 rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
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
        {tab === 'feishu' && <FeishuTab projectId={projectId} />}
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
