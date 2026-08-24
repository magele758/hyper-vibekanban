import { useCallback, useMemo, useRef, useState } from 'react';
import type {
  ExecutionProcessStatus,
  TrajectoryEvent,
  TrajectoryResponse,
  TrajectorySegment,
} from 'shared/types';
import { cn } from '@/shared/lib/utils';

type HoverTarget =
  | { kind: 'process'; segment: TrajectorySegment }
  | { kind: 'event'; segment: TrajectorySegment; event: TrajectoryEvent };

const PROCESS_STATUS_BAR: Record<ExecutionProcessStatus, string> = {
  completed: 'bg-emerald-500',
  failed: 'bg-red-500',
  killed: 'bg-orange-500',
  running: 'bg-sky-500',
};

const PROCESS_STATUS_TEXT: Record<ExecutionProcessStatus, string> = {
  completed: 'text-emerald-400',
  failed: 'text-red-400',
  killed: 'text-orange-400',
  running: 'text-sky-400',
};

const EVENT_KIND_COLORS: Record<string, string> = {
  user_message: 'bg-emerald-500',
  user_feedback: 'bg-orange-400',
  user_answered_questions: 'bg-emerald-400',
  assistant_message: 'bg-sky-500',
  thinking: 'bg-violet-500',
  tool_use: 'bg-brand',
  task: 'bg-indigo-500',
  system_message: 'bg-zinc-400',
  error_message: 'bg-red-500',
  token_usage_info: 'bg-teal-500',
  next_action: 'bg-amber-400',
  loading: 'bg-zinc-500',
};

const EVENT_STATUS_COLORS: Record<string, string> = {
  failed: 'bg-red-500',
  denied: 'bg-red-400',
  timed_out: 'bg-orange-500',
  pending_approval: 'bg-amber-400',
  created: 'bg-brand/70',
  success: 'bg-brand',
};

const LEGEND: { key: string; label: string; className: string }[] = [
  { key: 'user', label: 'User', className: EVENT_KIND_COLORS.user_message },
  {
    key: 'assistant',
    label: 'Assistant',
    className: EVENT_KIND_COLORS.assistant_message,
  },
  { key: 'thinking', label: 'Think', className: EVENT_KIND_COLORS.thinking },
  { key: 'tool', label: 'Tool', className: EVENT_KIND_COLORS.tool_use },
  { key: 'task', label: 'Task', className: EVENT_KIND_COLORS.task },
  {
    key: 'system',
    label: 'System',
    className: EVENT_KIND_COLORS.system_message,
  },
  { key: 'error', label: 'Error', className: EVENT_KIND_COLORS.error_message },
];

function isTaskEvent(event: TrajectoryEvent): boolean {
  return event.kind === 'tool_use' && event.label === 'task';
}

function eventColor(event: TrajectoryEvent): string {
  if (isTaskEvent(event)) return EVENT_KIND_COLORS.task;
  if (event.kind === 'tool_use' && event.status) {
    return EVENT_STATUS_COLORS[event.status] ?? EVENT_KIND_COLORS.tool_use;
  }
  return EVENT_KIND_COLORS[event.kind] ?? 'bg-zinc-500';
}

type EventStep = {
  id: string;
  events: TrajectoryEvent[];
  parallel: boolean;
};

/** Consecutive tool_use calls are one parallel batch; other entries are sequential steps. */
function toEventSteps(events: TrajectoryEvent[]): EventStep[] {
  const steps: EventStep[] = [];
  let tools: TrajectoryEvent[] = [];

  const flushTools = () => {
    if (tools.length === 0) return;
    steps.push({
      id: `tools-${tools[0].index}`,
      events: tools,
      parallel: tools.length > 1,
    });
    tools = [];
  };

  for (const event of events) {
    if (event.kind === 'tool_use') {
      tools.push(event);
      continue;
    }
    flushTools();
    steps.push({
      id: `solo-${event.index}`,
      events: [event],
      parallel: false,
    });
  }
  flushTools();
  return steps;
}

function parallelBatchStats(segments: TrajectorySegment[]) {
  let batches = 0;
  let toolsInBatches = 0;
  let maxWidth = 0;
  let tasks = 0;
  for (const segment of segments) {
    const steps = toEventSteps(segment.events ?? []);
    for (const step of steps) {
      if (step.parallel) {
        batches += 1;
        toolsInBatches += step.events.length;
        maxWidth = Math.max(maxWidth, step.events.length);
      }
      tasks += step.events.filter(isTaskEvent).length;
    }
  }
  return { batches, toolsInBatches, maxWidth, tasks };
}

