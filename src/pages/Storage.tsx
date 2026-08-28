/**
 * ============================================================
 *  BillingAudit = 账本与对账（/billing）—— 真实数据，无 demo
 *  - 从后端 IPC 读取真实账本汇总，无数据则显示空态
 * ============================================================
 */
import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillButton from '@components/ui/PillButton';
import PillBadge from '@components/ui/PillBadge';
import PillTabs from '@components/ui/PillTabs';
import Icon from '@icons/index';
import { invoke } from '@commands/index';

function fmtInt(n: number) { return n.toLocaleString('en-US'); }
function fmtM(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(2)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return String(n);
}

interface Summary {
  requests_total: number;
  requests_ok: number;
  tokens_input: number;
  tokens_output: number;
  total_charge_cny: number;
}

const BillingAudit: React.FC = () => {
  const { t } = useTranslation();
  const [range, setRange] = useState('24h');
  const [summary, setSummary] = useState<Summary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    try {
      setLoading(true); setError(null);
      const s: any = await invoke('billing_summary', { window: range });
      setSummary(s ?? null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, [range]);

  const hasData = !!summary && (summary.requests_total > 0 || summary.total_charge_cny > 0);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold">{t('storage.title')}</h2>
          <p className="text-sm text-neutral-500 mt-1">
            {t('storage.description')}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <PillTabs size="sm" variant="solid" value={range} onChange={(v) => setRange(String(v))}
            items={[
              { key: '24h', label: t('storage.r24h') },
              { key: '7d',  label: t('storage.r7d') },
              { key: '30d', label: t('storage.r30d') },
            ]} />
          <PillButton size="md" variant="soft" leftIcon={<Icon name="refresh-cw" size={16} />} onClick={load}>
            {t('storage.refresh')}
          </PillButton>
        </div>
      </div>

      {loading ? (
        <div className="animate-pulse h-40 rounded-softer bg-neutral-100 dark:bg-neutral-900 border border-dashed border-neutral-200 dark:border-neutral-800" />
      ) : error ? (
        <PillCard padding="md">
          <div className="text-sm text-neutral-500">{t('storage.load_failed')}: {error}</div>
        </PillCard>
      ) : !hasData ? (
        <PillCard padding="md" className="text-center py-12">
          <div className="text-3xl font-black tracking-tight mb-2">0</div>
          <p className="text-sm text-neutral-500">{t('storage.no_data')}</p>
          <p className="mt-1 text-xs text-neutral-400">
            {t('storage.no_data_hint')}
          </p>
        </PillCard>
      ) : (
        <>
          {/* 汇总卡 */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <PillCard padding="md">
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('storage.requests')}</div>
              <div className="text-2xl font-bold mt-2 tabular-nums">{fmtInt(summary!.requests_total)}</div>
              <div className="flex items-center gap-2 mt-2 text-[11px]">
                <PillBadge variant="pass" size="sm">
                  {t('storage.success_rate')} {summary!.requests_total > 0 ? ((summary!.requests_ok / summary!.requests_total) * 100).toFixed(2) : 0}%
                </PillBadge>
              </div>
            </PillCard>
            <PillCard padding="md">
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('storage.tokens')}</div>
              <div className="text-2xl font-bold mt-2 tabular-nums">{fmtM(summary!.tokens_input + summary!.tokens_output)}</div>
              <div className="mt-2 text-[11px] text-neutral-500 tabular-nums flex gap-3">
                <span>{t('storage.input')} {fmtM(summary!.tokens_input)}</span>
                <span>{t('storage.output')} {fmtM(summary!.tokens_output)}</span>
              </div>
            </PillCard>
            {/* 客户端计费 / 上游成本 尚未接入，不显示伪数据 */}
          </div>
        </>
      )}
    </div>
  );
};

export default BillingAudit;
