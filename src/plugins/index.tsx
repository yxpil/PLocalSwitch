/**
 * ============================================================
 *  插件化注册中心
 * ============================================================
 *  目标：每个功能模块（页面/侧边栏项/设置分区/卡片 widget…）以 Plugin 形式注册，
 *        单个模块加载失败不会污染主进程，失败后由 Logger 自动标记 disabled，
 *        UI 通过 <SafeRender> / <ModuleGuard> 统一屏蔽，防止白屏。
 *
 *  插件类型：
 *    - route   ：新增路由页面
 *    - sidebar ：新增侧边栏入口
 *    - widget  ：在首页或某个区域挂载一个卡片组件
 *    - setting ：设置页添加分区
 *
 *  设计：
 *    - 注册时立即执行（同步失败捕获）；异步加载时（dynamic import）在 loader 里再捕获一次
 *    - 每个 plugin 绑定自己的 Logger 实例，错误自动计数
 * ============================================================
 */
import React, { type ComponentType, type LazyExoticComponent } from 'react';
import { useTranslation } from 'react-i18next';
import { useLogger, isModuleDisabled, enableModule, getModuleState } from '@logger/index';
import i18n from '@i18n/index';
// 公开 re-export，便于设置页/模块管理查询状态
export { useLogger, isModuleDisabled, enableModule, getModuleState };
import type { IconName } from '@icons/index';

export type AnyComponent = ComponentType<any> | LazyExoticComponent<ComponentType<any>>;

export interface PluginRoute {
  path: string;
  component: AnyComponent;
  title?: string;
  priority?: number; // 越小越靠前
}
export interface PluginSidebarItem {
  key: string;
  label: string;
  to: string;
  icon?: IconName;
  badge?: React.ReactNode;
  order?: number;
}
export interface PluginWidget {
  slot: 'home.hero' | 'home.stats' | 'home.content' | 'global.topbar.right';
  component: AnyComponent;
  order?: number;
}
export interface PluginSettingSection {
  tab: string;     // 归属设置 tab key
  title: string;
  component: AnyComponent;
  order?: number;
}

export interface PLSPlugin {
  name: string;              // 模块名（用于 Logger + 屏蔽）
  version?: string;
  routes?: PluginRoute[];
  sidebar?: PluginSidebarItem[];
  widgets?: PluginWidget[];
  settings?: PluginSettingSection[];

  /** 初始化钩子（可选）。若抛错 → 自动禁用本插件 */
  setup?: () => void | Promise<void>;
}

// —— 注册表 ——
const registry: PLSPlugin[] = [];
const loaded = new Set<string>();

const rootLog = useLogger('plugins:registry');

/** 注册插件（同步安全，出错自动禁用） */
export function registerPlugin(p: PLSPlugin): boolean {
  if (loaded.has(p.name)) {
    rootLog.warn(`插件 [${p.name}] 重复注册，已跳过`);
    return false;
  }
  if (isModuleDisabled(p.name)) {
    rootLog.warn(`插件 [${p.name}] 已被标记禁用，跳过注册。可 enableModule('${p.name}') 恢复。`);
    return false;
  }
  const log = useLogger(`plugin:${p.name}`);
  try {
    if (p.setup) p.setup();
    registry.push(p);
    loaded.add(p.name);
    log.info(`已注册 v${p.version ?? '0.0.0'}`);
    return true;
  } catch (e) {
    log.error(`setup() 执行失败，插件禁用`, e instanceof Error ? e : new Error(String(e)));
    return false;
  }
}

/** 异步懒加载一个插件（ESM dynamic import） */
export async function loadPluginLazy(
  name: string,
  loader: () => Promise<{ default?: PLSPlugin } | PLSPlugin>,
): Promise<boolean> {
  if (loaded.has(name) || isModuleDisabled(name)) return false;
  const log = useLogger(`plugin:${name}`);
  try {
    const mod = await loader();
    const plugin = (mod as any)?.default ?? mod as PLSPlugin;
    return registerPlugin(plugin);
  } catch (e) {
    log.error(`动态加载失败，插件禁用`, e instanceof Error ? e : new Error(String(e)));
    return false;
  }
}

