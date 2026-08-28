import React, { Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillTabs, { type PillTabItem } from '@components/ui/PillTabs';
import { getSettingSections } from '@plugins/index';
import { SafeRender } from '@plugins/index';

const TAB_DEFS = [
  { key: 'general', labelKey: 'settings.tab_general' },
  { key: 'network', labelKey: 'settings.tab_network' },
  { key: 'modules', labelKey: 'settings.tab_modules' },
  { key: 'about',   labelKey: 'settings.tab_about' },
];

const FallbackSection = () => (
  <div className="animate-pulse h-28 rounded-softer bg-neutral-100 dark:bg-neutral-900 border border-dashed border-neutral-200 dark:border-neutral-800"/>
);

const Settings: React.FC = () => {
  const { t } = useTranslation();
  const [tab, setTab] = useState('general');
  const sections = getSettingSections(tab);
  const TABS: PillTabItem[] = TAB_DEFS.map(d => ({ key: d.key, label: t(d.labelKey) }));

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold">{t('settings.title')}</h2>
          <p className="text-sm text-neutral-500 mt-1">{t('settings.description')}</p>
        </div>
        <PillTabs value={tab} onChange={(v) => setTab(String(v))} items={TABS} variant="soft"/>
      </div>

      <div className="grid grid-cols-1 gap-4">
        {sections.length === 0 && (
          <PillCard padding="md">
            <div className="text-sm text-neutral-500">{t('settings.empty')}</div>
          </PillCard>
        )}
        {sections.map((s, i) => {
          // s.component 为 LazyExoticComponent（builtin 已 lazy），直接渲染避免二次 lazy
          const Comp = s.component as any;
          return (
            <PillCard key={`${tab}-${i}`} padding="md" hoverable={false}
              header={<div className="font-semibold">{t(s.title)}</div>}>
              <SafeRender moduleName={`settings:${tab}:${s.title}`}>
                <Suspense fallback={<FallbackSection/>}>
                  <Comp/>
                </Suspense>
              </SafeRender>
            </PillCard>
          );
        })}
      </div>
    </div>
  );
};

export default Settings;
