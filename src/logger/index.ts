/**
 * ============================================================
 *  前端日志系统（模块级分类 + 自动屏蔽错误模块 + Tauri 文件写入）
 * ============================================================
 *  设计：
 *   - 每个模块独立一个 Logger（带模块名 tag），可单独开启/屏蔽
 *   - ERROR 级别自动上报：若 Tauri 可用，写入沙箱目录 logs/frontend.log
 *   - 任意模块累计错误超过阈值（默认 3）→ 自动标记 disabled，
 *     由 <ModuleGuard> 组件在下一次渲染时屏蔽对应 UI 模块，防止连锁崩溃
 * ============================================================
 */
import type { ApiResponse } from '@types';

export type LogLevel = 'debug' | 'info' | 'warn' | 'error' | 'fatal';

export interface LogRecord {
  time: number;
  level: LogLevel;
  module: string;
  message: string;
  stack?: string;
  extra?: unknown;
}

const STORAGE_KEY_MODULES = 'pls.modules.state.v1';
const STORAGE_KEY_LOG     = 'pls.log.ring.v1';
const ERROR_THRESHOLD = 3;
const RING_MAX = 200;

// ============= 模块状态（持久化） =============
interface ModuleState {
  errors: number;
  disabled: boolean;      // 自动屏蔽标记（UI 层 ModuleGuard 配合）
  manuallyDisabled: boolean;
  lastError?: string;
}

function loadModules(): Record<string, ModuleState> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_MODULES);
    return raw ? JSON.parse(raw) : {};
  } catch { return {}; }
}
function saveModules(map: Record<string, ModuleState>) {
  try { localStorage.setItem(STORAGE_KEY_MODULES, JSON.stringify(map)); } catch { /* noop */ }
}

const modules: Record<string, ModuleState> = loadModules();

export function getModuleState(name: string): ModuleState {
  if (!modules[name]) modules[name] = { errors: 0, disabled: false, manuallyDisabled: false };
  return modules[name];
}

/** 手动禁用一个模块 */
export function disableModule(name: string) {
  getModuleState(name).manuallyDisabled = true;
  saveModules(modules);
}
export function enableModule(name: string) {
  const s = getModuleState(name);
  s.manuallyDisabled = false;
  s.disabled = false;
  s.errors = 0;
  s.lastError = undefined;
  saveModules(modules);
}
export function isModuleDisabled(name: string): boolean {
  const s = modules[name];
  return !!(s && (s.disabled || s.manuallyDisabled));
}

// ============= 环形内存日志（控制台 + 持久化） =============
const ring: LogRecord[] = (() => {
  try {
    const r = localStorage.getItem(STORAGE_KEY_LOG);
    return r ? JSON.parse(r) : [];
  } catch { return []; }
})();

function pushRing(r: LogRecord) {
  ring.push(r);
  if (ring.length > RING_MAX) ring.splice(0, ring.length - RING_MAX);
  try { localStorage.setItem(STORAGE_KEY_LOG, JSON.stringify(ring)); } catch { /* noop */ }
}

export function getRecentLogs(): LogRecord[] { return ring.slice(); }

// ============= 写 Tauri 后端日志文件 =============
async function flushToTauri(record: LogRecord) {
  try {
    const inTauri = !!(window as any).__TAURI_INTERNALS__
      || /Tauri\//i.test(navigator.userAgent)
      || !!import.meta.env.TAURI_ENV_PLATFORM;
    if (!inTauri) return;
    const { invoke } = await import('@tauri-apps/api/core');
    // 若 Rust 端注册了 write_text_file 命令则写入
    const line = `${new Date(record.time).toISOString()} [${record.level.toUpperCase()}] [${record.module}] ${record.message}${record.stack ? `\n    ${record.stack}` : ''}\n`;
    const resp = await invoke('write_text_file', {
      relative_path: `logs/frontend-${new Date().toISOString().slice(0, 10)}.log`,
      content: line,
    }) as ApiResponse<boolean> | boolean;
    void resp;
  } catch { /* 后端不可用时静默 */ }
}

// ============= Logger =============
export class Logger {
  constructor(public readonly name: string) {}

  private emit(level: LogLevel, message: string, extra?: unknown) {
    const record: LogRecord = {
      time: Date.now(), level, module: this.name, message, extra,
    };
    // 堆栈
    if (level === 'error' || level === 'fatal') {
      const s = (extra instanceof Error ? extra : undefined)?.stack;
      if (s) record.stack = s;
    }
    // 控制台（保持原生方法以便浏览器格式化）
    const args: any[] = [
      `%c[${this.name}]`,
      'color:#888;font-weight:600;',
      message,
    ];
    if (extra !== undefined) args.push(extra);
    switch (level) {
      case 'debug': console.debug.apply(console, args as any); break;
      case 'info':  console.info.apply(console, args as any);  break;
      case 'warn':  console.warn.apply(console, args as any);  break;
      case 'error': console.error.apply(console, args as any); break;
      case 'fatal': console.error.apply(console, args as any); break;
    }
    pushRing(record);

    if (level === 'error' || level === 'fatal') {
      // 统计错误并自动屏蔽
      const s = getModuleState(this.name);
      s.errors += 1;
      s.lastError = message;
      if (s.errors >= ERROR_THRESHOLD) {
        s.disabled = true;
        console.warn(`[Logger] 模块 ${this.name} 错误次数超过阈值，已自动屏蔽。可调用 enableModule('${this.name}') 恢复。`);
      }
      saveModules(modules);
      // 异步写到后端日志
      flushToTauri(record);
    }
  }

  debug(msg: string, extra?: unknown) { this.emit('debug', msg, extra); }
  info (msg: string, extra?: unknown) { this.emit('info',  msg, extra); }
  warn (msg: string, extra?: unknown) { this.emit('warn',  msg, extra); }
  error(msg: string, extra?: unknown) { this.emit('error', msg, extra); }
  fatal(msg: string, extra?: unknown) { this.emit('fatal', msg, extra); }
}

// 全局 logger 缓存（避免重复 new）
const pool = new Map<string, Logger>();
export function useLogger(module: string): Logger {
  let l = pool.get(module);
  if (!l) { l = new Logger(module); pool.set(module, l); }
  return l;
}

/**
 * 窗口级错误捕获 → 作为 ui:app 模块日志
 */
if (typeof window !== 'undefined') {
  const log = useLogger('ui:app');
  window.addEventListener('error', (e) => {
    log.error(
      `window.error: ${e.message} @ ${String(e.filename || '')}:${e.lineno}:${e.colno}`,
      e.error ?? undefined,
    );
  });
  window.addEventListener('unhandledrejection', (e) => {
    const reason = (e.reason instanceof Error) ? e.reason : new Error(String(e.reason));
    log.error(`unhandledrejection: ${reason.message}`, reason);
  });
}
