import { create } from 'zustand';
import type { ThemeMode } from '@types';
import { useLogger } from '@logger/index';

const STORE_KEY = 'pls.theme.v1';
const DENSITY_KEY = 'pls.density.v1';
const log = useLogger('store:theme');

interface ThemeState {
  mode: ThemeMode;
  isDark: boolean;
  /** 紧凑密度（真实生效：html.compact 根字号 14px，全局 rem 缩放） */
  compact: boolean;
  init: () => void;
  toggle: () => void;
  set: (m: ThemeMode) => void;
  setCompact: (v: boolean) => void;
  _apply: () => void;
}

function readMode(): ThemeMode {
  try {
    const v = localStorage.getItem(STORE_KEY) as ThemeMode | null;
    if (v === 'light' || v === 'dark' || v === 'auto') return v;
  } catch { /* noop */ }
  return 'auto';
}
function readCompact(): boolean {
  try { return localStorage.getItem(DENSITY_KEY) === 'compact'; } catch { return false; }
}
function calcDark(m: ThemeMode): boolean {
  if (m === 'dark') return true;
  if (m === 'light') return false;
  try { return window.matchMedia('(prefers-color-scheme: dark)').matches; }
  catch { return false; }
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  mode: readMode(),
  isDark: calcDark(readMode()),
  compact: readCompact(),

  init() {
    try {
      // 跟随系统
      if (window.matchMedia) {
        const mql = window.matchMedia('(prefers-color-scheme: dark)');
        const onChange = () => {
          if (get().mode === 'auto') get()._apply();
        };
        if (typeof mql.addEventListener === 'function') mql.addEventListener('change', onChange);
        else (mql as any).addListener(onChange);
      }
    } catch (e) { log.warn('初始化主题监听失败', e instanceof Error ? e : undefined); }
    get()._apply();
  },

  toggle() {
    const next: ThemeMode = get().isDark ? 'light' : 'dark';
    get().set(next);
  },

  set(m: ThemeMode) {
    try { localStorage.setItem(STORE_KEY, m); } catch { /* noop */ }
    set({ mode: m });
    get()._apply();
  },

  setCompact(v: boolean) {
    try { localStorage.setItem(DENSITY_KEY, v ? 'compact' : 'comfortable'); } catch { /* noop */ }
    set({ compact: v });
    get()._apply();
  },

  _apply() {
    const isDark = calcDark(get().mode);
    set({ isDark });
    try {
      const root = document.documentElement;
      if (isDark) root.classList.add('dark');
      else root.classList.remove('dark');
      // 紧凑密度：根字号 16px → 14px，Tailwind rem 工具类全局等比收紧
      if (get().compact) root.classList.add('compact');
      else root.classList.remove('compact');
      root.style.colorScheme = isDark ? 'dark' : 'light';
      log.info(`主题已应用: mode=${get().mode}, dark=${isDark}, compact=${get().compact}`);
    } catch (e) { log.warn('应用主题失败', e instanceof Error ? e : undefined); }
  },
}));
