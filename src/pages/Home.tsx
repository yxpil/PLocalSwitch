import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillBadge from '@components/ui/PillBadge';
import PillButton from '@components/ui/PillButton';
import Icon from '@icons/index';
import { NavLink } from 'react-router-dom';
import { getWidgets } from '@plugins/index';
import { SafeRender } from '@plugins/index';
import { invoke } from '@commands/index';
import { gatewayStatus, gatewayStart, gatewayStop, gatewayRestart, gatewayAutoRestart } from '@commands/app';
import PillSwitch from '@components/ui/PillSwitch';
import { accessHost } from '../utils/net';

/**
 * Dashboard = 网关总览（真实配置统计数据，无假数据）
 */
function fmtUptime(sec: number): string {
  if (!sec || sec < 0) return '—';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

const Home: React.FC = () => {
  const { t } = useTranslation();
  const heroes = getWidgets('home.hero');
  const stats  = getWidgets('home.stats');
  const [upstreamCount, setUpstreamCount] = useState(0);
  const [aliasCount, setAliasCount] = useState(0);
  const [keyCount, setKeyCount] = useState(0);
  const [listenAddr, setListenAddr] = useState('');
  // 网关总控
  const [running, setRunning] = useState(false);
  const [ctrlBusy, setCtrlBusy] = useState(false);
  const [requests, setRequests] = useState(0);
  const [uptime, setUptime] = useState(0);
  // 崩溃自动重启
  const [autoRestart, setAutoRestart] = useState(false);
  const [restarts, setRestarts] = useState(0);

  const refreshGateway = async () => {
    try {
      const st: any = await gatewayStatus();
      setRunning(!!st.running);
      setListenAddr(accessHost(st.listen));
      setRequests(st.requests_total ?? 0);
      setUptime(st.uptime_seconds ?? 0);
      setAutoRestart(!!st.auto_restart);
      setRestarts(st.restarts ?? 0);
    } catch { /* 无后端时忽略 */ }
  };

  const toggleAutoRestart = async (v: boolean) => {
    setAutoRestart(v); // 乐观更新
    try { const ok = await gatewayAutoRestart(v); setAutoRestart(!!ok); }
    catch { await refreshGateway(); }
  };

  const toggleGateway = async (target: 'start' | 'stop') => {
    setCtrlBusy(true);
    try {
      if (target === 'stop') await gatewayStop();
      else await gatewayStart();
      await refreshGateway();
    } catch { /* 无后端时忽略 */ }
    finally { setCtrlBusy(false); }
  };

  useEffect(() => {
    (async () => {
      try {
        const cfg: any = await invoke('load_config');
        const groups: any[] = cfg?.node_groups ?? [];
        setUpstreamCount(groups.reduce((s, g) => s + (g.nodes?.length ?? 0), 0));
        setAliasCount((cfg?.model_aliases ?? []).length);
        setKeyCount((cfg?.billing?.client_keys ?? []).length);
      } catch { /* 无后端时忽略 */ }
      await refreshGateway();
    })();
  }, []);

  return (
    <div className="space-y-6">
      {/* Hero slot */}
      {heroes.map((h, i) => (
        <SafeRender key={`h-${i}`} moduleName={`w:hero:${i}`}>{<h.component />}</SafeRender>
      ))}

      {/* 网关总控（启停） */}
      <PillCard padding="md" className={running ? 'border-neutral-300 dark:border-neutral-700' : 'border-dashed'}>
        <div className="flex flex-wrap items-center gap-4">
          <div className="flex items-center gap-3">
            <div className={`h-12 w-12 rounded-pill flex items-center justify-center ${running ? 'bg-neutral-900 text-white dark:bg-white dark:text-black' : 'bg-neutral-100 dark:bg-neutral-900'}`}>
              <Icon name="zap" size={20} />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="font-semibold">{t('home.gateway_service')}</span>
                <PillBadge variant={running ? 'pass' : 'neutral'} size="sm">
                  {running ? t('home.running') : t('home.stopped')}
                </PillBadge>
              </div>
              <div className="text-[11px] text-neutral-500 tabular-nums mt-0.5">
                {listenAddr || '—'} {running && `· ${t('home.total_requests')} ${requests}`}
              </div>
            </div>
          </div>
          <div className="ml-auto flex items-center gap-2 flex-wrap">
            {/* 启动/暂停 二合一按钮，悬浮显示统计与状态 */}
            <div className="group relative">
              <PillButton size="md" variant={running ? 'ghost' : 'soft'}
                leftIcon={<Icon name={running ? 'pause' : 'play'} size={14} />}
                disabled={ctrlBusy}
                onClick={() => toggleGateway(running ? 'stop' : 'start')}>
                {running ? t('home.stop_gateway') : t('home.start_gateway')}
              </PillButton>
              <div className="pointer-events-none absolute right-0 top-full mt-2 z-50 w-64
                              rounded-xl border border-neutral-200 dark:border-neutral-800
                              bg-white dark:bg-neutral-950 p-3 text-xs shadow-card
                              opacity-0 invisible translate-y-0 transition-all duration-200
                              group-hover:opacity-100 group-hover:visible group-hover:translate-y-0">
                <div className="grid grid-cols-2 gap-x-3 gap-y-2.5">
                  <div>
                    <div className="text-neutral-500">{t('home.gateway_service')}</div>
                    <PillBadge variant={running ? 'pass' : 'neutral'} size="sm">
                      {running ? t('home.running') : t('home.stopped')}
                    </PillBadge>
                  </div>
                  <div>
                    <div className="text-neutral-500">{t('home.total_requests')}</div>
                    <div className="font-semibold tabular-nums">{requests}</div>
                  </div>
                  <div>
                    <div className="text-neutral-500">{t('about.uptime')}</div>
                    <div className="font-semibold tabular-nums">{fmtUptime(uptime)}</div>
                  </div>
                  <div>
                    <div className="text-neutral-500">{t('home.upstream_nodes')}</div>
                    <div className="font-semibold tabular-nums">{upstreamCount}</div>
                  </div>
                  <div>
                    <div className="text-neutral-500">{t('home.alias_key')}</div>
                    <div className="font-semibold tabular-nums">{aliasCount} / {keyCount}</div>
                  </div>
                  <div>
                    <div className="text-neutral-500">{t('home.listen')}</div>
                    <div className="font-mono truncate tabular-nums">{listenAddr || '—'}</div>
                  </div>
                </div>
              </div>
            </div>
            <PillButton size="md" variant="ghost" leftIcon={<Icon name="refresh-cw" size={14} />}
              disabled={ctrlBusy} onClick={refreshGateway}>
              {t('home.refresh')}
            </PillButton>
          </div>
        </div>
        <div className="mt-3 pt-3 border-t border-neutral-100 dark:border-neutral-900 flex flex-wrap items-center justify-between gap-3">
          <PillSwitch size="sm" checked={autoRestart} onChange={toggleAutoRestart}
            label={t('home.auto_restart')}
            description={t('home.auto_restart_desc')} />
          {restarts > 0 && (
            <span className="text-[11px] text-neutral-500 tabular-nums">
              {t('home.auto_restarts')} · {restarts}
            </span>
          )}
        </div>
        <div className="mt-2 text-[11px] text-neutral-500">
          {t('home.stop_hint')}
        </div>
      </PillCard>

      {/* Stats slot (4 张网关指标卡) */}
      {stats.length > 0 && (
        <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4">
          {stats.map((w, i) => (
            <SafeRender key={`s-${i}`} moduleName={`w:stats:${i}`}>{<w.component />}</SafeRender>
          ))}
        </div>
      )}

      {/* 真实配置概览 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <PillCard padding="md">
          <div className="text-[11px] text-neutral-500 uppercase tracking-wide mb-2">{t('home.listen')}</div>
          <div className="font-mono text-sm break-all">{listenAddr || '—'}</div>
          <div className="mt-3 flex items-center gap-2 text-xs text-neutral-500">
            <PillBadge variant="pass" size="sm">{t('home.running')}</PillBadge>
          </div>
        </PillCard>

        <PillCard padding="md">
          <div className="text-[11px] text-neutral-500 uppercase tracking-wide mb-2">{t('home.upstream_nodes')}</div>
          <div className="text-2xl font-bold tabular-nums">{upstreamCount}</div>
          <NavLink to="/models" className="mt-2 inline-flex items-center gap-1 text-xs text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-100">
            {t('home.add_upstream')} <Icon name="chevron-right" size={12} />
          </NavLink>
        </PillCard>

        <PillCard padding="md">
          <div className="text-[11px] text-neutral-500 uppercase tracking-wide mb-2">{t('home.alias_key')}</div>
          <div className="text-2xl font-bold tabular-nums">
            {aliasCount}<span className="text-sm text-neutral-400 mx-1">/</span>{keyCount}
          </div>
          <div className="mt-2 text-xs text-neutral-500">{t('home.downstream_use')}</div>
        </PillCard>
      </div>

      {/* 快速入口 */}
      <PillCard padding="md">
        <div className="flex flex-wrap gap-2">
          <NavLink to="/models" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                   bg-neutral-900 text-white shadow-pill hover:-translate-y-0.5 transition-transform duration-PILL dark:bg-white dark:text-black">
            <Icon name="network" size={16} />{t('home.add_upstream_gateway')}
          </NavLink>
          <NavLink to="/models" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                   border border-neutral-200 dark:border-neutral-800 hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
            <Icon name="key" size={16} />{t('home.config_client_key')}
          </NavLink>
          <NavLink to="/traces" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                   border border-neutral-200 dark:border-neutral-800 hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
            <Icon name="git-branch" size={16} />{t('home.view_traces')}
          </NavLink>
        </div>
      </PillCard>
    </div>
  );
};

export default Home;
