/** 全局类型（React 版）*/

export type ThemeMode = 'light' | 'dark' | 'auto';

export interface AppConfig {
  locale: string;
  theme: ThemeMode;
  primary_color: string;
  animation_enabled: boolean;
  extras: Record<string, unknown>;
}

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  message: string;
  timestamp: number;
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
}

export interface SystemInfo {
  app_version: string;
  rust_version: string;
  os: string;
  arch: string;
  uptime: number;
  request_count: number;
}

export interface FileItem {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  modified_at: number;
}

export type ToastType = 'info' | 'success' | 'warning' | 'error';
export type Maybe<T>   = T | null | undefined;
export type DeepPartial<T> = T extends object
  ? { [P in keyof T]?: DeepPartial<T[P]> }
  : T;
