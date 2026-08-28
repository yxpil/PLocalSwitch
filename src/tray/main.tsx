import React, { StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from '@i18n/index';
import TrayMenu from '../pages/TrayMenu';
import '../styles/output.css';
import '../styles/input.css';

// 托盘独立网页（tray-menu.html）入口：只渲染菜单，不依赖主 React App / 路由。
// 整页背景透明，卡片圆角由 TrayMenu 自身样式提供，四周留透明边距形成「圆角」。
const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('#root not found');
const rootContainer: HTMLElement = rootEl;

document.documentElement.style.background = 'transparent';
document.body.style.background = 'transparent';

// 等 i18n 初始化完成再渲染，避免菜单文案显示成翻译 key（如 tray.show）
function boot() {
  createRoot(rootContainer).render(
    <StrictMode>
      <Suspense fallback={null}>
        <TrayMenu />
      </Suspense>
    </StrictMode>,
  );
}

i18n.init().then(boot).catch(boot);
