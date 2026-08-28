/**
 * ============================================================
 *  Traces = 链路追踪（/traces）—— 真实数据，无 demo
 *  - 从后端 IPC 读取真实 trace，无数据则显示空态引导
 * ============================================================
 */
import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillButton from '@components/ui/PillButton';
import PillBadge from '@components/ui/PillBadge';
import PillInput from '@components/ui/PillInput';
import Icon from '@icons/index';
import { invoke } from '@commands/index';

interface TraceRow {
  id: string;
  model: string;
  status: string;
  latency_ms: number;
  tokens: number;
  ts: string;
  client: string;
}

const STATUS_BADGE: Record<string, 'pass' | 'warn' | 'fail' | 'neutral'> = {
  ok: 'pass', success: 'pass', partial: 'warn', fail: 'fail',
};

function shortId(id: string) { return (id ?? '').length > 16 ? id.slice(0, 16) + '…' : id; }
function fmtTs(v: any): string {
  const n = typeof v === 'number' ? v : parseInt(v, 10);
  if (!n || Number.isNaN(n)) return '';
  const d = new Date(n < 1e12 ? n * 1000 : n);
  return isNaN(d.getTime()) ? '' : d.toLocaleString();
}
// 归一化：兼容后端 recent_traces 实际字段（trace_id/latency/created_at 等）与前端期望，防御 undefined
function normRow(r: any): TraceRow {
  return {
    id: r.trace_id ?? r.id ?? r.traceId ?? '',
    model: r.model ?? r.resolved_model ?? '',
    status: String(r.status ?? r.final_status_code ?? ''),
    latency_ms: r.latency_ms ?? r.latency ?? 0,
    tokens: r.tokens ?? r.billed_total ?? 0,
    ts: fmtTs(r.created_at ?? r.ts),
    client: r.node_group ?? r.client ?? '',
  };
}
function statusKind(s: string): 'pass' | 'warn' | 'fail' | 'neutral' {
  if (STATUS_BADGE[s]) return STATUS_BADGE[s];
  const n = parseInt(s, 10);
  if (!Number.isNaN(n)) {
    if (n >= 200 && n < 300) return 'pass';
    if (n >= 400) return 'fail';
    if (n >= 300) return 'neutral';
    return 'warn';
  }
  return 'neutral';
}
function fmtT(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(n);
}

const Traces: React.FC = () => {
  const { t } = useTranslation();
  const [q, setQ] = useState('');
  const [traces, setTraces] = useState<TraceRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    try {
      setLoading(true); setError(null);
      const rows: any = await invoke('list_traces');
      setTraces(Array.isArray(rows) ? rows.map(normRow) : []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, []);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return traces;
    return traces.filter(t =>
      (t.id ?? '').toLowerCase().includes(s) || (t.model ?? '').toLowerCase().includes(s) || (t.client ?? '').toLowerCase().includes(s)
    );
  }, [q, traces]);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold">{t('traces.title')}</h2>
          <p className="text-sm text-neutral-500 mt-1">
            {t('traces.description')}
          </p>
        </div>
      </div>

      {/* 搜索 */}
      <PillCard padding="md">
        <div className="grid grid-cols-1 md:grid-cols-12 gap-3">
          <div className="md:col-span-8">
            <PillInput
              label={t('traces.search_label')}
              placeholder={t('traces.search_placeholder')}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              prefix={<Icon name="search" size={14} />}
            />
          </div>
          <div className="md:col-span-4 flex items-end">
            <PillButton size="md" variant="soft" leftIcon={<Icon name="refresh-cw" size={16} />} onClick={load} className="w-full">
              {t('traces.refresh')}
            </PillButton>
          </div>
        </div>
      </PillCard>

      {/* Trace 表格 */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="git-branch" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('traces.recent')}</div>
              <div className="text-[11px] text-neutral-500">{t('traces.real_records')}</div>
            </div>
          </div>
          <div className="text-[11px] text-neutral-500 tabular-nums">{t('traces.show_count', { count: filtered.length })}</div>
        </div>

        {loading ? (
          <div className="p-8 animate-pulse rounded-b-softer bg-neutral-100 dark:bg-neutral-900/50" />
        ) : error ? (
          <div className="px-5 py-6 text-sm text-neutral-500">
            {t('traces.load_failed')}: {error}
          </div>
        ) : filtered.length === 0 ? (
          <div className="px-5 py-14 text-center text-sm text-neutral-500">
            <p>{t('traces.no_data')}</p>
            <p className="mt-1 text-xs text-neutral-400">
              {t('traces.no_data_hint')}
            </p>
          </div>
        ) : (
          <div className="overflow-hidden rounded-b-softer">
            <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                            bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
              <div className="col-span-4">{t('traces.col_id')}</div>
              <div className="col-span-3">{t('traces.col_model')}</div>
              <div className="col-span-2 text-center">{t('traces.col_status')}</div>
              <div className="col-span-2 text-right">{t('traces.col_latency')}</div>
              <div className="col-span-1 text-right">{t('traces.col_tokens')}</div>
            </div>
            {filtered.map((t) => (
              <div key={t.id}
                className="grid grid-cols-12 px-5 py-3 text-xs items-center
                           border-b last:border-b-0 border-neutral-200/50 dark:border-neutral-800/50">
                <div className="col-span-4 min-w-0">
                  <div className="font-mono truncate">{shortId(t.id)}</div>
                  <div className="text-[10px] text-neutral-500 tabular-nums mt-0.5">{t.ts}</div>
                </div>
                <div className="col-span-3 font-mono truncate min-w-0">{t.model}</div>
                <div className="col-span-2 text-center">
                  <PillBadge variant={statusKind(t.status)} size="sm">{t.status || '—'}</PillBadge>
                </div>
                <div className="col-span-2 text-right tabular-nums">{Number(t.latency_ms) || 0}ms</div>
                <div className="col-span-1 text-right tabular-nums">{fmtT(t.tokens)}</div>
              </div>
            ))}
          </div>
        )}
      </PillCard>
    </div>
  );
};

export default Traces;
