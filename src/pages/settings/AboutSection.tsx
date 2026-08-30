import React from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@stores/app';

const AboutSection: React.FC = () => {
  const { t } = useTranslation();
  const info = useAppStore(s => s.appInfo);
  const sys  = useAppStore(s => s.systemInfo);
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
      <div className="rounded-softer border border-neutral-200/70 dark:border-neutral-800/70 p-5 space-y-2 text-sm">
        <Row label={t('about.app')}  value={info?.name ?? 'PLocalSwitch'}/>
        <Row label={t('about.version')}  value={info?.version ?? '0.1.0'}/>
        <Row label="AppID" value={info?.identifier ?? 'com.plocalswitch.app'}/>
        <Row label={t('about.request')}  value={sys ? String(sys.request_count) : '—'}/>
        <Row label={t('about.uptime')}  value={sys ? `${sys.uptime} s` : '—'}/>
      </div>
      <div className="rounded-softer border border-neutral-200/70 dark:border-neutral-800/70 p-5 space-y-2 text-sm">
        <Row label={t('about.data_dir')} value={useAppStore.getState().dataDir}/>
      </div>
    </div>
  );
};

const Row: React.FC<{ label: string; value: React.ReactNode }> = ({ label, value }) => (
  <div className="flex items-center justify-between">
    <div className="text-xs text-neutral-500">{label}</div>
    <div className="text-sm font-medium max-w-[60%] truncate">{value}</div>
  </div>
);

export default AboutSection;
