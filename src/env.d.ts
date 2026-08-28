/**
 * Vue 文件类型声明
 */
declare module '*.vue' {
  import type { DefineComponent } from 'vue';
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module '*.json' {
  const value: any;
  export default value;
}

declare module '*.svg' {
  const src: string;
  export default src;
}

declare module '*.png' { const src: string; export default src; }
declare module '*.jpg' { const src: string; export default src; }
declare module '*.jpeg' { const src: string; export default src; }
declare module '*.webp' { const src: string; export default src; }
declare module '*.gif' { const src: string; export default src; }
declare module '*.ico' { const src: string; export default src; }

// Tauri 环境变量
interface ImportMetaEnv {
  readonly VITE_?: string;
  readonly TAURI_ENV_PLATFORM?: string;
  readonly TAURI_ENV_ARCH?: string;
  readonly TAURI_ENV_FAMILY?: string;
  readonly TAURI_DEBUG?: string;
}
interface ImportMeta {
  readonly env: ImportMetaEnv;
}

// Window 上的 Tauri 内部标记
interface Window {
  __TAURI_INTERNALS__?: unknown;
}
