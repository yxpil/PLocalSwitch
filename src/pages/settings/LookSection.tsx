import React from 'react';
import { useTranslation } from 'react-i18next';
import PillSelect from '@components/ui/PillSelect';
import PillSwitch from '@components/ui/PillSwitch';
import PillCard from '@components/ui/PillCard';
import { useThemeStore } from '@stores/theme';
import { useAppStore } from '@stores/app';

const LookSection: React.FC = () => {
  const { t } = useTranslation();
  const mode = useThemeStore(s => s.mode);
  const set  = useThemeStore(s => s.set);
  const anim = useAppStore(s => s.config?.animation_enabled ?? true);
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
      <PillSelect label={t('look.theme_mode')} value={mode} onChange={(v) => set(v as any)}
        options={[
          { value: 'auto',  label: t('look.system') },
          { value: 'light', label: t('look.light') },
          { value: 'dark',  label: t('look.dark') },
        ]}
        hint={t('look.hint')}/>
      <div className="rounded-[1.5rem] border border-neutral-200/70 dark:border-neutral-800/70 px-5 py-3">
        <PillSwitch checked={anim} label={t('look.compact')} description={t('look.compact_desc')} onChange={() => {}}/>
      </div>
      <div className="md:col-span-2 grid grid-cols-2 gap-3">
        <PillCard padding="md" hoverable={false}>
          <div className="h-10 rounded-pill bg-gradient-pill shadow-pill mb-2"/>
          <div className="text-xs text-neutral-500">{t('look.color_light')}</div>
        </PillCard>
        <PillCard padding="md" hoverable={false}>
          <div className="h-10 rounded-pill bg-gradient-pill-dark shadow-pill mb-2"/>
          <div className="text-xs text-neutral-500">{t('look.color_dark')}</div>
        </PillCard>
      </div>
    </div>
  );
};

export default LookSection;