function formatClock(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString();
}

function formatTick(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))}ms`;
  const totalSeconds = Math.round(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

function segmentTimes(segment: TrajectorySegment, now: number) {
  const start = Date.parse(segment.started_at);
  const end = segment.completed_at ? Date.parse(segment.completed_at) : now;
  return {
    start: Number.isNaN(start) ? now : start,
    end: Number.isNaN(end) ? now : Math.max(end, start),
  };
}

function timeDomain(segments: TrajectorySegment[], now: number) {
  if (segments.length === 0) {
    return { min: now, max: now + 1 };
  }
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  for (const segment of segments) {
    const { start, end } = segmentTimes(segment, now);
    min = Math.min(min, start);
    max = Math.max(max, end);
  }
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
    return { min: now, max: now + 1 };
  }
  return { min, max };
}

export function TrajectoryTimeline({ data }: { data: TrajectoryResponse }) {
  const now = useMemo(() => Date.now(), [data.session_id]);
  const domain = useMemo(
    () => timeDomain(data.segments, now),
    [data.segments, now]
  );
  const span = Math.max(1, domain.max - domain.min);

  const [hover, setHover] = useState<HoverTarget | null>(null);
  const [selected, setSelected] = useState<HoverTarget | null>(null);
  const hoverTimer = useRef<number | null>(null);

  const clearHoverTimer = () => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
  };

  const scheduleHover = useCallback((target: HoverTarget) => {
    clearHoverTimer();
    hoverTimer.current = window.setTimeout(() => {
      setHover(target);
    }, 400);
  }, []);

  const hideHover = useCallback(() => {
    clearHoverTimer();
    setHover(null);
  }, []);

  const inspect = hover ?? selected;
  const eventCount = data.segments.reduce(
    (sum, segment) => sum + (segment.events?.length ?? 0),
    0
  );
  const concurrency = useMemo(
    () => parallelBatchStats(data.segments),
    [data.segments]
  );
  const idleGaps = useMemo(() => {
    const gaps: { start: number; end: number }[] = [];
    const timed = data.segments.map((segment) => ({
      segment,
      ...segmentTimes(segment, now),
    }));
    timed.sort((a, b) => a.start - b.start);
    for (let i = 1; i < timed.length; i += 1) {
      const prev = timed[i - 1];
      const next = timed[i];
      if (next.start - prev.end >= 30_000) {
        gaps.push({ start: prev.end, end: next.start });
      }
    }
    return gaps;
  }, [data.segments, now]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-base overflow-hidden p-base">
      <header className="shrink-0 rounded bg-secondary p-base">
        <div className="text-base font-medium text-high">
          {data.session_name || 'Unnamed Session'}
        </div>
        <div className="mt-half flex flex-wrap items-center gap-x-base gap-y-half text-xs text-low">
          {data.executor && <span>{data.executor}</span>}
          <span>{data.completeness.total_processes} processes</span>
          <span>{eventCount} events</span>
          <span>
            logs {data.completeness.with_logs}/
            {data.completeness.total_processes}
          </span>
          {data.completeness.dropped > 0 && (
            <span>dropped {data.completeness.dropped}</span>
          )}
          {data.totals.last_token_usage && (
            <span>
              tokens{' '}
              {data.totals.last_token_usage.total_tokens.toLocaleString()}
            </span>
          )}
          {concurrency.batches > 0 && (
            <span>
              {concurrency.batches} parallel batches (max {concurrency.maxWidth}
              )
            </span>
          )}
          {concurrency.tasks > 0 && <span>{concurrency.tasks} tasks</span>}
        </div>
        <p className="mt-half text-xs text-low">
          Turns are sequential follow-ups. Stacked tools in one column were
          issued together.
        </p>
      </header>

      <section className="shrink-0 rounded bg-secondary p-base">
        <div className="mb-half flex items-center justify-between gap-base">
          <div className="text-xs font-medium text-medium">Time overview</div>
          <div className="font-ibm-plex-mono text-xs text-low">
            {formatDuration(span)}
          </div>
        </div>
        <div className="relative h-10 rounded bg-primary">
          {idleGaps.map((gap) => {
            const left = ((gap.start - domain.min) / span) * 100;
            const width = ((gap.end - gap.start) / span) * 100;
            const ms = gap.end - gap.start;
            return (
              <div
                key={`idle-${gap.start}`}
                className="absolute top-1.5 flex h-7 items-center justify-center border border-dashed border-border/70 bg-primary/40"
                style={{ left: `${left}%`, width: `${width}%` }}
                title={`Idle ${formatDuration(ms)}`}
              >
                {width >= 8 && (
                  <span className="truncate px-half font-ibm-plex-mono text-[10px] text-low">
                    idle {formatDuration(ms)}
                  </span>
                )}
              </div>
            );
          })}
          {data.segments.map((segment) => {
            const { start, end } = segmentTimes(segment, now);
            const left = ((start - domain.min) / span) * 100;
            const width = Math.max(((end - start) / span) * 100, 1.5);
            const isSelected =
              selected?.kind === 'process' &&
              selected.segment.execution_process_id ===
                segment.execution_process_id;
            return (
              <button
                key={segment.execution_process_id}
                type="button"
                aria-label={`${segment.run_reason} ${segment.status}`}
                className={cn(
                  'absolute top-1.5 h-7 rounded-sm px-1 text-left transition-opacity',
                  PROCESS_STATUS_BAR[segment.status],
                  segment.dropped && 'opacity-40',
                  isSelected
                    ? 'ring-1 ring-inset ring-white/80'
                    : 'hover:brightness-110'
                )}
                style={{ left: `${left}%`, width: `${width}%` }}
                onMouseEnter={() => scheduleHover({ kind: 'process', segment })}
                onMouseLeave={hideHover}
                onFocus={() => setHover({ kind: 'process', segment })}
                onBlur={hideHover}
                onClick={() => setSelected({ kind: 'process', segment })}
              >
                <span className="block truncate px-half font-ibm-plex-mono text-[10px] leading-7 text-white">
                  {segment.run_reason}
                </span>
              </button>
            );
          })}
        </div>
        <div className="mt-half flex justify-between font-ibm-plex-mono text-xs text-low">
          <span>{formatTick(domain.min)}</span>
          <span>{formatTick(domain.min + span / 2)}</span>
          <span>{formatTick(domain.max)}</span>
        </div>
      </section>

      <section className="flex min-h-0 flex-1 flex-col gap-half overflow-hidden rounded bg-secondary p-base">
        <div className="flex shrink-0 items-center justify-between gap-base">
          <div className="text-xs font-medium text-medium">
            Event sequence · stacked = parallel
          </div>
          <div className="flex flex-wrap items-center gap-base text-xs text-low">
            {LEGEND.map((item) => (
              <span key={item.key} className="inline-flex items-center gap-1">
                <span className={cn('size-2 rounded-sm', item.className)} />
                {item.label}
              </span>
            ))}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-auto">
          <div className="flex flex-col gap-base">
            {data.segments.map((segment) => (
              <div
                key={segment.execution_process_id}
                className="flex min-w-0 flex-col gap-1"
              >
                <div className="flex items-center gap-half text-xs">
                  <span
                    className={cn(
                      'font-medium',
                      PROCESS_STATUS_TEXT[segment.status]
                    )}
                  >
                    {segment.run_reason}
                  </span>
                  <span className="text-low">{segment.status}</span>
                  <span className="text-low">
                    {formatDuration(
                      segmentTimes(segment, now).end -
                        segmentTimes(segment, now).start
                    )}
                  </span>
                  <span className="text-low">
                    {segment.events?.length ?? 0} events
                  </span>
                  {(() => {
                    const local = parallelBatchStats([segment]);
                    return local.batches > 0 ? (
                      <span className="text-low">{local.batches} parallel</span>
                    ) : null;
                  })()}
                </div>
                {(segment.events?.length ?? 0) === 0 ? (
                  <div className="text-xs text-low">No events</div>
                ) : (
                  <div className="flex flex-wrap items-end gap-px">
                    {toEventSteps(segment.events ?? []).map((step) => (
                      <div
                        key={`${segment.execution_process_id}-${step.id}`}
                        className={cn(
                          'flex flex-col gap-px',
                          step.parallel &&
                            'rounded-sm bg-white/5 p-px ring-1 ring-white/20'
                        )}
                        title={
                          step.parallel
                            ? `${step.events.length} tools in parallel`
                            : undefined
                        }
                      >
                        {step.events.map((event) => {
                          const isSelected =
                            selected?.kind === 'event' &&
                            selected.segment.execution_process_id ===
                              segment.execution_process_id &&
                            selected.event.index === event.index;
                          return (
                            <button
                              key={`${segment.execution_process_id}-${event.index}`}
                              type="button"
                              aria-label={`${event.kind} ${event.label}`}
                              title={event.label}
                              className={cn(
                                'rounded-[1px]',
                                step.parallel
                                  ? 'h-2.5 min-w-3'
                                  : 'h-4 min-w-1.5',
                                eventColor(event),
                                isSelected && 'ring-1 ring-white'
                              )}
                              onMouseEnter={() =>
                                scheduleHover({
                                  kind: 'event',
                                  segment,
                                  event,
                                })
                              }
                              onMouseLeave={hideHover}
                              onFocus={() =>
                                setHover({ kind: 'event', segment, event })
                              }
                              onBlur={hideHover}
                              onClick={() =>
                                setSelected({
                                  kind: 'event',
                                  segment,
                                  event,
                                })
                              }
                            />
                          );
                        })}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </section>

      <DetailPanel
        target={inspect}
        pinned={selected !== null}
        now={now}
        onClear={() => {
          setSelected(null);
          setHover(null);
        }}
      />
    </div>
  );
}

function DetailPanel({
  target,
  pinned,
  now,
  onClear,
}: {
  target: HoverTarget | null;
  pinned: boolean;
  now: number;
  onClear: () => void;
}) {
  if (!target) {
    return (
      <aside className="shrink-0 rounded bg-secondary p-base text-xs text-low">
        Hover a block for details. Click to pin.
      </aside>
    );
  }

  if (target.kind === 'process') {
    const { segment } = target;
    const { start, end } = segmentTimes(segment, now);
    return (
      <aside className="max-h-[40%] shrink-0 overflow-auto rounded bg-secondary p-base">
        <DetailHeader
          title={segment.run_reason}
          subtitle={segment.status}
          pinned={pinned}
          onClear={onClear}
        />
        <dl className="mt-half grid grid-cols-2 gap-x-base gap-y-1 text-xs">
          <DetailField
            label="Started"
            value={formatClock(segment.started_at)}
          />
          <DetailField
            label="Completed"
            value={
              segment.completed_at
                ? formatClock(segment.completed_at)
                : 'running'
            }
          />
          <DetailField label="Duration" value={formatDuration(end - start)} />
          <DetailField
            label="Exit"
            value={
              segment.exit_code === null || segment.exit_code === undefined
                ? '—'
                : String(segment.exit_code)
            }
          />
          <DetailField label="Events" value={String(segment.entry_count)} />
          <DetailField
            label="Logs"
            value={segment.has_logs ? 'yes' : 'missing'}
          />
        </dl>
        {segment.final_message && (
          <p className="mt-base whitespace-pre-wrap text-xs text-normal">
            {segment.final_message}
          </p>
        )}
      </aside>
    );
  }

  const { segment, event } = target;
  return (
    <aside className="max-h-[40%] shrink-0 overflow-auto rounded bg-secondary p-base">
      <DetailHeader
        title={event.label}
        subtitle={`${event.kind}${event.status ? ` · ${event.status}` : ''}`}
        pinned={pinned}
        onClear={onClear}
      />
      <dl className="mt-half grid grid-cols-2 gap-x-base gap-y-1 text-xs">
        <DetailField label="Process" value={segment.run_reason} />
        <DetailField
          label="Time"
          value={event.timestamp ? formatClock(event.timestamp) : 'sequence'}
        />
      </dl>
      {event.preview && (
        <p className="mt-base whitespace-pre-wrap break-words text-xs text-normal">
          {event.preview}
        </p>
      )}
    </aside>
  );
}

function DetailHeader({
  title,
  subtitle,
  pinned,
  onClear,
}: {
  title: string;
  subtitle: string;
  pinned: boolean;
  onClear: () => void;
}) {
  return (
    <div className="flex items-start justify-between gap-base">
      <div className="min-w-0">
        <div className="truncate text-xs font-medium text-high">{title}</div>
        <div className="text-xs text-low">{subtitle}</div>
      </div>
      {pinned && (
        <button
          type="button"
          className="shrink-0 text-xs text-low hover:text-normal"
          onClick={onClear}
        >
          Unpin
        </button>
      )}
    </div>
  );
}

function DetailField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-low">{label}</dt>
      <dd className="font-ibm-plex-mono text-normal">{value}</dd>
    </div>
  );
}
