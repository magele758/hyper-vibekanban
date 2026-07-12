import { useMemo, useState } from 'react';
import { cn } from '@/shared/lib/utils';

export type ScheduleInputMode = 'simple' | 'cron';

type SimpleKind = 'daily' | 'weekdays' | 'interval' | 'custom';

const WEEKDAYS: { value: number; label: string }[] = [
  { value: 1, label: '一' },
  { value: 2, label: '二' },
  { value: 3, label: '三' },
  { value: 4, label: '四' },
  { value: 5, label: '五' },
  { value: 6, label: '六' },
  { value: 0, label: '日' },
];

const selectClass =
  'rounded-md border border-border bg-primary px-2 py-1.5 text-sm';

function pad2(n: number): string {
  return String(n).padStart(2, '0');
}

function buildCron(
  kind: SimpleKind,
  hour: number,
  minute: number,
  intervalHours: number,
  days: number[]
): string {
  switch (kind) {
    case 'daily':
      return `${minute} ${hour} * * *`;
    case 'weekdays':
      return `${minute} ${hour} * * 1-5`;
    case 'interval': {
      const n = Math.max(1, Math.min(24, intervalHours));
      return `0 */${n} * * *`;
    }
    case 'custom': {
      const sorted = [...days].sort((a, b) => a - b);
      const dow = sorted.length > 0 ? sorted.join(',') : '*';
      return `${minute} ${hour} * * ${dow}`;
    }
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}

function describeCron(cron: string): string {
  const expr = cron.trim();
  if (!expr) return '请输入 Cron 表达式';

  const specials: Record<string, string> = {
    '@hourly': '每小时',
    '@daily': '每天午夜',
    '@midnight': '每天午夜',
    '@weekly': '每周',
  };
  if (specials[expr]) return specials[expr];

  const parts = expr.split(/\s+/);
  if (parts.length !== 5) return `Cron：${expr}`;

  const [min, hour, , , dow] = parts;

  if (min.startsWith('*/') && hour === '*') {
    const n = min.slice(2);
    return `每 ${n} 分钟`;
  }
  if (hour.startsWith('*/') && (min === '0' || min === '00')) {
    const n = hour.slice(2);
    return `每 ${n} 小时`;
  }

  const timeLabel =
    /^\d+$/.test(min) && /^\d+$/.test(hour)
      ? `${pad2(Number(hour))}:${pad2(Number(min))}`
      : null;

  if (timeLabel) {
    if (dow === '*') return `每天 ${timeLabel}`;
    if (dow === '1-5') return `工作日 ${timeLabel}`;
    if (/^[\d,]+$/.test(dow)) {
      const labels = dow
        .split(',')
        .map((d) => WEEKDAYS.find((w) => w.value === Number(d))?.label ?? d)
        .join('、');
      return `每周${labels} ${timeLabel}`;
    }
  }

  return `Cron：${expr}`;
}

function parseSimpleFromCron(cron: string): {
  kind: SimpleKind;
  hour: number;
  minute: number;
  intervalHours: number;
  days: number[];
} | null {
  const parts = cron.trim().split(/\s+/);
  if (parts.length !== 5) return null;
  const [min, hour, dom, mon, dow] = parts;
  if (dom !== '*' || mon !== '*') return null;

  if (hour.startsWith('*/') && (min === '0' || min === '00')) {
    const n = Number(hour.slice(2));
    if (Number.isFinite(n) && n >= 1 && n <= 24) {
      return {
        kind: 'interval',
        hour: 9,
        minute: 0,
        intervalHours: n,
        days: [1, 2, 3, 4, 5],
      };
    }
  }

  if (!/^\d+$/.test(min) || !/^\d+$/.test(hour)) return null;
  const minute = Number(min);
  const hourNum = Number(hour);
  if (minute > 59 || hourNum > 23) return null;

  if (dow === '*') {
    return {
      kind: 'daily',
      hour: hourNum,
      minute,
      intervalHours: 2,
      days: [1, 2, 3, 4, 5],
    };
  }
  if (dow === '1-5') {
    return {
      kind: 'weekdays',
      hour: hourNum,
      minute,
      intervalHours: 2,
      days: [1, 2, 3, 4, 5],
    };
  }
  if (/^[\d,]+$/.test(dow)) {
    const days = dow
      .split(',')
      .map(Number)
      .filter((d) => d >= 0 && d <= 6);
    if (days.length > 0) {
      return {
        kind: 'custom',
        hour: hourNum,
        minute,
        intervalHours: 2,
        days,
      };
    }
  }
  return null;
}

/** IANA timezone string for the browser local zone. */
export function getLocalTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
  } catch {
    return 'UTC';
  }
}

type Props = {
  value: string;
  onChange: (cron: string) => void;
  className?: string;
};

