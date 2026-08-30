import { create } from 'zustand';
import type { AppConfig, AppInfo, SystemInfo, DeepPartial } from '@types';
import { useLogger } from '@logger/index';
import {
  getAppInfo, getSystemInfo, ping, loadConfig, saveConfig,
} from '@commands/app';
import { setLocale } from '@i18n/index';

const log = useLogger('store:app');

interface AppState {
  appInfo: AppInfo | null;
  systemInfo: SystemInfo | null;
  config: AppConfig | null;
  dataDir: string;
  initialized: boolean;
  error: string | null;

  initApp: () => Promise<void>;
  ping: (msg?: string) => Promise<string>;
  updateConfig: (patch: DeepPartial<AppConfig>) => Promise<void>;
  setLocale: (loc: string) => Promise<void>;
  refreshSystemInfo: () => Promise<void>;
}

function fallbackConfig(): AppConfig {
  return {
    locale: 'zh-CN', theme: 'auto', primary_color: '#000000',
    animation_enabled: true, extras: {},
  };
}
function dataDirHint() {
  const p = navigator.platform.toLowerCase();
  if (p.includes('win')) return '%APPDATA%\\PLocalSwitch\\data';
  if (p.includes('mac')) return '~/Library/Application Support/com.plocalswitch.PLocalSwitch';
  return '~/.local/share/plocalswitch';
}
function deepMerge<T extends Record<string, any>>(a: T, b: Partial<T>): T {
  const out: any = { ...a };
  for (const k of Object.keys(b as Record<string, any>)) {
    const key = k as keyof T;
    const va = out[key];
    const vb = (b as any)[key];
    if (va && vb && typeof va === 'object' && typeof vb === 'object' && !Array.isArray(va) && !Array.isArray(vb)) {
      out[key] = deepMerge(va, vb);
    } else if (vb !== undefined) {
      out[key] = vb;
    }
  }
  return out as T;
}

export const useAppStore = create<AppState>((set, get) => ({
  appInfo: null, systemInfo: null, config: null,
  dataDir: dataDirHint(), initialized: false, error: null,

  async initApp() {
    try {
      set({ error: null });
      const [infoR, cfgR, sysR] = await Promise.allSettled([ getAppInfo(), loadConfig(), getSystemInfo() ]);
      if (infoR.status === 'fulfilled') set({ appInfo: infoR.value });
      if (cfgR.status  === 'fulfilled' && cfgR.value) {
        set({ config: cfgR.value });
        // 仅当配置里确实有受支持的语言码时才切换；否则保留 i18n 自身从 localStorage 读到的语言，
        // 避免每次启动都用 undefined 强制回退到 zh-CN，导致用户改的语言被覆盖。
        const loc = cfgR.value.locale;
        if (loc && ['zh-CN','zh-TW','en-US','ja-JP','ko-KR','ru-RU','de-DE','fr-FR','es-ES'].includes(loc)) {
          try { await setLocale(loc); } catch (e) { log.warn('i18n 初始化失败', e as any); }
        }
      } else {
        set({ config: fallbackConfig() });
      }
      if (sysR.status  === 'fulfilled') set({ systemInfo: sysR.value });
      set({ initialized: true, dataDir: dataDirHint() });
      log.info('应用初始化完成');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg });
      set({ config: fallbackConfig(), initialized: true });
      log.error('应用初始化部分失败，已回退默认配置', e instanceof Error ? e : undefined);
    }
  },

  async ping(msg) {
    try { return await ping(msg); }
    catch (e: any) { return `后端不可用（${String(e?.message || e)}）`; }
  },

  async updateConfig(patch) {
    const cur = get().config ?? fallbackConfig();
    const merged = deepMerge(cur, patch as Partial<AppConfig>);
    try {
      const saved = await saveConfig(merged);
      set({ config: saved });
      log.info(`配置已保存`);
    } catch (e) {
      // 后端不可用时至少更新内存
      set({ config: merged });
      log.warn('保存到后端失败，已使用内存配置', e instanceof Error ? e : undefined);
    }
    if ((patch as any)?.locale) {
      try { await setLocale((patch as any).locale); } catch { /* noop */ }
    }
  },

  async setLocale(loc: string) {
    await get().updateConfig({ locale: loc });
  },

  async refreshSystemInfo() {
    try { set({ systemInfo: await getSystemInfo() }); }
    catch (e) { log.warn('刷新系统信息失败', e as any); }
  },
}));
