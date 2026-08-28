import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillButton from '@components/ui/PillButton';
import PillBadge from '@components/ui/PillBadge';
import Icon from '@icons/index';
import { useLogger, getRecentLogs } from '@logger/index';

const ModulesSection: React.FC = () => {
  const { t } = useTranslation();
  const [, tick] = useState(0);
  const force = () => tick(x => x + 1);
  useLogger('ui:modules');
  const recentLogs = getRecentLogs().slice(-6).reverse();

  return (
    <div className="space-y-5">
      <div className="rounded-softer border border-dashed border-neutral-300 dark:border-neutral-700 p-4 text-xs text-neutral-500">
        <div className="flex items-start gap-2">
          <Icon name="shield-alert" size={16}/>
          <span dangerouslySetInnerHTML={{ __html: t('modules.desc1') }}/>
        </div>
      </div>

      {/* 最近日志 */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <div className="font-semibold text-sm">{t('modules.recent_logs')}</div>
          <PillButton size="sm" variant="ghost" leftIcon={<Icon name="refresh-cw" size={12}/>} onClick={force}>{t('modules.refresh')}</PillButton>
        </div>
        <div className="rounded-softer bg-neutral-50 dark:bg-neutral-900 border border-neutral-200/70 dark:border-neutral-800/70 p-3 max-h-80 overflow-auto">
          {recentLogs.length === 0 && (
            <div className="text-xs text-neutral-500 py-3 text-center">{t('modules.no_logs')}</div>
          )}
          {recentLogs.map((r, i) => (
            <div key={i} className="text-xs font-mono py-1 border-b last:border-b-0 border-neutral-200/60 dark:border-neutral-800/60 flex gap-2">
              <span className="text-neutral-500 tabular-nums shrink-0">{new Date(r.time).toLocaleTimeString()}</span>
              <PillBadge variant={
                r.level === 'error' || r.level === 'fatal' ? 'fail'
                : r.level === 'warn' ? 'warn'
                : r.level === 'info' ? 'pass' : 'muted'
              } size="sm" className="shrink-0">{r.level.toUpperCase()}</PillBadge>
              <span className="text-neutral-600 dark:text-neutral-300 shrink-0">[{r.module}]</span>
              <span className="truncate">{r.message}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

export default ModulesSection;
