import { useEffect, useRef, useState } from 'react';
import {
  ChatCircleIcon,
  PaperPlaneTiltIcon,
  SpinnerIcon,
  XIcon,
} from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { cn } from '@/shared/lib/utils';
import type { Agent, Issue } from 'shared/remote-types';
import type { SquadEditorDraft } from './SquadPipelineEditor';
import {
  findAgentWithApiKey,
  generateSquadFromChat,
  type SquadChatGeneration,
  type SquadChatGenStatus,
} from './squadPipelineFromChat';

type ChatMsg = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
};

type Props = {
  projectId: string;
  agents: Agent[];
  issues: Issue[];
  /** Current draft when iterating inside the editor. */
  draft: SquadEditorDraft | null;
  onApply: (draft: SquadEditorDraft, meta: SquadChatGeneration) => void;
  onClose: () => void;
  /** Compact panel under editor vs full create entry. */
  compact?: boolean;
};

const EXAMPLES = [
  '先用小八查代码，再并行 review，最后汇总',
  '先调研，等待 30 秒，再实现，最后测试',
  '并行 code-review 和 安全检查，再汇总',
];

function statusLabel(status: SquadChatGenStatus | null): string {
  switch (status) {
    case 'checking':
      return '检查 Agent…';
    case 'llm':
      return '正在调用 Agent…';
    case 'template':
      return '模板解析';
    default:
      return '正在生成流水线…';
  }
}

