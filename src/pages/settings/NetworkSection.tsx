import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillInput from '@components/ui/PillInput';
import PillButton from '@components/ui/PillButton';
import PillSwitch from '@components/ui/PillSwitch';
import Icon from '@icons/index';
import { invoke } from '@commands/index';

const NetworkSection: React.FC = () => {
  const { t } = useTranslation();
  const [enable, setEnable] = useState(false);
  const [http, setHttp]   = useState('http://127.0.0.1:7890');
  const [socks, setSocks] = useState('socks5://127.0.0.1:1080');
  const [bypass, setBypass] = useState('localhost,127.0.0.1,.local');
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 加载真实代理配置（由后端 gateway.yaml 存储）
  const load = async () => {
    try {
      const s: any = await invoke('get_proxy_settings');
      setEnable(!!s?.enable);
      setHttp(s?.http || '');
      setSocks(s?.socks || '');
      setBypass(s?.bypass || '');
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => { load(); }, []);

  const save = async () => {
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const s: any = await invoke('set_proxy_settings', {
        setting: { enable, http, socks, bypass },
      });
      setEnable(!!s?.enable);
      setHttp(s?.http || '');
      setSocks(s?.socks || '');
      setBypass(s?.bypass || '');
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="rounded-[1.5rem] border border-neutral-200/70 dark:border-neutral-800/70 px-5 py-3">
        <PillSwitch checked={enable} onChange={setEnable} label={t('network.enable_proxy')} description={t('network.proxy_hint')}/>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
        <PillInput label={t('network.http_proxy')} value={http}   onChange={(e) => setHttp(e.target.value)}
          prefix={<Icon name="globe" size={16}/>}/>
        <PillInput label={t('network.socks5_proxy')}  value={socks}  onChange={(e) => setSocks(e.target.value)}/>
        <PillInput label={t('network.bypass')}  value={bypass} onChange={(e) => setBypass(e.target.value)}
          hint={t('network.bypass_hint')} className="md:col-span-2"/>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <PillButton onClick={save} disabled={saving} leftIcon={<Icon name="save" size={16}/>}>
          {saving ? t('network.saving') : t('network.save_apply')}
        </PillButton>
        <PillButton variant="soft" onClick={load} disabled={saving} leftIcon={<Icon name="rotate-ccw" size={16}/>}>{t('network.rollback')}</PillButton>
        {saved && <span className="text-xs text-emerald-500">{t('network.saved_ok')}</span>}
        {error && <span className="text-xs text-red-500">{error}</span>}
      </div>
    </div>
  );
};

export default NetworkSection;
