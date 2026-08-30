/**
 * ============================================================
 *  Traces = 链路追踪 + 对账账单（/traces）—— 真实数据，无 demo
 *  - v0.2.24：分页 + 批量选择/删除 + 导出错误可读化
 *  - v0.2.27：对账（BillingAudit）并入本页，tab 切换统一「记录与对账」板块
 *  - 从后端 IPC 分页读取真实 trace，无数据则显示空态引导
 * ============================================================
 */
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillButton from '@components/ui/PillButton';
import PillBadge from '@components/ui/PillBadge';
import PillInput from '@components/ui/PillInput';
import PillTabs from '@components/ui/PillTabs';
import Icon from '@icons/index';
import { invoke } from '@commands/index';
import { save } from '@tauri-apps/plugin-dialog';
import BillingAudit from './Storage';

interface TraceRow {
  id: string;
  model: string;
  upstream: string;
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
// 归一化：兼容后端实际字段（trace_id/latency/created_at 等）与前端期望，防御 undefined
function normRow(r: any): TraceRow {
  return {
    id: r.trace_id ?? r.id ?? r.traceId ?? '',
    model: r.resolved_model || r.model || '',
    upstream: r.served_host || '',
    status: String(r.status ?? r.final_status_code ?? ''),
    latency_ms: r.latency_ms ?? r.latency ?? 0,
    tokens: r.tokens ?? r.billed_total ?? 0,
    ts: fmtTs(r.created_at ?? r.ts),
    client: r.client_key_name || r.node_group || '',
  };
}
function statusKind(s: string): 'pass' | 'warn' | 'fail' | 'neutral' {
  if (STATUS_BADGE[s]) return STATUS_BADGE[s];
  const n = parseInt(s, 10);
  if (!Number.isNaN(n)) {
    if (n === 0) return 'neutral';
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
// 错误可读化：Tauri 命令失败时 reject 的是序列化后的对象（如 {error:{message}}），
// 直接 String(e) 会得到 "[object Object]"，这里提取真实信息
function errMsg(e: any): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  const m = e?.error?.message ?? e?.message;
  if (m) return String(m);
  try { return JSON.stringify(e); } catch { return String(e); }
}

interface ErrLogRow {
  id: number;
  ts_ms: number;
  context: string;
  label: string;
  message: string;
}

const PAGE_SIZES = [25, 50, 100, 200];

const Traces: React.FC = () => {
  const { t } = useTranslation();
  // v0.2.27：链路追踪 / 对账账单 双 tab（原 /billing 独立页已并入）
  const [tab, setTab] = useState<'trace' | 'billing'>('trace');
  const [q, setQ] = useState('');
  const [traces, setTraces] = useState<TraceRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exportMsg, setExportMsg] = useState<string | null>(null);
  // 分页
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);
  const [total, setTotal] = useState(0);
  const pages = Math.max(1, Math.ceil(total / pageSize));
  // 批量选择
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);
  // 错误日志面板
  const [errLogs, setErrLogs] = useState<ErrLogRow[]>([]);
  const [errTotal, setErrTotal] = useState(0);
  const [errLoading, setErrLoading] = useState(false);

  const loadErrLogs = useCallback(async () => {
    try {
      setErrLoading(true);
      const r: any = await invoke('list_error_logs', { page: 1, pageSize: 100 });
      const items: any[] = Array.isArray(r) ? r : (r?.items ?? []);
      setErrLogs(items);
      setErrTotal(typeof r === 'object' && !Array.isArray(r) ? (r?.total ?? items.length) : items.length);
    } catch { /* 静默：面板加载失败不影响主表格 */ }
    finally { setErrLoading(false); }
  }, []);

  const clearErrLogs = async () => {
    if (!window.confirm(t('errlog.clear_confirm'))) return;
    try { await invoke('clear_error_logs'); await loadErrLogs(); } catch (e) { setError(errMsg(e)); }
  };

  const load = useCallback(async (p: number, ps: number) => {
    try {
      setLoading(true); setError(null);
      const r: any = await invoke('list_traces', { page: p, pageSize: ps });
      const items: any[] = Array.isArray(r) ? r : (r?.items ?? []);
      setTraces(items.map(normRow));
      setTotal(typeof r === 'object' && !Array.isArray(r) ? (r?.total ?? items.length) : items.length);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(page, pageSize); setSelected(new Set()); loadErrLogs(); }, [page, pageSize, load, loadErrLogs]);

  const exportExcel = async () => {
    try {
      setExportMsg(null);
      const path = await save({
        title: t('traces.export'),
        defaultPath: `PLocalSwitch-traces-${new Date().toISOString().slice(0, 10)}.xlsx`,
        filters: [{ name: 'Excel', extensions: ['xlsx'] }],
      });
      if (!path) return;
      const r: any = await invoke('export_traces_excel', { path });
      const n = typeof r === 'number' ? r : (r?.data ?? r?.count ?? 0);
      setExportMsg(t('traces.export_done', { count: n }));
    } catch (e) {
      setExportMsg(t('traces.export_fail') + ': ' + errMsg(e));
    }
  };

  const batchDelete = async () => {
    const ids = Array.from(selected);
    if (ids.length === 0 || deleting) return;
    if (!window.confirm(t('traces.delete_confirm', { count: ids.length }))) return;
    try {
      setDeleting(true);
      await invoke('delete_traces', { ids });
      setSelected(new Set());
      // 删除后当前页可能变空：回退一页
      const remaining = total - ids.length;
      const maxPage = Math.max(1, Math.ceil(remaining / pageSize));
      const next = Math.min(page, maxPage);
      if (next !== page) setPage(next); else await load(next, pageSize);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setDeleting(false);
    }
  };

  const toggleAll = () => {
    const allSelected = traces.length > 0 && traces.every(t => selected.has(t.id));
    if (allSelected) {
      const next = new Set(selected);
      traces.forEach(t => next.delete(t.id));
      setSelected(next);
    } else {
      const next = new Set(selected);
      traces.forEach(t => next.add(t.id));
      setSelected(next);
    }
  };
  const toggleOne = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id); else next.add(id);
    setSelected(next);
  };

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return traces;
    return traces.filter(t =>
      (t.id ?? '').toLowerCase().includes(s) || (t.model ?? '').toLowerCase().includes(s) || (t.client ?? '').toLowerCase().includes(s)
    );
  }, [q, traces]);

  const allPageSelected = filtered.length > 0 && filtered.every(t => selected.has(t.id));

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold">{t('traces.title')}</h2>
          <p className="text-sm text-neutral-500 mt-1">
            {t('traces.description')}
          </p>
        </div>
        <PillTabs size="md" variant="solid" value={tab} onChange={(v) => setTab(v as 'trace' | 'billing')}
          items={[
            { key: 'trace',   label: t('traces.tab_trace') },
            { key: 'billing', label: t('traces.tab_billing') },
          ]} />
      </div>

      {tab === 'billing' ? <BillingAudit /> : (<>
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
          <div className="md:col-span-4 flex items-end gap-2">
            <PillButton size="md" variant="soft" leftIcon={<Icon name="refresh-cw" size={16} />} onClick={() => load(page, pageSize)} className="flex-1">
              {t('traces.refresh')}
            </PillButton>
            <PillButton size="md" variant="primary" leftIcon={<Icon name="download" size={16} />} onClick={exportExcel} className="flex-1">
              {t('traces.export')}
            </PillButton>
          </div>
        </div>
        {exportMsg && (
          <div className="mt-2 text-[11px] text-neutral-500">{exportMsg}</div>
        )}
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
          <div className="flex items-center gap-3">
            {selected.size > 0 && (
              <PillButton size="sm" variant="danger" leftIcon={<Icon name="trash-2" size={14} />} onClick={batchDelete} disabled={deleting}>
                {t('traces.batch_delete')} ({selected.size})
              </PillButton>
            )}
            <div className="text-[11px] text-neutral-500 tabular-nums">{t('traces.show_count', { count: filtered.length })}</div>
          </div>
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
              <div className="col-span-1 flex items-center">
                <input type="checkbox" aria-label={t('traces.select_all')} className="accent-current"
                  checked={allPageSelected} onChange={toggleAll} />
              </div>
              <div className="col-span-3">{t('traces.col_id')}</div>
              <div className="col-span-2">{t('traces.col_model')}</div>
              <div className="col-span-2">{t('traces.col_upstream')}</div>
              <div className="col-span-1 text-center">{t('traces.col_status')}</div>
              <div className="col-span-2 text-right">{t('traces.col_latency')}</div>
              <div className="col-span-1 text-right">{t('traces.col_tokens')}</div>
            </div>
            {filtered.map((row) => (
              <div key={row.id}
                className="grid grid-cols-12 px-5 py-3 text-xs items-center
                           border-b last:border-b-0 border-neutral-200/50 dark:border-neutral-800/50">
                <div className="col-span-1 flex items-center">
                  <input type="checkbox" aria-label={t('traces.select_row')} className="accent-current"
                    checked={selected.has(row.id)} onChange={() => toggleOne(row.id)} />
                </div>
                <div className="col-span-3 min-w-0">
                  <div className="font-mono truncate" title={row.id}>{shortId(row.id)}</div>
                  <div className="text-[10px] text-neutral-500 tabular-nums mt-0.5">{row.ts}</div>
                </div>
                <div className="col-span-2 font-mono truncate min-w-0" title={row.model}>{row.model}</div>
                <div className="col-span-2 font-mono truncate min-w-0 text-neutral-500" title={row.upstream}>{row.upstream || '—'}</div>
                <div className="col-span-1 text-center">
                  <PillBadge variant={statusKind(row.status)} size="sm">{row.status && row.status !== '0' ? row.status : '—'}</PillBadge>
                </div>
                <div className="col-span-2 text-right tabular-nums">{Number(row.latency_ms) || 0}ms</div>
                <div className="col-span-1 text-right tabular-nums">{fmtT(row.tokens)}</div>
              </div>
            ))}
            {/* 分页条 */}
            <div className="flex flex-wrap items-center justify-between gap-2 px-5 py-3 text-[11px] text-neutral-500">
              <div className="flex items-center gap-2">
                <span>{t('traces.page_size')}</span>
                <select
                  value={pageSize}
                  onChange={(e) => { setPageSize(Number(e.target.value)); setPage(1); }}
                  className="rounded-md border border-neutral-300/70 dark:border-neutral-700 bg-transparent px-1.5 py-0.5 text-[11px]">
                  {PAGE_SIZES.map(s => <option key={s} value={s}>{s}</option>)}
                </select>
                <span className="tabular-nums">{t('traces.page_info', { page, pages, total })}</span>
              </div>
              <div className="flex items-center gap-1">
                <button aria-label={t('traces.prev_page')} disabled={page <= 1}
                  onClick={() => setPage(page - 1)}
                  className="p-1 rounded-md disabled:opacity-30 hover:bg-neutral-100 dark:hover:bg-neutral-800">
                  <Icon name="chevron-left" size={14} />
                </button>
                <span className="tabular-nums px-1">{page} / {pages}</span>
                <button aria-label={t('traces.next_page')} disabled={page >= pages}
                  onClick={() => setPage(page + 1)}
                  className="p-1 rounded-md disabled:opacity-30 hover:bg-neutral-100 dark:hover:bg-neutral-800">
                  <Icon name="chevron-right" size={14} />
                </button>
              </div>
            </div>
          </div>
        )}
      </PillCard>

      {/* 错误日志面板（转发链路失败统一落库） */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="alert-triangle" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('errlog.title')}</div>
              <div className="text-[11px] text-neutral-500">{t('errlog.hint', { count: errTotal })}</div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <PillButton size="sm" variant="soft" leftIcon={<Icon name="refresh-cw" size={14} />} onClick={loadErrLogs} disabled={errLoading}>
              {t('traces.refresh')}
            </PillButton>
            <PillButton size="sm" variant="danger" leftIcon={<Icon name="trash-2" size={14} />} onClick={clearErrLogs} disabled={errLogs.length === 0}>
              {t('errlog.clear')}
            </PillButton>
          </div>
        </div>
        {errLogs.length === 0 ? (
          <div className="px-5 pb-6 text-xs text-neutral-500">{t('errlog.empty')}</div>
        ) : (
          <div className="overflow-hidden rounded-b-softer">
            <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                            bg-neutral-50 dark:bg-neutral-900/60 border-y border-neutral-200/70 dark:border-neutral-800/70">
              <div className="col-span-3">{t('errlog.col_time')}</div>
              <div className="col-span-2">{t('errlog.col_context')}</div>
              <div className="col-span-2">{t('errlog.col_label')}</div>
              <div className="col-span-5">{t('errlog.col_message')}</div>
            </div>
            {errLogs.map((l) => (
              <div key={l.id}
                className="grid grid-cols-12 px-5 py-2.5 text-xs items-start
                           border-b last:border-b-0 border-neutral-200/50 dark:border-neutral-800/50">
                <div className="col-span-3 tabular-nums text-neutral-500">{fmtTs(l.ts_ms) || '—'}</div>
                <div className="col-span-2 font-mono truncate" title={l.context}>{l.context || '—'}</div>
                <div className="col-span-2 font-mono truncate text-amber-600 dark:text-amber-400" title={l.label}>{l.label || '—'}</div>
                <div className="col-span-5 break-words min-w-0" title={l.message}>{l.message || '—'}</div>
              </div>
            ))}
          </div>
        )}
      </PillCard>
      </>)}
    </div>
  );
};

export default Traces;