export function resetPlugin(name: string) {
  enableModule(name);
  loaded.delete(name);
  const i = registry.findIndex(p => p.name === name);
  if (i >= 0) registry.splice(i, 1);
}

export function listPlugins(): Readonly<PLSPlugin[]> { return registry; }

/* ===== 查询类 API（供布局/设置页消费） ===== */

export function getSidebarItems(): PluginSidebarItem[] {
  const all = registry.flatMap(p => p.sidebar ?? []);
  return all.sort((a, b) => (a.order ?? 999) - (b.order ?? 999));
}
export function getRoutes(): PluginRoute[] {
  const all = registry.flatMap(p => p.routes ?? []);
  return all.sort((a, b) => (a.priority ?? 999) - (b.priority ?? 999));
}
export function getWidgets(slot: PluginWidget['slot']): PluginWidget[] {
  return registry
    .flatMap(p => (p.widgets ?? []).filter(w => w.slot === slot).map(w => ({ ...w, _name: p.name })))
    .sort((a: any, b: any) => (a.order ?? 999) - (b.order ?? 999));
}
export function getSettingSections(tab: string): PluginSettingSection[] {
  return registry
    .flatMap(p => (p.settings ?? []).filter(s => s.tab === tab))
    .sort((a, b) => (a.order ?? 999) - (b.order ?? 999));
}

/* =============================================================
 *  UI 辅助：错误边界 + SafeRender（保护任何子树，失败显示占位）
 * =============================================================*/
interface ErrorBoundaryProps {
  moduleName: string;
  children: React.ReactNode;
  fallback?: (ctx: { reset: () => void; error: string }) => React.ReactNode;
}
interface ErrorBoundaryState { hasError: boolean; error?: string; }

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false };
  static getDerivedStateFromError(e: any): ErrorBoundaryState {
    return { hasError: true, error: e?.message ?? String(e) };
  }
  componentDidCatch(e: Error) {
    useLogger(`ui:${this.props.moduleName}`).error('组件崩溃', e);
  }
  reset = () => {
    enableModule(this.props.moduleName);
    this.setState({ hasError: false, error: undefined });
  };
  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback({ reset: this.reset, error: this.state.error ?? '' });
      return (
        <div className="rounded-softer border border-dashed border-neutral-300 dark:border-neutral-700
                        bg-neutral-50 dark:bg-neutral-900 p-5 text-sm">
          <div className="font-semibold text-neutral-900 dark:text-neutral-100">
            {i18n.t('common.module_shielded', { name: this.props.moduleName })}
          </div>
          <div className="mt-1 text-xs text-neutral-500 break-all">
            {this.state.error}
          </div>
          <div className="mt-3 flex gap-2">
            <button onClick={this.reset}
              className="pill-btn pill-variant-ghost !py-1.5 !px-4 text-xs">
              {i18n.t('common.reload')}
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

/**
 * <ModuleGuard name="xxx"><SomePluginOutput /></ModuleGuard>
 *  - 若模块被禁用（手动或自动）→ 渲染屏蔽占位
 *  - 若子组件内部抛错 → ErrorBoundary 兜底
 */
export const ModuleGuard: React.FC<{ name: string; children: React.ReactNode }> = ({ name, children }) => {
  const { t } = useTranslation();
  if (isModuleDisabled(name)) {
    return (
      <div className="rounded-softer border border-dashed border-neutral-300 dark:border-neutral-700
                      bg-neutral-50 dark:bg-neutral-900 p-4 text-xs text-neutral-500 flex items-center justify-between">
        <div>{t('common.module_disabled', { name })}</div>
        <button onClick={() => enableModule(name)}
          className="pill-btn pill-variant-ghost !py-1 !px-3 text-[11px]">{t('common.enabled')}</button>
      </div>
    );
  }
  return <ErrorBoundary moduleName={name}>{children}</ErrorBoundary>;
};

/** 同步/异步组件容错包装（对 dynamic import 尤其有用） */
export function SafeRender(props: { moduleName: string; children: React.ReactNode }) {
  return <ModuleGuard name={props.moduleName}>{props.children}</ModuleGuard>;
}
