/**
 * IPC 基础封装（Tauri / 纯浏览器 mock 降级，零彩色语义）
 */
import type { ApiResponse } from '@types';
import { useLogger } from '@logger/index';

const log = useLogger('ipc');

export function isTauriEnv(): boolean {
  if (typeof window === 'undefined') return false;
  return !!(window as any).__TAURI_INTERNALS__
    || /Tauri\//i.test(navigator.userAgent)
    || !!import.meta.env.TAURI_ENV_PLATFORM;
}

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const t0 = Date.now();
  if (!isTauriEnv()) {
    // 本应用为 Tauri 桌面应用：非 Tauri 环境直接报错，不提供任何 mock/demo 数据
    const err = new Error(`命令 ${cmd} 仅在桌面应用内可用（当前非 Tauri 环境）`);
    log.error('[no-tauri]', err);
    throw err;
  }
  try {
    const { invoke: raw } = await import('@tauri-apps/api/core');
    const resp = await raw(cmd, args) as ApiResponse<T> | T;
    log.debug(`[ok] ${cmd} (${Date.now() - t0}ms)`);
    return unwrap(resp);
  } catch (e) {
    log.error(`[fail] ${cmd}`, e instanceof Error ? e : new Error(String(e)));
    throw e;
  }
}

function unwrap<T>(resp: ApiResponse<T> | T): T {
  if (resp && typeof resp === 'object' && 'success' in (resp as object)) {
    const r = resp as ApiResponse<T>;
    if (!r.success) {
      const err = new Error(r.message || '后端业务失败');
      (err as any).code = (r as any).code || 'BIZ';
      throw err;
    }
    return r.data as T;
  }
  return resp as T;
}
