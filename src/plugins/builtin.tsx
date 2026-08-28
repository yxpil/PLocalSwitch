/**
 * ============================================================
 *  PLocalSwitch LLM 网关管理端：内建插件注册
 *  导航、路由、Home Widget、Settings Tab 全部插件化注入，
 *  以后新增模块只需追加 registerPlugin，无需改动核心。
 * ============================================================
 */
import React, { lazy, Suspense } from 'react';
import { registerPlugin, SafeRender } from './index';
import type { PLSPlugin } from './index';

const Dashboard   = lazy(() => import('@pages/Home'));     // / Dashboard
const Models      = lazy(() => import('@pages/Switch'));   // /models 模型别名 + 节点路由
const Billing     = lazy(() => import('@pages/Storage'));  // /billing 账本 + 对账报表
const Traces      = lazy(() => import('@pages/About'));    // /traces 链路追踪查询
const Settings    = lazy(() => import('@pages/Settings')); // /settings 设置（含网关配置）
const Chat        = lazy(() => import('@pages/Chat'));     // /chat 聊天框（Markdown，选模型）

function Suspended(name: string, C: React.LazyExoticComponent<React.ComponentType<any>>): React.ComponentType {
  const Wrapper: React.FC = () => (
    <SafeRender moduleName={name}>
      <Suspense fallback={
        <div className="animate-pulse h-40 rounded-softer bg-neutral-100 dark:bg-neutral-900 border border-dashed border-neutral-200 dark:border-neutral-800"/>
      }>
        <C />
      </Suspense>
    </SafeRender>
  );
  Wrapper.displayName = `Suspended(${name})`;
  return Wrapper;
}

registerPlugin({
  name: 'gateway-core',
  version: '0.2.0',
  routes: [
    { path: '/',         component: Suspended('page:dashboard', Dashboard), title: 'nav.dashboard', priority: 0 },
    { path: '/models',   component: Suspended('page:models',    Models),    title: 'nav.models',    priority: 1 },
    { path: '/chat',     component: Suspended('page:chat',      Chat),      title: 'nav.chat',      priority: 2 },
    { path: '/billing',  component: Suspended('page:billing',   Billing),   title: 'nav.billing',   priority: 3 },
    { path: '/traces',   component: Suspended('page:traces',    Traces),    title: 'nav.traces',    priority: 4 },
    { path: '/settings', component: Suspended('page:settings',  Settings),  title: 'nav.settings',  priority: 98 },
  ],
  sidebar: [
    { key: 'nav:dashboard', label: 'nav.dashboard', to: '/',        icon: 'layout-dashboard',  order: 0 },
    { key: 'nav:models',    label: 'nav.models',    to: '/models',  icon: 'network',           badge: 'CORE', order: 1 },
    { key: 'nav:chat',      label: 'nav.chat',      to: '/chat',    icon: 'message-circle',    order: 2 },
    { key: 'nav:billing',   label: 'nav.billing',   to: '/billing', icon: 'receipt',           badge: 'BETA', order: 3 },
    { key: 'nav:traces',    label: 'nav.traces',    to: '/traces',  icon: 'git-branch',        order: 4 },
    { key: 'nav:settings',  label: 'nav.settings',  to: '/settings',icon: 'settings',          order: 98 },
  ],
});

// 主横幅：网关欢迎卡片（Dashboard 顶部 slot）
registerPlugin({
  name: 'dashboard-hero',
  version: '0.2.0',
  widgets: [{ slot: 'home.hero', component: lazy(() => import('@pages/widgets/HeroWidget')), order: 0 }],
});

// 统计卡片：Dashboard 中部 4 卡（实时吞吐 / P99 / Today Cost / 健康节点数）
registerPlugin({
  name: 'dashboard-stats',
  version: '0.2.0',
  widgets: [{ slot: 'home.stats', component: lazy(() => import('@pages/widgets/StatsWidget')), order: 0 }],
});

// 设置分区：核心通用 + 网关配置专属 tab
registerPlugin({
  name: 'settings-gateway',
  version: '0.2.0',
  settings: [
    { tab: 'general',  title: 'settings.sec_lang_anim',   component: lazy(() => import('@pages/settings/GeneralSection')), order: 0 },
    { tab: 'gateway',  title: 'settings.sec_gateway',      component: lazy(() => import('@pages/settings/GatewaySection')),  order: 1 },
    { tab: 'network',  title: 'settings.sec_proxy',        component: lazy(() => import('@pages/settings/NetworkSection')), order: 2 },
    { tab: 'modules',  title: 'settings.sec_modules',      component: lazy(() => import('@pages/settings/ModulesSection')), order: 98 },
    { tab: 'about',    title: 'settings.sec_runtime',      component: lazy(() => import('@pages/settings/AboutSection')),   order: 99 },
  ],
});

export const builtinPluginList: PLSPlugin[] = [];
