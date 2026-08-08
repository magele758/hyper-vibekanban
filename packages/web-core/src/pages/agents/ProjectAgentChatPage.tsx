import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate } from '@tanstack/react-router';
import {
  ArrowLeftIcon,
  CaretDownIcon,
  CaretRightIcon,
  PlusIcon,
  PaperPlaneTiltIcon,
  SpinnerIcon,
} from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Tooltip } from '@vibe/ui/components/Tooltip';
import {
  boardAgentsApi,
  type AgentLlmSettings,
  type CopilotMessage,
  type CopilotSession,
} from '@/shared/lib/boardAgentsApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import { cn } from '@/shared/lib/utils';
import { FolderPickerDialog } from '@/shared/dialogs/shared/FolderPickerDialog';
import { AgentModelNameField } from './AgentModelNameField';

function AgentChatInner({
  projectId,
  agentId,
}: {
  projectId: string;
  agentId: string | null;
}) {
  const navigate = useNavigate();
  const { agents, updateAgent, agentTasks, issues } = useProjectContext();
  const agent = useMemo(
    () => (agentId ? agents.find((a) => a.id === agentId) : null),
    [agents, agentId]
  );

  const recentTasks = useMemo(() => {
    if (!agentId) return [];
    return agentTasks
      .filter((t) => t.agent_id === agentId)
      .slice()
      .sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      )
      .slice(0, 8);
  }, [agentTasks, agentId]);

  const issuesById = useMemo(() => {
    const map = new Map(issues.map((i) => [i.id, i]));
    return map;
  }, [issues]);

  const [sessions, setSessions] = useState<CopilotSession[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<CopilotMessage[]>([]);
  const [input, setInput] = useState('');
  const [streaming, setStreaming] = useState('');
  const [toolStatus, setToolStatus] = useState<{
    name: string;
    done: boolean;
    ok: boolean;
  } | null>(null);
  /** True only while a chat turn is in flight (not config save). */
  const [chatBusy, setChatBusy] = useState(false);
  const [configBusy, setConfigBusy] = useState(false);
  const busy = chatBusy || configBusy;
  const [error, setError] = useState<string | null>(null);
  const [justFinished, setJustFinished] = useState(false);
  const [turnMeta, setTurnMeta] = useState<{
    transport?: string;
    history_turns?: number;
  } | null>(null);
  const [llm, setLlm] = useState<AgentLlmSettings | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [modelName, setModelName] = useState('');
  const [workingDirectory, setWorkingDirectory] = useState('');
  const [defaultCwd, setDefaultCwd] = useState<string | null>(null);
  const [lastEffectiveCwd, setLastEffectiveCwd] = useState<{
    cwd: string;
    source: 'request' | 'saved' | 'default';
  } | null>(null);
  const [instructions, setInstructions] = useState('');
  const [chatRuntime, setChatRuntime] = useState<'cursor' | 'pi' | 'opencode'>(
    'cursor'
  );
  const [agentConfigOpen, setAgentConfigOpen] = useState(false);
  const [cwdLoading, setCwdLoading] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);

  const title = agent?.name ?? '项目 Copilot';

  const refreshSessions = useCallback(async () => {
    const list = await boardAgentsApi.listSessions({
      project_id: projectId,
      agent_id: agentId ?? undefined,
      project_copilot: !agentId,
    });
    setSessions(list);
    if (!sessionId && list[0]) {
      setSessionId(list[0].id);
    }
  }, [projectId, agentId, sessionId]);

  useEffect(() => {
    void refreshSessions().catch((e) =>
      setError(e instanceof Error ? e.message : String(e))
    );
  }, [projectId, agentId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    setCwdLoading(true);
    void boardAgentsApi
      .getDefaultCwd()
      .then((cwd) => setDefaultCwd(cwd || null))
      .catch(() => setDefaultCwd(null))
      .finally(() => setCwdLoading(false));
  }, []);

  useEffect(() => {
    if (!agentId) return;
    void boardAgentsApi
      .getLlmSettings(agentId)
      .then((s) => {
        setLlm(s);
        setBaseUrl(s.base_url ?? '');
        setModelName(s.model_name ?? 'composer-2.5');
        setWorkingDirectory(s.working_directory ?? '');
        // Expand credentials when key is missing so first-time setup is obvious.
        if (!s.has_api_key) setAgentConfigOpen(true);
      })
      .catch(() => undefined);
    if (agent) {
      setInstructions(agent.instructions);
      setChatRuntime(agent.chat_runtime ?? 'cursor');
    }
  }, [agentId, agent]);

  useEffect(() => {
    if (!sessionId) {
      setMessages([]);
      return;
    }
    void boardAgentsApi
      .listMessages(sessionId)
      .then(setMessages)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [sessionId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streaming, chatBusy, toolStatus, justFinished]);

  useEffect(() => {
    if (!justFinished) return;
    const t = window.setTimeout(() => setJustFinished(false), 1800);
    return () => window.clearTimeout(t);
  }, [justFinished]);

  const handleNewSession = async () => {
    const session = await boardAgentsApi.createSession({
      project_id: projectId,
      agent_id: agentId,
      title: `会话 ${new Date().toLocaleString()}`,
    });
    setSessions((prev) => [session, ...prev]);
    setSessionId(session.id);
    setMessages([]);
  };

  const handleSaveAgentConfig = async () => {
    if (!agentId) return;
    setConfigBusy(true);
    setError(null);
    try {
      const patch: {
        instructions?: string;
        chat_runtime?: 'cursor' | 'pi' | 'opencode';
      } = {};
      if (agent && instructions !== agent.instructions) {
        patch.instructions = instructions;
      }
      if (agent && chatRuntime !== (agent.chat_runtime ?? 'cursor')) {
        patch.chat_runtime = chatRuntime;
      }
      if (Object.keys(patch).length > 0) {
        await updateAgent(agentId, patch).persisted;
      }
      const saved = await boardAgentsApi.upsertLlmSettings(agentId, {
        api_key: apiKey.trim() || undefined,
        base_url: baseUrl.trim(),
      });
      setLlm(saved);
      setBaseUrl(saved.base_url ?? '');
      setApiKey('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConfigBusy(false);
    }
  };

  const handleSaveModelAndCwd = async () => {
    if (!agentId) return;
    setConfigBusy(true);
    setError(null);
    try {
      const saved = await boardAgentsApi.upsertLlmSettings(agentId, {
        model_name: modelName.trim(),
        working_directory: workingDirectory.trim(),
      });
      setLlm(saved);
      setModelName(saved.model_name ?? modelName.trim());
      setWorkingDirectory(saved.working_directory ?? '');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConfigBusy(false);
    }
  };

  const handlePickDirectory = async () => {
    const selected = await FolderPickerDialog.show({
      value: workingDirectory || defaultCwd || undefined,
      title: '选择 Agent 工作目录',
      description:
        'Cursor SDK 会在此目录读写文件。留空则使用 sidecar 进程当前目录。',
    });
    if (selected) setWorkingDirectory(selected);
  };

  const previewCwd =
    workingDirectory.trim() ||
    lastEffectiveCwd?.cwd ||
    defaultCwd ||
    (cwdLoading ? '（读取中…）' : '（无法读取 sidecar 默认目录）');
  const previewSource = workingDirectory.trim()
    ? '已指定'
    : lastEffectiveCwd?.source === 'saved'
      ? '已保存'
      : lastEffectiveCwd?.source === 'default'
        ? 'sidecar 默认'
        : defaultCwd
          ? 'sidecar 默认（未对话前预览）'
          : cwdLoading
            ? '读取中'
            : '未知';

  const handleSend = async () => {
    const text = input.trim();
    if (!text || chatBusy) return;
    setChatBusy(true);
    setJustFinished(false);
    setError(null);
    setStreaming('');
    setToolStatus(null);
    setTurnMeta(null);
    setInput('');

    try {
      let sid = sessionId;
      if (!sid) {
        const session = await boardAgentsApi.createSession({
          project_id: projectId,
          agent_id: agentId,
          title: text.slice(0, 40),
        });
        sid = session.id;
        setSessionId(sid);
        setSessions((prev) => [session, ...prev]);
      }

      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          session_id: sid!,
          role: 'user',
          content: text,
          created_at: new Date().toISOString(),
        },
      ]);

      const token = await getAuthRuntime().getToken();
      if (!token) throw new Error('未登录 Remote，无法调用 Agent');

      const { reply, cwd, cwd_source } = await boardAgentsApi.chatStream({
        project_id: projectId,
        session_id: sid,
        agent_id: agentId,
        message: text,
        cwd: workingDirectory.trim() || null,
        token,
        onDelta: (t) => setStreaming((s) => s + t),
        onStatus: (status) => {
          if (status.cwd && status.cwd_source) {
            setLastEffectiveCwd({
              cwd: status.cwd,
              source: status.cwd_source,
            });
          }
          if (status.transport || status.history_turns != null) {
            setTurnMeta({
              transport: status.transport,
              history_turns: status.history_turns,
            });
          }
        },
        onToolStart: (name) => setToolStatus({ name, done: false, ok: true }),
        onToolResult: (name, ok) => setToolStatus({ name, done: true, ok }),
      });
      if (cwd && cwd_source) {
        setLastEffectiveCwd({ cwd, source: cwd_source });
      }

      setStreaming('');
      setToolStatus(null);
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          session_id: sid!,
          role: 'assistant',
          content: reply || '(无回复)',
          created_at: new Date().toISOString(),
        },
      ]);
      setJustFinished(true);
      await refreshSessions();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStreaming('');
      setToolStatus(null);
      setJustFinished(false);
    } finally {
      setChatBusy(false);
    }
  };

  return (
    <div className="flex h-full min-h-0">
      {/* Sessions rail */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-secondary">
        <div className="flex items-center gap-1 border-b border-border px-2 py-2.5">
          <button
            type="button"
            className="flex shrink-0 items-center gap-0.5 rounded-md px-1.5 py-1 text-xs text-low hover:bg-primary hover:text-normal"
            aria-label="返回 Agents"
            onClick={() =>
              void navigate({
                to: '/projects/$projectId/agents',
                params: { projectId },
              })
            }
          >
            <ArrowLeftIcon className="size-3.5" />
            Agents
          </button>
          <span className="min-w-0 flex-1 truncate text-sm font-medium text-normal">
            会话列表
          </span>
          <Tooltip content="新会话" side="bottom">
            <button
              type="button"
              className="shrink-0 rounded-md p-1.5 text-low hover:bg-primary hover:text-normal"
              title="新会话"
              aria-label="新会话"
              onClick={() => void handleNewSession()}
            >
              <PlusIcon className="size-4" />
            </button>
          </Tooltip>
        </div>
        <ul className="min-h-0 flex-1 overflow-auto p-2">
          {sessions.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                className={cn(
                  'mb-1 w-full rounded-md px-2 py-2 text-left text-xs',
                  s.id === sessionId
                    ? 'bg-brand/15 text-normal'
                    : 'text-low hover:bg-primary'
                )}
                onClick={() => setSessionId(s.id)}
              >
                <div className="truncate font-medium">
                  {s.title || '未命名会话'}
                </div>
                <div className="text-[10px] opacity-70">
                  {new Date(s.updated_at).toLocaleString()}
                </div>
              </button>
            </li>
          ))}
        </ul>
      </aside>

      {/* Chat */}
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <h1 className="font-semibold text-normal">{title}</h1>
            <p className="text-xs text-low">
              {(agent?.chat_runtime ?? 'cursor').toUpperCase()} · 新建会话 /
              继续会话
            </p>
            <p
              className="mt-1 max-w-xl truncate text-[11px] text-low"
              title={previewCwd}
            >
              工作目录：{previewCwd}
              <span className="opacity-70"> · {previewSource}</span>
            </p>
          </div>
          {agentId && (
            <span
              className={cn(
                'rounded-full px-2.5 py-1 text-xs',
                llm?.has_api_key
                  ? 'bg-brand/15 text-normal'
                  : 'bg-destructive/10 text-destructive'
              )}
            >
              {llm?.has_api_key ? 'API Key 已配置' : '请先配置 API Key'}
            </span>
          )}
        </header>

        {agentId && (
          <>
            <section className="shrink-0 border-b border-border bg-secondary px-4 py-2">
              <button
                type="button"
                className="flex w-full items-center justify-between gap-2 text-left"
                onClick={() => setAgentConfigOpen((v) => !v)}
              >
                <div className="flex items-center gap-1.5">
                  {agentConfigOpen ? (
                    <CaretDownIcon className="size-3.5 text-low" />
                  ) : (
                    <CaretRightIcon className="size-3.5 text-low" />
                  )}
                  <h2 className="text-sm font-medium text-normal">
                    Agent 配置
                  </h2>
                </div>
                <span className="text-xs text-low">
                  instructions · runtime · api_key · base_url
                </span>
              </button>
              {agentConfigOpen && (
                <div className="mt-3 grid max-w-3xl gap-2 sm:grid-cols-2">
                  <textarea
                    className="sm:col-span-2 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
                    rows={2}
                    placeholder="Instructions / 系统提示"
                    value={instructions}
                    onChange={(e) => setInstructions(e.target.value)}
                  />
                  <label className="block text-xs text-low sm:col-span-2">
                    对话 Runtime
                    <select
                      className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal"
                      value={chatRuntime}
                      onChange={(e) =>
                        setChatRuntime(
                          e.target.value as 'cursor' | 'pi' | 'opencode'
                        )
                      }
                    >
                      <option value="cursor">Cursor SDK（默认）</option>
                      <option value="pi">Pi（OpenAI 兼容 base_url）</option>
                      <option value="opencode">
                        OpenCode（OpenAI 兼容 base_url）
                      </option>
                    </select>
                  </label>
                  <label className="block text-xs text-low sm:col-span-2">
                    {chatRuntime === 'cursor'
                      ? 'Cursor User API Key'
                      : 'API Key（OpenAI 兼容）'}
                    <input
                      className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal"
                      type="password"
                      autoComplete="off"
                      placeholder={
                        llm?.has_api_key
                          ? '已保存（留空不改；填新值可覆盖）'
                          : chatRuntime === 'cursor'
                            ? '必填：key_...（Cursor Dashboard → API Keys）'
                            : '必填：对应 runtime 的 API Key'
                      }
                      value={apiKey}
                      onChange={(e) => setApiKey(e.target.value)}
                    />
                  </label>
                  <label className="block text-xs text-low sm:col-span-2">
                    Base URL
                    {chatRuntime !== 'cursor'
                      ? '（Pi/OpenCode 必填）'
                      : '（可选）'}
                    <input
                      className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal"
                      placeholder={
                        chatRuntime === 'cursor'
                          ? '默认留空（官方 Cursor）'
                          : 'https://api.openai.com/v1 或自建网关'
                      }
                      value={baseUrl}
                      onChange={(e) => setBaseUrl(e.target.value)}
                    />
                  </label>
                  <div className="sm:col-span-2">
                    <PrimaryButton
                      disabled={busy}
                      onClick={() => void handleSaveAgentConfig()}
                    >
                      保存 Agent 配置
                    </PrimaryButton>
                  </div>
                </div>
              )}
            </section>

            <section className="shrink-0 border-b border-border px-4 py-3">
              <div className="mb-2 flex items-center justify-between">
                <h2 className="text-sm font-medium text-normal">LLM 设置</h2>
                <span className="text-xs text-low">model_name · 工作目录</span>
              </div>
              <div className="grid max-w-3xl gap-2 sm:grid-cols-2">
                <label className="block text-xs text-low">
                  Model name
                  <AgentModelNameField
                    value={modelName}
                    onChange={setModelName}
                    chatRuntime={chatRuntime}
                    apiKey={apiKey}
                    baseUrl={baseUrl}
                    agentId={agentId}
                    hasSavedApiKey={!!llm?.has_api_key}
                    placeholder={
                      chatRuntime === 'cursor' ? 'composer-2.5' : 'gpt-4.1-mini'
                    }
                  />
                </label>
                <label className="block text-xs text-low sm:col-span-2">
                  工作目录（Cursor 本地文件操作）
                  <div className="mt-1 flex gap-2">
                    <input
                      className="min-w-0 flex-1 rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal"
                      placeholder={
                        defaultCwd
                          ? `留空则用 sidecar 默认：${defaultCwd}`
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
                      onClick={() => void handlePickDirectory()}
                    >
                      选择…
                    </button>
                    {workingDirectory && (
                      <button
                        type="button"
                        className="shrink-0 rounded-md px-2 py-2 text-sm text-low hover:text-destructive"
                        onClick={() => setWorkingDirectory('')}
                      >
                        清除
                      </button>
                    )}
                  </div>
                  <p className="mt-1 text-[11px] text-low">
                    未指定时实际启动目录：
                    {cwdLoading
                      ? '（读取中…）'
                      : (defaultCwd ?? '（无法读取 sidecar 默认目录）')}
                  </p>
                </label>
                <div className="sm:col-span-2">
                  <PrimaryButton
                    disabled={busy}
                    onClick={() => void handleSaveModelAndCwd()}
                  >
                    保存
                  </PrimaryButton>
                </div>
              </div>
            </section>
          </>
        )}

        {agentId && recentTasks.length > 0 && (
          <section className="shrink-0 border-b border-border px-4 py-2">
            <h2 className="mb-1 text-xs font-medium text-low">最近执行</h2>
            <ul className="flex flex-wrap gap-2">
              {recentTasks.map((task) => {
                const issue = issuesById.get(task.issue_id);
                return (
                  <li
                    key={task.id}
                    className="rounded-md border border-border bg-secondary px-2 py-1 text-[11px] text-low"
                  >
                    <span className="font-medium text-normal">
                      {issue?.simple_id ?? task.issue_id.slice(0, 8)}
                    </span>
                    {' · '}
                    {task.status}
                    {' · '}
                    {task.trigger}
                    {' · '}
                    {task.attempt}/{task.max_attempts}
                  </li>
                );
              })}
            </ul>
          </section>
        )}

        <div className="min-h-0 flex-1 space-y-3 overflow-auto px-4 py-4">
          {messages.map((m) => (
            <div
              key={m.id}
              className={cn(
                'max-w-3xl rounded-lg px-3 py-2 text-sm',
                m.role === 'user'
                  ? 'ml-auto bg-brand/20 text-normal'
                  : 'bg-secondary text-normal'
              )}
            >
              <p className="whitespace-pre-wrap">{m.content}</p>
            </div>
          ))}
          {chatBusy && (
            <div className="max-w-3xl rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-normal">
              {streaming ? (
                <p className="whitespace-pre-wrap">{streaming}</p>
              ) : (
                <p className="text-low">等待 Agent 回复…</p>
              )}
              <div className="mt-2 flex items-center gap-2 border-t border-border/60 pt-2 text-xs text-low">
                <SpinnerIcon className="size-3.5 shrink-0 animate-spin text-brand" />
                <span className="animate-pulse">
                  {streaming
                    ? toolStatus && !toolStatus.done
                      ? '工具执行中…'
                      : '生成中…'
                    : '收尾中…'}
                </span>
                {turnMeta?.transport && (
                  <span className="truncate opacity-70">
                    ·{' '}
                    {turnMeta.transport === 'openai-compatible'
                      ? 'OpenAI 兼容'
                      : turnMeta.transport === 'cursor-sdk'
                        ? 'Cursor SDK'
                        : turnMeta.transport}
                    {turnMeta.history_turns != null
                      ? ` · 上下文 ${turnMeta.history_turns} 条`
                      : ''}
                  </span>
                )}
                {toolStatus && (
                  <span className="truncate opacity-80">
                    ·{' '}
                    {toolStatus.done
                      ? `${toolStatus.ok ? '✓' : '✗'} ${toolStatus.name}${
                          toolStatus.ok ? '' : ' 失败'
                        }`
                      : `${toolStatus.name} 执行中…`}
                  </span>
                )}
              </div>
            </div>
          )}
          {!chatBusy && justFinished && (
            <div className="max-w-3xl px-1 text-xs text-low opacity-70">
              本轮已完成
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {error && (
          <div className="border-t border-border px-4 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        <div className="border-t border-border p-3">
          {chatBusy && (
            <p className="mb-2 text-xs text-low">
              Agent 运行中，请等待本轮结束后再发送
            </p>
          )}
          <div className="flex gap-2">
            <textarea
              className={cn(
                'min-h-[44px] flex-1 resize-none rounded-md border border-border bg-primary px-3 py-2 text-sm',
                chatBusy && 'cursor-not-allowed opacity-60'
              )}
              placeholder={
                chatBusy
                  ? 'Agent 运行中…'
                  : '输入消息…（Enter 发送，Shift+Enter 换行）'
              }
              value={input}
              disabled={chatBusy}
              rows={2}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  void handleSend();
                }
              }}
            />
            <PrimaryButton
              disabled={chatBusy || !input.trim()}
              onClick={() => void handleSend()}
              title={chatBusy ? 'Agent 运行中' : '发送'}
            >
              {chatBusy ? (
                <SpinnerIcon className="size-4 animate-spin" />
              ) : (
                <PaperPlaneTiltIcon className="size-4" />
              )}
            </PrimaryButton>
          </div>
        </div>
      </div>
    </div>
  );
}

export function ProjectAgentDetailPage() {
  const { isSignedIn } = useAuth();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const params = useParams({ strict: false }) as {
    projectId: string;
    agentId: string;
  };

  if (!isSignedIn) return <LoginRequiredPrompt />;
  if (!selectedOrgId || !params.projectId || !params.agentId) return null;

  return (
    <OrgProvider organizationId={selectedOrgId}>
      <ProjectProvider projectId={params.projectId}>
        <AgentChatInner projectId={params.projectId} agentId={params.agentId} />
      </ProjectProvider>
    </OrgProvider>
  );
}

export function ProjectCopilotPage() {
  const { isSignedIn } = useAuth();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const { projectId } = useParams({ strict: false }) as { projectId: string };

  if (!isSignedIn) return <LoginRequiredPrompt />;
  if (!selectedOrgId || !projectId) return null;

  return (
    <OrgProvider organizationId={selectedOrgId}>
      <ProjectProvider projectId={projectId}>
        <AgentChatInner projectId={projectId} agentId={null} />
      </ProjectProvider>
    </OrgProvider>
  );
}
