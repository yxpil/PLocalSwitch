import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillBadge from '@components/ui/PillBadge';
import Icon from '@icons/index';
import { useAppStore } from '@stores/app';
import { listPlugins } from '@plugins/index';
import { NavLink } from 'react-router-dom';
import { invoke } from '@commands/index';
import { accessHost } from '../../utils/net';

const HeroWidget: React.FC = () => {
  const { t } = useTranslation();
  const info = useAppStore(s => s.appInfo);
  const [now, setNow] = useState(new Date());
  const [listenAddr, setListenAddr] = useState('');
  useEffect(() => {
    const tm = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(tm);
  }, []);
  useEffect(() => {
    (async () => {
      try {
        const cfg: any = await invoke('load_config');
        setListenAddr(accessHost(cfg?.http?.listen));
      } catch { /* 无后端时忽略 */ }
    })();
  }, []);
  const plugins = listPlugins().length;

  return (
    <div className="rounded-softer p-6 md:p-8 relative overflow-hidden
                    bg-white dark:bg-neutral-950
                    border border-neutral-200/70 dark:border-neutral-800/70 shadow-card
                    transition-all duration-300 ease-PILL hover:-translate-y-0.5 hover:shadow-card-hover">
      <div className="relative flex flex-col md:flex-row items-start gap-6">
        <div className="flex items-center gap-4 min-w-0">
          <div className="h-20 w-20 shrink-0 rounded-pill bg-gradient-pill dark:bg-gradient-pill-dark shadow-pill
                          flex items-center justify-center">
            <img src="/logo.png" alt="logo" className="h-12 w-12 rounded-full bg-white dark:bg-black"/>
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-2xl font-bold truncate">{info?.name ?? 'PLocalSwitch'} Gateway</span>
              <PillBadge variant="pass" dot>v{info?.version ?? '0.2.0'}</PillBadge>
              <PillBadge variant="neutral">{t('home.hero_modules', { count: plugins })}</PillBadge>
              <PillBadge variant="warn">{t('home.hero_adaptive')}</PillBadge>
            </div>
            <div className="mt-1 text-sm text-neutral-500 truncate">
              {t('home.hero_subtitle')}
            </div>
            <div className="mt-2 text-xs text-neutral-500 tabular-nums">
              {listenAddr ? <span className="font-mono">{listenAddr}</span> : t('home.listen_placeholder')}
            </div>
          </div>
        </div>
        <div className="shrink-0 md:ml-auto md:text-right">
          <div className="text-xs uppercase tracking-wider text-neutral-500 mb-1 whitespace-nowrap">{t('home.local_time')}</div>
          <div className="text-3xl md:text-4xl font-black tabular-nums tracking-tight whitespace-nowrap">
            {now.toLocaleTimeString()}
          </div>
        </div>
      </div>

      <div className="relative mt-6 flex flex-wrap gap-2">
        <NavLink to="/models" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                 bg-neutral-900 text-white dark:bg-white dark:text-black shadow-pill
                 hover:-translate-y-0.5 transition-transform duration-PILL ease-PILL">
          <Icon name="network" size={16}/>{t('home.add_upstream_gateway')}
        </NavLink>
        <NavLink to="/traces" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                 border border-neutral-200 dark:border-neutral-800 hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
          <Icon name="wallet" size={16}/>{t('home.view_billing')}
        </NavLink>
        <NavLink to="/chat" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                 border border-neutral-200 dark:border-neutral-800 hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
          <Icon name="message-circle" size={16}/>{t('nav.chat')}
        </NavLink>
        <NavLink to="/settings" className="inline-flex items-center gap-2 rounded-pill px-4 py-2 text-sm
                 border border-neutral-200 dark:border-neutral-800 hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
          <Icon name="settings" size={16}/>{t('home.gateway_config')}
        </NavLink>
      </div>
    </div>
  );
};

export default HeroWidget;
