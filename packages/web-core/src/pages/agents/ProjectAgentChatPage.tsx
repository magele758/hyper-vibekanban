import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useNavigate } from '@tanstack/react-router';
import {
  ArrowLeftIcon,
  PlusIcon,
  PaperPlaneTiltIcon,
} from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  boardAgentsApi,
  type AgentLlmSettings,
  type CopilotMessage,
  type CopilotSession,
} from '@/shared/lib/boardAgentsApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import { cn } from '@/shared/lib/utils';

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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [llm, setLlm] = useState<AgentLlmSettings | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [modelName, setModelName] = useState('');
  const [instructions, setInstructions] = useState('');
  const [chatRuntime, setChatRuntime] = useState<'cursor' | 'pi' | 'opencode'>(
    'cursor'
  );
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
    if (!agentId) return;
    void boardAgentsApi
      .getLlmSettings(agentId)
      .then((s) => {
        setLlm(s);
        setBaseUrl(s.base_url ?? '');
        setModelName(s.model_name ?? 'composer-2.5');
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
  }, [messages, streaming]);

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

  const handleSaveSettings = async () => {
    if (!agentId) return;
    setBusy(true);
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
        model_name: modelName.trim(),
      });
      setLlm(saved);
      setApiKey('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleSend = async () => {
    const text = input.trim();
    if (!text || busy) return;
    setBusy(true);
    setError(null);
    setStreaming('');
    setToolStatus(null);
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

      setStreaming('…');
      const { reply } = await boardAgentsApi.chatStream({
        project_id: projectId,
        session_id: sid,
        agent_id: agentId,
        message: text,
        token,
        onDelta: (t) => setStreaming((s) => (s === '…' ? t : s + t)),
        onToolStart: (name) =>
          setToolStatus({ name, done: false, ok: true }),
        onToolResult: (name, ok) =>
          setToolStatus({ name, done: true, ok }),
      });

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
      await refreshSessions();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStreaming('');
      setToolStatus(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex h-full min-h-0">
      {/* Sessions rail */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-secondary">
        <div className="flex items-center justify-between border-b border-border px-3 py-3">
          <button
            type="button"
            className="flex items-center gap-1 text-sm text-low hover:text-normal"
            onClick={() =>
              void navigate({
                to: '/projects/$projectId/agents',
                params: { projectId },
              })
            }
          >
            <ArrowLeftIcon className="size-4" />
            Agents
          </button>
          <button
            type="button"
            className="rounded p-1 text-low hover:text-normal"
            title="新建会话"
            onClick={() => void handleNewSession()}
          >
            <PlusIcon className="size-4" />
          </button>
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
          <section className="shrink-0 border-b border-border bg-secondary px-4 py-3">
            <div className="mb-2 flex items-center justify-between">
              <h2 className="text-sm font-medium text-normal">LLM 设置</h2>
              <span className="text-xs text-low">
                api_key · base_url · model_name
              </span>
            </div>
            <div className="grid max-w-3xl gap-2 sm:grid-cols-2">
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
                  <option value="opencode">OpenCode（OpenAI 兼容 base_url）</option>
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
              <label className="block text-xs text-low">
                Base URL
                {chatRuntime !== 'cursor' ? '（Pi/OpenCode 必填）' : '（可选）'}
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
              <label className="block text-xs text-low">
                Model name
                <input
                  className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal"
                  placeholder="composer-2.5"
                  value={modelName}
                  onChange={(e) => setModelName(e.target.value)}
                />
              </label>
              <div className="sm:col-span-2">
                <PrimaryButton
                  disabled={busy}
                  onClick={() => void handleSaveSettings()}
                >
                  保存 LLM 设置
                </PrimaryButton>
              </div>
            </div>
          </section>
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
              {m.role === 'assistant' ? (
                <p className="whitespace-pre-wrap">{m.content}</p>
              ) : (
                <p className="whitespace-pre-wrap">{m.content}</p>
              )}
            </div>
          ))}
          {toolStatus && (
            <div className="max-w-3xl rounded-lg bg-secondary px-3 py-1.5 text-xs text-low">
              {toolStatus.done ? (
                <span>
                  {toolStatus.ok ? '✓' : '✗'} {toolStatus.name}{' '}
                  {toolStatus.ok ? 'ok' : 'failed'}
                </span>
              ) : (
                <span>🔧 {toolStatus.name} …</span>
              )}
            </div>
          )}
          {streaming && (
            <div className="max-w-3xl rounded-lg bg-secondary px-3 py-2 text-sm">
              <p className="whitespace-pre-wrap">{streaming}</p>
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {error && (
          <div className="border-t border-border px-4 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        <div className="flex gap-2 border-t border-border p-3">
          <textarea
            className="min-h-[44px] flex-1 resize-none rounded-md border border-border bg-primary px-3 py-2 text-sm"
            placeholder="输入消息…（Enter 发送，Shift+Enter 换行）"
            value={input}
            disabled={busy}
            rows={2}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                void handleSend();
              }
            }}
          />
          <PrimaryButton disabled={busy || !input.trim()} onClick={() => void handleSend()}>
            <PaperPlaneTiltIcon className="size-4" />
          </PrimaryButton>
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
