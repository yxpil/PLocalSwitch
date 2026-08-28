import React from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@utils/cn';
import { NavLink, useLocation } from 'react-router-dom';
import Icon, { type IconName } from '@icons/index';
import { PillBadge } from '@components/index';
import { getSidebarItems } from '@plugins/index';
import { useThemeStore } from '@stores/theme';
import { useAppStore } from '@stores/app';

const Sidebar: React.FC = () => {
  const { t } = useTranslation();
  const items = getSidebarItems();
  const { isDark, toggle } = useThemeStore();
  const location = useLocation();
  const appInfo = useAppStore(s => s.appInfo);

  return (
    <aside
      className="hidden md:flex h-full w-[240px] shrink-0 flex-col
                 border-r border-neutral-200/70 dark:border-neutral-800/70
                 bg-white/60 dark:bg-black/40 backdrop-blur-md">
      {/* Logo */}
      <div className="px-6 py-5 border-b border-neutral-200/70 dark:border-neutral-800/70">
        <NavLink to="/" className="flex items-center gap-3 group">
          <div className={cn(
            'h-11 w-11 rounded-pill shrink-0',
            'bg-gradient-pill dark:bg-gradient-pill-dark shadow-pill',
            'flex items-center justify-center',
            'transition-transform duration-PILL ease-PILL group-hover:rotate-[-4deg] group-hover:scale-105',
          )}>
            <img src="/logo.png" alt="logo" className="h-6 w-6 rounded-full bg-white dark:bg-black" />
          </div>
          <div className="min-w-0">
            <div className="font-bold text-neutral-900 dark:text-neutral-100 tracking-tight truncate">
              PLocalSwitch
            </div>
            <div className="text-[11px] text-neutral-500 dark:text-neutral-400 truncate">
              v{appInfo?.version ?? '0.1.0'}
            </div>
          </div>
        </NavLink>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-3 py-4 space-y-1 overflow-y-auto">
        {items.map((it) => {
          const active = it.to === '/' ? location.pathname === '/' : location.pathname.startsWith(it.to);
          return (
            <NavLink key={it.key} to={it.to}
              className={cn(
                'flex items-center gap-3 rounded-pill px-4 py-2.5 text-sm font-medium',
                'transition-all duration-PILL ease-PILL',
                active
                  ? 'bg-neutral-200 text-neutral-900 dark:bg-neutral-800 dark:text-white'
                  : 'text-neutral-700 hover:bg-neutral-100 hover:text-neutral-900 dark:text-neutral-200 dark:hover:bg-neutral-900 dark:hover:text-white',
              )}>
              {it.icon && <Icon name={it.icon as IconName} size={18}/>}
              <span className="flex-1 truncate">{t(it.label)}</span>
              {it.badge && <PillBadge variant="muted" size="sm">{it.badge}</PillBadge>}
            </NavLink>
          );
        })}
      </nav>

      {/* Footer */}
      <div className="px-3 py-4 border-t border-neutral-200/70 dark:border-neutral-800/70 space-y-2">
        <button
          onClick={toggle}
          className="w-full flex items-center justify-between rounded-pill px-4 py-2.5 text-sm font-medium
                     text-neutral-700 hover:text-neutral-900 hover:bg-neutral-100
                     dark:text-neutral-200 dark:hover:text-white dark:hover:bg-neutral-900
                     transition-all duration-PILL ease-PILL"
        >
          <span className="flex items-center gap-2">
            <Icon name={isDark ? 'moon' : 'sun'} size={16} />
            <span>{t(isDark ? 'shell.theme_dark' : 'shell.theme_light')}</span>
          </span>
          <Icon name="chevron-right" size={14}/>
        </button>
        <NavLink to="/settings"
          className="flex items-center gap-2 text-xs text-neutral-600 dark:text-neutral-300 px-4 py-1.5 hover:text-neutral-900 dark:hover:text-white">
          <Icon name="info" size={14}/> <span>{t('shell.about_info')}</span>
        </NavLink>
      </div>
    </aside>
  );
};

export default Sidebar;
