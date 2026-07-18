import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  PlusIcon,
  PaperPlaneTiltIcon,
  SpinnerIcon,
  CaretDownIcon,
  GearIcon,
} from '@phosphor-icons/react';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import {
  boardAgentsApi,
  type CopilotMessage,
  type CopilotSession,
} from '@/shared/lib/boardAgentsApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import { cn } from '@/shared/lib/utils';
import { Tooltip } from '@vibe/ui/components/Tooltip';

function GlobalAgentsChatInner() {
  const { selectedOrgId } = useOrganizationStore();
  const { data: projects } = useOrganizationProjects(selectedOrgId);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null
  );

  // Auto-select first project if none selected
  useEffect(() => {
    if (!selectedProjectId && projects.length > 0) {
      setSelectedProjectId(projects[0].id);
    }
  }, [projects, selectedProjectId]);

  const selectedProject = useMemo(
    () => projects.find((p) => p.id === selectedProjectId),
    [projects, selectedProjectId]
  );

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
  const [chatBusy, setChatBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [justFinished, setJustFinished] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // 模型服务配置（服务端存储，按 project_id，多端共享）
  const [showSettings, setShowSettings] = useState(false);
  const [modelCfg, setModelCfg] = useState<{
    base_url: string;
    api_key: string;
    model: string;
    has_api_key: boolean;
  }>({ base_url: '', api_key: '', model: '', has_api_key: false });
  const [savingCfg, setSavingCfg] = useState(false);

  useEffect(() => {
    if (!selectedProjectId) return;
    void boardAgentsApi
      .getCopilotConfig(selectedProjectId)
      .then((c) =>
        setModelCfg({
          base_url: c.base_url ?? '',
          api_key: '',
          model: c.model ?? '',
          has_api_key: c.has_api_key,
        })
      )
      .catch(() => {});
  }, [selectedProjectId]);

  const saveModelCfg = useCallback(async () => {
    if (!selectedProjectId) return;
    setSavingCfg(true);
    try {
      const body: { base_url?: string; api_key?: string; model?: string } = {
        base_url: modelCfg.base_url,
        model: modelCfg.model,
      };
      // 只在用户输入了新 key 时才提交，避免覆盖成空。
      if (modelCfg.api_key) body.api_key = modelCfg.api_key;
      const saved = await boardAgentsApi.putCopilotConfig(
        selectedProjectId,
        body
      );
      setModelCfg({
        base_url: saved.base_url ?? '',
        api_key: '',
        model: saved.model ?? '',
        has_api_key: saved.has_api_key,
      });
      setShowSettings(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSavingCfg(false);
    }
  }, [selectedProjectId, modelCfg]);

  const refreshSessions = useCallback(async () => {
    if (!selectedProjectId) return;
    const list = await boardAgentsApi.listSessions({
      project_id: selectedProjectId,
      project_copilot: true,
    });
    setSessions(list);
    if (!sessionId && list[0]) {
      setSessionId(list[0].id);
    }
  }, [selectedProjectId, sessionId]);

  useEffect(() => {
    if (!selectedProjectId) return;
    void refreshSessions().catch((e) =>
      setError(e instanceof Error ? e.message : String(e))
    );
  }, [selectedProjectId]); // eslint-disable-line react-hooks/exhaustive-deps

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
    if (!selectedProjectId) return;
    const session = await boardAgentsApi.createSession({
      project_id: selectedProjectId,
      agent_id: null,
      title: `全局会话 ${new Date().toLocaleString()}`,
    });
    setSessions((prev) => [session, ...prev]);
    setSessionId(session.id);
    setMessages([]);
  };

  const handleSend = async () => {
    const text = input.trim();
    if (!text || chatBusy || !selectedProjectId) return;
    setChatBusy(true);
    setJustFinished(false);
    setError(null);
    setStreaming('');
    setToolStatus(null);
    setInput('');

    try {
      let sid = sessionId;
      if (!sid) {
        const session = await boardAgentsApi.createSession({
          project_id: selectedProjectId,
          agent_id: null,
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

      const { reply } = await boardAgentsApi.chatStream({
        project_id: selectedProjectId,
        session_id: sid,
        agent_id: null,
        message: text,
        cwd: null,
        token,
        onDelta: (t) => setStreaming((s) => s + t),
        onStatus: () => {},
        onToolStart: (name) => setToolStatus({ name, done: false, ok: true }),
        onToolResult: (name, ok) => setToolStatus({ name, done: true, ok }),
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
          <span className="min-w-0 flex-1 truncate text-sm font-medium text-normal">
            会话列表
          </span>
          <Tooltip content="新会话" side="bottom">
            <button
              type="button"
              className="shrink-0 rounded-md p-1.5 text-low hover:bg-primary hover:text-normal"
              title="新会话"
              onClick={() => void handleNewSession()}
              disabled={!selectedProjectId}
            >
              <PlusIcon className="size-4" />
            </button>
          </Tooltip>
        </div>
        <div className="flex-1 overflow-y-auto">
          {sessions.map((s) => (
            <button
              key={s.id}
              type="button"
              className={cn(
                'block w-full px-3 py-2 text-left text-sm hover:bg-primary',
                sessionId === s.id && 'bg-primary font-medium'
              )}
              onClick={() => setSessionId(s.id)}
            >
              {s.title}
            </button>
          ))}
        </div>
      </aside>

      {/* Chat area */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Project selector header */}
        <div className="flex items-center gap-2 border-b border-border bg-secondary px-4 py-3">
          <span className="text-sm text-low">目标项目：</span>
          <div className="relative">
            <select
              value={selectedProjectId ?? ''}
              onChange={(e) => {
                setSelectedProjectId(e.target.value);
                setSessionId(null);
                setSessions([]);
                setMessages([]);
              }}
              className="appearance-none rounded-md border border-border bg-primary px-3 py-1.5 pr-8 text-sm text-normal hover:bg-secondary focus:outline-none focus:ring-2 focus:ring-brand"
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <CaretDownIcon className="pointer-events-none absolute right-2 top-1/2 size-4 -translate-y-1/2 text-low" />
          </div>
          <span className="text-xs text-low">
            （全局指挥台 • 可创建/运行 Squad/Autopilot）
          </span>
          <div className="ml-auto flex items-center gap-2">
            {!modelCfg.base_url && !modelCfg.has_api_key && (
              <span className="text-xs text-amber-600">未配置模型服务</span>
            )}
            <Tooltip content="模型服务设置" side="bottom">
              <button
                type="button"
                className="shrink-0 rounded-md p-1.5 text-low hover:bg-primary hover:text-normal"
                onClick={() => setShowSettings((v) => !v)}
              >
                <GearIcon className="size-4" />
              </button>
            </Tooltip>
          </div>
        </div>

        {showSettings && (
          <div className="flex flex-col gap-2 border-b border-border bg-primary px-4 py-3">
            <div className="text-xs font-medium text-normal">
              模型服务（OpenAI 兼容）· 存服务端，按项目多端共享
            </div>
            <input
              value={modelCfg.base_url}
              onChange={(e) =>
                setModelCfg((c) => ({ ...c, base_url: e.target.value }))
              }
              placeholder="base_url，如 https://api.openai.com/v1"
              className="rounded-md border border-border bg-secondary px-3 py-1.5 text-sm text-normal placeholder:text-low focus:outline-none focus:ring-2 focus:ring-brand"
            />
            <input
              value={modelCfg.api_key}
              onChange={(e) =>
                setModelCfg((c) => ({ ...c, api_key: e.target.value }))
              }
              type="password"
              placeholder={
                modelCfg.has_api_key
                  ? 'api_key 已保存（留空不改）'
                  : 'api_key，如 sk-...'
              }
              className="rounded-md border border-border bg-secondary px-3 py-1.5 text-sm text-normal placeholder:text-low focus:outline-none focus:ring-2 focus:ring-brand"
            />
            <input
              value={modelCfg.model}
              onChange={(e) =>
                setModelCfg((c) => ({ ...c, model: e.target.value }))
              }
              placeholder="model_name，如 gpt-4o / deepseek-chat"
              className="rounded-md border border-border bg-secondary px-3 py-1.5 text-sm text-normal placeholder:text-low focus:outline-none focus:ring-2 focus:ring-brand"
            />
            <div className="flex items-center justify-between">
              <p className="text-xs text-low">
                留空则回落到 sidecar 的环境变量配置。
              </p>
              <button
                type="button"
                onClick={() => void saveModelCfg()}
                disabled={savingCfg || !selectedProjectId}
                className="rounded-md bg-brand px-3 py-1.5 text-sm font-medium text-white hover:bg-brand/90 disabled:opacity-50"
              >
                {savingCfg ? '保存中…' : '保存'}
              </button>
            </div>
          </div>
        )}

        {/* Messages */}
        <div className="flex-1 overflow-y-auto px-4 py-4">
          {!selectedProject && (
            <div className="flex h-full items-center justify-center text-sm text-low">
              请选择一个项目开始对话
            </div>
          )}
          {selectedProject && messages.length === 0 && !streaming && (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
              <p className="text-sm text-low">
                全局 Copilot 指挥台 · 当前项目：
                <span className="font-medium text-normal">
                  {selectedProject.name}
                </span>
              </p>
              <p className="text-xs text-low">
                可通过自然语言编排 Agent / Squad / Autopilot
              </p>
            </div>
          )}
          {messages.map((m) => (
            <div
              key={m.id}
              className={cn(
                'mb-4 rounded-lg px-4 py-3',
                m.role === 'user'
                  ? 'bg-brand/10 text-normal'
                  : 'bg-secondary text-normal'
              )}
            >
              <div className="mb-1 text-xs font-medium text-low">
                {m.role === 'user' ? '你' : 'Copilot'}
              </div>
              <div className="whitespace-pre-wrap text-sm">{m.content}</div>
            </div>
          ))}
          {streaming && (
            <div className="mb-4 rounded-lg bg-secondary px-4 py-3">
              <div className="mb-1 text-xs font-medium text-low">Copilot</div>
              <div className="whitespace-pre-wrap text-sm text-normal">
                {streaming}
              </div>
            </div>
          )}
          {toolStatus && (
            <div className="mb-4 flex items-center gap-2 text-xs text-low">
              {!toolStatus.done && (
                <SpinnerIcon className="size-4 animate-spin" />
              )}
              <span>
                工具 <code className="text-brand">{toolStatus.name}</code>
                {toolStatus.done
                  ? toolStatus.ok
                    ? ' 完成'
                    : ' 失败'
                  : ' 运行中…'}
              </span>
            </div>
          )}
          {justFinished && (
            <div className="mb-4 text-xs text-low">✓ 回复完成</div>
          )}
          {error && (
            <div className="mb-4 rounded-lg bg-red-500/10 px-4 py-3 text-sm text-red-600">
              错误：{error}
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {/* Input area */}
        <div className="border-t border-border bg-secondary px-4 py-3">
          <div className="flex items-end gap-2">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  void handleSend();
                }
              }}
              placeholder={
                selectedProject
                  ? '描述需求或编排任务…（Enter 发送，Shift+Enter 换行）'
                  : '请先选择项目'
              }
              disabled={chatBusy || !selectedProjectId}
              className="min-h-[80px] flex-1 resize-none rounded-md border border-border bg-primary px-3 py-2 text-sm text-normal placeholder:text-low focus:outline-none focus:ring-2 focus:ring-brand disabled:opacity-50"
            />
            <button
              type="button"
              onClick={() => void handleSend()}
              disabled={chatBusy || !input.trim() || !selectedProjectId}
              className="shrink-0 rounded-md bg-brand px-4 py-2 text-sm font-medium text-white hover:bg-brand/90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {chatBusy ? (
                <SpinnerIcon className="size-4 animate-spin" />
              ) : (
                <PaperPlaneTiltIcon className="size-4" />
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export function GlobalAgentsPage() {
  const { isSignedIn } = useAuth();

  if (!isSignedIn) {
    return <LoginRequiredPrompt />;
  }

  return <GlobalAgentsChatInner />;
}