export function SquadChatCreatePanel({
  projectId,
  agents,
  issues,
  draft,
  onApply,
  onClose,
  compact,
}: Props) {
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [genStatus, setGenStatus] = useState<SquadChatGenStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  /** Prefer Agent/LLM when a key is available; user can force template. */
  const [preferAgent, setPreferAgent] = useState(true);
  const [messages, setMessages] = useState<ChatMsg[]>([
    {
      id: 'welcome',
      role: 'assistant',
      content:
        '用自然语言描述流水线即可生成 DAG。默认会调用已配置 API Key 的 Agent；也可切到「仅模板」。生成后可在画布微调。',
    },
  ]);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, busy, genStatus]);

  useEffect(() => {
    let cancelled = false;
    setHasApiKey(null);
    void findAgentWithApiKey(agents).then((keyed) => {
      if (cancelled) return;
      const ok = Boolean(keyed);
      setHasApiKey(ok);
      setPreferAgent(ok);
    });
    return () => {
      cancelled = true;
    };
  }, [agents]);

  const send = async (text: string) => {
    const msg = text.trim();
    if (!msg || busy) return;
    setInput('');
    setError(null);
    setMessages((prev) => [
      ...prev,
      { id: `u_${Date.now()}`, role: 'user', content: msg },
    ]);
    setBusy(true);
    setGenStatus(preferAgent ? 'checking' : 'template');
    try {
      const result = await generateSquadFromChat({
        message: msg,
        agents,
        issues,
        projectId,
        current: draft,
        preferLlm: preferAgent,
        onStatus: setGenStatus,
      });
      const warnSuffix =
        result.warnings.length > 0 && !result.llmError
          ? `\n⚠️ ${result.warnings.slice(0, 3).join('；')}`
          : result.llmError
            ? ''
            : '';
      const sourceTag =
        result.source === 'llm'
          ? '（LLM）'
          : result.source === 'patch'
            ? '（增量）'
            : '（模板）';
      setMessages((prev) => [
        ...prev,
        {
          id: `a_${Date.now()}`,
          role: 'assistant',
          content: `${result.summary}${sourceTag}${warnSuffix}`,
        },
      ]);
      onApply(result.draft, result);
    } catch (e) {
      const err = e instanceof Error ? e.message : String(e);
      setError(err);
      setMessages((prev) => [
        ...prev,
        {
          id: `a_${Date.now()}`,
          role: 'assistant',
          content: `生成失败：${err}`,
        },
      ]);
    } finally {
      setBusy(false);
      setGenStatus(null);
    }
  };

  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border border-border bg-secondary',
        compact ? 'max-h-72' : 'max-h-[28rem]'
      )}
    >
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div className="flex items-center gap-2 text-sm font-medium text-normal">
          <ChatCircleIcon className="size-4 text-brand" />
          对话创建流水线
        </div>
        <div className="flex items-center gap-2">
          <div
            className="flex rounded-md border border-border p-0.5 text-[11px]"
            role="group"
            aria-label="生成方式"
          >
            <button
              type="button"
              disabled={busy || hasApiKey === false}
              title={
                hasApiKey === false
                  ? '没有配置 API Key 的 Agent'
                  : '调用 Agent 生成 pipeline JSON'
              }
              className={cn(
                'rounded px-2 py-0.5 transition-colors',
                preferAgent
                  ? 'bg-brand/15 text-brand'
                  : 'text-low hover:text-normal',
                hasApiKey === false && 'cursor-not-allowed opacity-50'
              )}
              onClick={() => setPreferAgent(true)}
            >
              用 Agent 生成
            </button>
            <button
              type="button"
              disabled={busy}
              className={cn(
                'rounded px-2 py-0.5 transition-colors',
                !preferAgent
                  ? 'bg-brand/15 text-brand'
                  : 'text-low hover:text-normal'
              )}
              onClick={() => setPreferAgent(false)}
            >
              仅模板
            </button>
          </div>
          <button
            type="button"
            className="rounded p-1 text-low hover:bg-primary hover:text-normal"
            onClick={onClose}
            aria-label="关闭"
          >
            <XIcon className="size-4" />
          </button>
        </div>
      </div>

      {hasApiKey === false && (
        <p className="border-b border-border px-3 py-1.5 text-[11px] text-low">
          当前项目没有已配置 API Key 的 Agent，将使用模板解析。可在 Agent
          设置中配置 Key 后启用「用 Agent 生成」。
        </p>
      )}

      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-3 py-2">
        {messages.map((m) => (
          <div
            key={m.id}
            className={cn(
              'max-w-[95%] whitespace-pre-wrap rounded-md px-2.5 py-1.5 text-sm',
              m.role === 'user'
                ? 'ml-auto bg-brand/15 text-normal'
                : 'mr-auto bg-primary text-low'
            )}
          >
            {m.content}
          </div>
        ))}
        {busy && (
          <div className="flex items-center gap-2 text-xs text-low">
            <SpinnerIcon className="size-3.5 animate-spin" />
            {statusLabel(genStatus)}
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {!compact && messages.length <= 2 && (
        <div className="flex flex-wrap gap-1.5 border-t border-border px-3 py-2">
          {EXAMPLES.map((ex) => (
            <button
              key={ex}
              type="button"
              className="rounded-full border border-border px-2 py-0.5 text-[11px] text-low hover:border-brand hover:text-brand"
              onClick={() => void send(ex)}
              disabled={busy}
            >
              {ex}
            </button>
          ))}
        </div>
      )}

      {error && <p className="px-3 pb-1 text-xs text-destructive">{error}</p>}

      <div className="flex items-end gap-2 border-t border-border p-2">
        <textarea
          className="min-h-[2.5rem] max-h-24 flex-1 resize-y rounded-md border border-border bg-primary px-2 py-1.5 text-sm text-normal outline-none focus:border-brand"
          placeholder="描述工作流，或说「再加一个 wait」…"
          value={input}
          disabled={busy}
          rows={2}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send(input);
            }
          }}
        />
        <PrimaryButton
          disabled={busy || !input.trim()}
          className="shrink-0"
          onClick={() => void send(input)}
        >
          {busy ? (
            <SpinnerIcon className="size-4 animate-spin" />
          ) : (
            <PaperPlaneTiltIcon className="size-4" />
          )}
          生成
        </PrimaryButton>
      </div>
    </div>
  );
}
