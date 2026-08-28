import React from 'react';
import { useTranslation } from 'react-i18next';
import PillSelect from '@components/ui/PillSelect';
import { setLocale as setI18nLocale } from '@i18n/index';

const LOCALE_OPTS = [
  { value: 'zh-CN', label: '简体中文 (zh-CN)' },
  { value: 'zh-TW', label: '繁體中文 (zh-TW)' },
  { value: 'en-US', label: 'English (en-US)' },
  { value: 'ja-JP', label: '日本語 (ja-JP)' },
  { value: 'ko-KR', label: '한국어 (ko-KR)' },
  { value: 'ru-RU', label: 'Русский (ru-RU)' },
  { value: 'de-DE', label: 'Deutsch (de-DE)' },
  { value: 'fr-FR', label: 'Français (fr-FR)' },
  { value: 'es-ES', label: 'Español (es-ES)' },
];

const GeneralSection: React.FC = () => {
  // 语言走 i18n（localStorage 持久化），与网关配置解耦，避免 locale 被网关配置丢弃
  const { t, i18n } = useTranslation();
  const locale = i18n.language as string;

  return (
    <div className="grid grid-cols-1 gap-5">
      <PillSelect label={t('settings.locale')}
        value={locale}
        onChange={(v) => setI18nLocale(String(v))}
        options={LOCALE_OPTS}
        hint={t('settings.locale_hint')}
      />
    </div>
  );
};

export default GeneralSection;