export function AutopilotScheduleField({ value, onChange, className }: Props) {
  const parsed = parseSimpleFromCron(value);
  const [mode, setMode] = useState<ScheduleInputMode>(
    parsed ? 'simple' : 'cron'
  );
  const [kind, setKind] = useState<SimpleKind>(parsed?.kind ?? 'weekdays');
  const [hour, setHour] = useState(parsed?.hour ?? 9);
  const [minute, setMinute] = useState(parsed?.minute ?? 0);
  const [intervalHours, setIntervalHours] = useState(
    parsed?.intervalHours ?? 2
  );
  const [days, setDays] = useState<number[]>(parsed?.days ?? [1, 2, 3, 4, 5]);

  const preview = useMemo(() => describeCron(value), [value]);

  const applySimple = (
    nextKind: SimpleKind,
    nextHour: number,
    nextMinute: number,
    nextInterval: number,
    nextDays: number[]
  ) => {
    onChange(buildCron(nextKind, nextHour, nextMinute, nextInterval, nextDays));
  };

  const toggleDay = (day: number) => {
    const next = days.includes(day)
      ? days.filter((d) => d !== day)
      : [...days, day];
    setDays(next);
    applySimple(kind, hour, minute, intervalHours, next);
  };

  const switchMode = (next: ScheduleInputMode) => {
    setMode(next);
    if (next === 'simple') {
      const again = parseSimpleFromCron(value);
      if (again) {
        setKind(again.kind);
        setHour(again.hour);
        setMinute(again.minute);
        setIntervalHours(again.intervalHours);
        setDays(again.days);
      } else {
        applySimple(kind, hour, minute, intervalHours, days);
      }
    }
  };

  return (
    <div className={cn('space-y-2', className)}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-low">计划时间</span>
        <div className="inline-flex rounded-md border border-border p-0.5 text-xs">
          <button
            type="button"
            className={cn(
              'rounded px-2.5 py-1 transition-colors',
              mode === 'simple'
                ? 'bg-brand/15 text-normal'
                : 'text-low hover:text-normal'
            )}
            onClick={() => switchMode('simple')}
          >
            简单
          </button>
          <button
            type="button"
            className={cn(
              'rounded px-2.5 py-1 transition-colors',
              mode === 'cron'
                ? 'bg-brand/15 text-normal'
                : 'text-low hover:text-normal'
            )}
            onClick={() => switchMode('cron')}
          >
            Cron
          </button>
        </div>
      </div>

      {mode === 'simple' ? (
        <div className="space-y-2">
          <select
            className={cn(selectClass, 'w-full')}
            value={kind}
            onChange={(e) => {
              const next = e.target.value as SimpleKind;
              setKind(next);
              applySimple(next, hour, minute, intervalHours, days);
            }}
          >
            <option value="daily">每天</option>
            <option value="weekdays">工作日（周一至周五）</option>
            <option value="interval">每隔 N 小时</option>
            <option value="custom">指定星期</option>
          </select>

          {kind === 'interval' ? (
            <label className="flex items-center gap-2 text-xs text-low">
              每隔
              <select
                className={selectClass}
                value={intervalHours}
                onChange={(e) => {
                  const n = Number(e.target.value);
                  setIntervalHours(n);
                  applySimple(kind, hour, minute, n, days);
                }}
              >
                {[1, 2, 3, 4, 6, 8, 12, 24].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
              小时
            </label>
          ) : (
            <div className="flex flex-wrap items-center gap-2 text-xs text-low">
              <span>时间</span>
              <select
                className={selectClass}
                value={hour}
                onChange={(e) => {
                  const h = Number(e.target.value);
                  setHour(h);
                  applySimple(kind, h, minute, intervalHours, days);
                }}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>
                    {pad2(i)}
                  </option>
                ))}
              </select>
              <span>:</span>
              <select
                className={selectClass}
                value={minute}
                onChange={(e) => {
                  const m = Number(e.target.value);
                  setMinute(m);
                  applySimple(kind, hour, m, intervalHours, days);
                }}
              >
                {[0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55].map((m) => (
                  <option key={m} value={m}>
                    {pad2(m)}
                  </option>
                ))}
              </select>
            </div>
          )}

          {kind === 'custom' && (
            <div className="flex flex-wrap gap-1">
              {WEEKDAYS.map((d) => {
                const active = days.includes(d.value);
                return (
                  <button
                    key={d.value}
                    type="button"
                    className={cn(
                      'size-7 rounded-md text-xs transition-colors',
                      active
                        ? 'bg-brand/20 text-normal'
                        : 'border border-border text-low hover:bg-primary'
                    )}
                    onClick={() => toggleDay(d.value)}
                  >
                    {d.label}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      ) : (
        <input
          className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm font-mono"
          placeholder="0 9 * * 1-5"
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
      )}

      <p className="text-xs text-low">
        {preview}
        <span className="ml-1 font-mono text-low/80">（{value || '—'}）</span>
      </p>
      <p className="text-[11px] text-low/70">
        时区：本地（{getLocalTimezone()}）
      </p>
    </div>
  );
}
