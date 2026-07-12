import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CaretDownIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';
import { cn } from '@/shared/lib/utils';

export type AgentChatRuntime = 'cursor' | 'pi' | 'opencode';

type Props = {
  value: string;
  onChange: (value: string) => void;
  chatRuntime: AgentChatRuntime;
  apiKey: string;
  baseUrl: string;
  /** When set, sidecar can load the saved API key for listing. */
  agentId?: string | null;
  /** True when agent already has a saved key (detail settings form). */
  hasSavedApiKey?: boolean;
  className?: string;
  placeholder?: string;
};

/** OpenAI-compatible listing when Pi/OpenCode, or Cursor with a custom base_url. */
function usesOpenAiListing(
  chatRuntime: AgentChatRuntime,
  baseUrl: string
): boolean {
  return chatRuntime !== 'cursor' || !!baseUrl.trim();
}

export function AgentModelNameField({
  value,
  onChange,
  chatRuntime,
  apiKey,
  baseUrl,
  agentId,
  hasSavedApiKey = false,
  className,
  placeholder = 'composer-2.5',
}: Props) {
  const openAiMode = usesOpenAiListing(chatRuntime, baseUrl);
  const cursorSdkMode = chatRuntime === 'cursor' && !baseUrl.trim();

  const [open, setOpen] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const fetchSeq = useRef(0);

  const filtered = useMemo(() => {
    const q = value.trim().toLowerCase();
    if (!q) return models;
    return models.filter((id) => id.toLowerCase().includes(q));
  }, [models, value]);

  const canFetch =
    (!!apiKey.trim() || (!!agentId && hasSavedApiKey)) &&
    (cursorSdkMode || !!baseUrl.trim());

  const fetchModels = useCallback(async () => {
    if (!canFetch) {
      setModels([]);
      if (openAiMode && !baseUrl.trim()) {
        setListError('填写 Base URL 后可扫描模型列表');
      } else {
        setListError('填写 API Key（或使用已保存的 Key）后可扫描');
      }
      return;
    }
    const seq = ++fetchSeq.current;
    setLoading(true);
    setListError(null);
    try {
      let token: string | undefined;
      try {
        token = (await getAuthRuntime().getToken()) ?? undefined;
      } catch {
        token = undefined;
      }
      const listed = await boardAgentsApi.listModels({
        api_key: apiKey.trim() || undefined,
        // Cursor official: omit base_url so sidecar uses Cursor.models.list()
        base_url: openAiMode ? baseUrl.trim() : undefined,
        agent_id: agentId ?? null,
        token,
      });
      if (seq !== fetchSeq.current) return;
      setModels(listed.map((m) => m.id));
      if (listed.length === 0) {
        setListError('未返回模型，可手动输入');
      }
    } catch (err) {
      if (seq !== fetchSeq.current) return;
      setModels([]);
      setListError(err instanceof Error ? err.message : String(err));
    } finally {
      if (seq === fetchSeq.current) setLoading(false);
    }
  }, [agentId, apiKey, baseUrl, canFetch, openAiMode]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void fetchModels();
    }, 400);
    return () => window.clearTimeout(timer);
  }, [fetchModels]);

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, []);

  const valueMissingFromList =
    !!value.trim() && models.length > 0 && !models.includes(value.trim());

  const discoveryHint = listError
    ? listError
    : valueMissingFromList
      ? cursorSdkMode
        ? `「${value.trim()}」不是 Cursor SDK 可用 ID（多为 CLI 变体名）。请改选下方列表中的模型，例如 grok-4.5。`
        : `「${value.trim()}」不在当前网关模型列表中，请改选或确认 ID`
      : null;

  return (
    <div ref={rootRef} className="relative">
      <div className="flex gap-1">
        <div className="relative min-w-0 flex-1">
          <input
            className={cn(
              'mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 pr-9 text-sm text-normal',
              className
            )}
            placeholder={placeholder}
            value={value}
            onChange={(e) => {
              onChange(e.target.value);
              setOpen(true);
            }}
            onFocus={() => setOpen(true)}
            autoComplete="off"
            spellCheck={false}
          />
          <button
            type="button"
            title="选择模型"
            className="absolute right-1 top-1/2 mt-0.5 -translate-y-1/2 rounded p-1 text-low hover:bg-secondary hover:text-normal"
            onClick={() => setOpen((v) => !v)}
          >
            <CaretDownIcon className="size-4" />
          </button>
        </div>
        <button
          type="button"
          title="刷新模型列表"
          disabled={loading || !canFetch}
          className={cn(
            'mt-1 shrink-0 rounded-md border border-border px-2 text-low',
            'hover:bg-secondary hover:text-normal disabled:opacity-40'
          )}
          onClick={() => {
            void fetchModels();
            setOpen(true);
          }}
        >
          <ArrowsClockwiseIcon
            className={cn('size-4', loading && 'animate-spin')}
          />
        </button>
      </div>
      {discoveryHint && (
        <p className="mt-1 text-[11px] text-low">{discoveryHint}</p>
      )}
      {!discoveryHint && models.length > 0 && (
        <p className="mt-1 text-[11px] text-low">
          {cursorSdkMode
            ? `已从 Cursor SDK 扫描到 ${models.length} 个模型，可点选或继续手输`
            : `已扫描到 ${models.length} 个模型，可点选或继续手输`}
        </p>
      )}
      {open && filtered.length > 0 && (
        <ul
          className={cn(
            'absolute z-20 mt-1 max-h-56 w-full overflow-auto rounded-md border border-border',
            'bg-primary py-1 shadow-md'
          )}
        >
          {filtered.slice(0, 80).map((id) => (
            <li key={id}>
              <button
                type="button"
                className={cn(
                  'flex w-full px-3 py-1.5 text-left text-sm text-normal',
                  'hover:bg-secondary',
                  id === value && 'bg-secondary font-medium'
                )}
                onClick={() => {
                  onChange(id);
                  setOpen(false);
                }}
              >
                {id}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
