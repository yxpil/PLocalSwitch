import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { NavLink, useLocation } from 'react-router-dom';
import Icon, { type IconName } from '@icons/index';
import { PillBadge, PillButton, PillModal } from '@components/index';
import { getSidebarItems } from '@plugins/index';
import { useThemeStore } from '@stores/theme';
import { useAppStore } from '@stores/app';

const Topbar: React.FC = () => {
  const { t } = useTranslation();
  const items = getSidebarItems();
  const { isDark, toggle } = useThemeStore();
  const loc = useLocation();
  const appInfo = useAppStore(s => s.appInfo);

  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <>
      <header className="h-16 shrink-0 flex items-center gap-4 px-5 md:px-8
                         border-b border-neutral-200/70 dark:border-neutral-800/70
                         bg-white/60 dark:bg-black/40 backdrop-blur-sm sticky top-0 z-30">
        {/* Mobile menu */}
        <button
          className="md:hidden rounded-full p-2 hover:bg-neutral-100 dark:hover:bg-neutral-900
                     transition-colors text-neutral-700 dark:text-neutral-300"
          onClick={() => setMobileOpen(true)} aria-label={t('shell.menu')}>
          <Icon name="menu" size={20}/>
        </button>

        {/* Breadcrumb-ish title */}
        <div className="flex-1 min-w-0 flex items-center gap-3">
          <div className="hidden md:flex items-center gap-2">
            <div className="h-9 w-9 rounded-pill bg-gradient-pill dark:bg-gradient-pill-dark shadow-pill
                            flex items-center justify-center">
              <img src="/logo.png" alt="logo" className="h-5 w-5 rounded-full bg-white dark:bg-black"/>
            </div>
            <div className="flex flex-col leading-tight">
              <div className="text-base font-bold text-neutral-900 dark:text-neutral-100">PLocalSwitch</div>
              <div className="text-[10px] text-neutral-500 dark:text-neutral-400">v{appInfo?.version ?? '0.1.0-dev'}</div>
            </div>
          </div>
          <span className="hidden md:inline-block text-neutral-300 dark:text-neutral-700 mx-2">/</span>
          <div className="text-sm text-neutral-600 dark:text-neutral-300 truncate">
            {(() => { const l = items.find(i => i.to === '/' ? loc.pathname === '/' : loc.pathname.startsWith(i.to))?.label; return l ? t(l) : ''; })()}
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-2">
          <button onClick={toggle} aria-label={isDark ? t('shell.switch_to_light') : t('shell.switch_to_dark')}
            className="rounded-full p-2 text-neutral-600 dark:text-neutral-300
                       hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
            <Icon name={isDark ? 'moon' : 'sun'} size={18}/>
          </button>
          <NavLink to="/settings" aria-label={t('shell.settings')}
            className="rounded-full p-2 text-neutral-600 dark:text-neutral-300
                       hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
            <Icon name="settings" size={18}/>
          </NavLink>
          <NavLink to="/settings" aria-label={t('shell.about')}
            className="rounded-full p-2 text-neutral-600 dark:text-neutral-300
                       hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-colors">
            <Icon name="info" size={18}/>
          </NavLink>
        </div>
      </header>

      {/* Mobile Sidebar Modal */}
      <PillModal open={mobileOpen} onClose={() => setMobileOpen(false)}
        size="sm" closeOnMask showClose title={t('shell.nav')}>
        <div className="space-y-1">
          {items.map((it) => {
            const active = it.to === '/' ? loc.pathname === '/' : loc.pathname.startsWith(it.to);
            return (
              <NavLink key={it.key} to={it.to}
                onClick={() => setMobileOpen(false)}
                className={cnp(
                  'flex items-center gap-3 rounded-pill px-4 py-2.5 text-sm font-medium',
                  active
                    ? 'text-white bg-neutral-900 shadow-pill dark:text-black dark:bg-white'
                    : 'text-neutral-700 hover:bg-neutral-100 hover:text-neutral-900 dark:text-neutral-200 dark:hover:bg-neutral-900 dark:hover:text-white',
                )}>
                {it.icon && <Icon name={it.icon as IconName} size={18}/>}
                <span className="flex-1 truncate">{t(it.label)}</span>
                {it.badge && <PillBadge variant="muted" size="sm">{it.badge}</PillBadge>}
              </NavLink>
            );
          })}
          <div className="mt-5 pt-4 border-t border-neutral-200/70 dark:border-neutral-800/70">
            <PillButton size="sm" variant="soft" leftIcon={<Icon name={isDark ? 'moon' : 'sun'} size={14}/>}
              onClick={toggle} full>
              {t(isDark ? 'shell.switch_to_light' : 'shell.switch_to_dark')}
            </PillButton>
          </div>
        </div>
      </PillModal>
    </>
  );
};

function cnp(...v: any[]) { return v.filter(Boolean).join(' '); }

export default Topbar;
