import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import zhCN from '../locales/lang/zh-CN.json';
import enUS from '../locales/lang/en-US.json';
import zhTW from '../locales/lang/zh-TW.json';
import jaJP from '../locales/lang/ja-JP.json';
import koKR from '../locales/lang/ko-KR.json';
import ruRU from '../locales/lang/ru-RU.json';
import deDE from '../locales/lang/de-DE.json';
import frFR from '../locales/lang/fr-FR.json';
import esES from '../locales/lang/es-ES.json';
import { useLogger } from '@logger/index';

const log = useLogger('i18n');

export type LocaleKey =
  | 'zh-CN' | 'en-US' | 'zh-TW' | 'ja-JP'
  | 'ko-KR' | 'ru-RU' | 'de-DE' | 'fr-FR' | 'es-ES';

export const SUPPORTED_LOCALES: LocaleKey[] = [
  'zh-CN', 'zh-TW', 'en-US', 'ja-JP', 'ko-KR', 'ru-RU', 'de-DE', 'fr-FR', 'es-ES',
];

// 明确的初始语言：优先读取上次选择（localStorage），否则回退 zh-CN。
// 绝不依赖 navigator 返回的短码（如 zh / en），避免命中不到资源而显示成 raw key。
function initialLocale(): LocaleKey {
  try {
    const stored = localStorage.getItem('pls-locale');
    if (stored && (SUPPORTED_LOCALES as string[]).includes(stored)) return stored as LocaleKey;
  } catch { /* 无 localStorage（如 SSR/受限环境） */ }
  return 'zh-CN';
}

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      'zh-CN': { translation: zhCN },
      'zh-TW': { translation: zhTW },
      'en-US': { translation: enUS },
      'ja-JP': { translation: jaJP },
      'ko-KR': { translation: koKR },
      'ru-RU': { translation: ruRU },
      'de-DE': { translation: deDE },
      'fr-FR': { translation: frFR },
      'es-ES': { translation: esES },
    },
    // 显式 lng：让初始语言确定是我们资源里有的 code，避免检测出的短码导致 raw key
    lng: initialLocale(),
    fallbackLng: 'zh-CN',
    supportedLngs: SUPPORTED_LOCALES,
    load: 'currentOnly',
    ns: ['translation'],
    defaultNS: 'translation',
    // 资源内联，用同步初始化：确保 useTranslation 首次渲染即就绪，避免显示成翻译 key
    initImmediate: false,
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'pls-locale',
    },
    interpolation: { escapeValue: false },
  })
  .catch((e) => log.error('i18n init 失败', e instanceof Error ? e : new Error(String(e))));

export async function setLocale(loc: string): Promise<void> {
  const safe: LocaleKey =
    (SUPPORTED_LOCALES as string[]).includes(loc) ? (loc as LocaleKey) : 'zh-CN';
  try {
    await i18n.changeLanguage(safe);
    try { localStorage.setItem('pls-locale', safe); } catch { /* noop */ }
    if (typeof document !== 'undefined') document.documentElement.setAttribute('lang', safe);
    log.info(`语言已切换: ${safe}`);
  } catch (e) {
    log.error('切换语言失败', e instanceof Error ? e : undefined);
  }
}
export function getLocale(): LocaleKey { return i18n.language as LocaleKey; }

export default i18n;
