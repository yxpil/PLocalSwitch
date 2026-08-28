import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Outlet } from 'react-router-dom';
import Sidebar from './Sidebar';
import Topbar from './Topbar';
import { SafeRender } from '@plugins/index';
import Icon from '@icons/index';

/**
 * 无边框窗口：顶部自绘标题栏（可拖动 + 最小化/最大化/关闭嵌在网页内）
 */
const AppShell: React.FC = () => {
  const { t } = useTranslation();
  const win = (globalThis as any).__TAURI_INTERNALS__ ? () => import('@tauri-apps/api/window') : null;
  const clampCleanup = useRef<(() => void)[]>([]);

  // 触摸屏/拖拽防越界：窗口移动/缩放后，把位置钳制在当前显示器范围内（避免拖出屏幕看到外部）
  useEffect(() => {
    if (!(globalThis as any).__TAURI_INTERNALS__) return;
    (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const { PhysicalPosition } = await import('@tauri-apps/api/dpi');
        const w = getCurrentWindow();
        const clamp = async () => {
          try {
            if ((await w.isMaximized()) || (await w.isFullscreen())) return;
            const mon = await (w as any).currentMonitor();
            if (!mon) return;
            const pos = await w.outerPosition();
            const size = await w.outerSize();
            const maxX = mon.position.x + mon.size.width - size.width;
            const maxY = mon.position.y + mon.size.height - size.height;
            const x = Math.min(Math.max(pos.x, mon.position.x), Math.max(mon.position.x, maxX));
            const y = Math.min(Math.max(pos.y, mon.position.y), Math.max(mon.position.y, maxY));
            if (x !== pos.x || y !== pos.y) await w.setPosition(new PhysicalPosition(x, y));
          } catch { /* 非 Tauri 或受限时忽略 */ }
        };
        const unMoved = await w.onMoved(clamp);
        const unResized = await w.onResized(clamp);
        clampCleanup.current.push(unMoved, unResized);
        void clamp();
      } catch { /* 非 Tauri 环境忽略 */ }
    })();
    return () => {
      clampCleanup.current.forEach((fn) => { try { fn(); } catch { /* noop */ } });
      clampCleanup.current = [];
    };
  }, []);

  const minimize = async () => { try { (await win!()).getCurrentWindow().minimize(); } catch {} };
  const toggleMaximize = async () => { try { (await win!()).getCurrentWindow().toggleMaximize(); } catch {} };
  const close = async () => { try { (await win!()).getCurrentWindow().close(); } catch {} };

  return (
    <SafeRender moduleName="shell">
      <div className="h-screen w-screen flex flex-row items-stretch pt-9
                      bg-white dark:bg-black text-neutral-900 dark:text-neutral-100
                      antialiased">
        {/* 自绘标题栏（可拖动区域 + 窗口按钮嵌网页） */}
        <div data-tauri-drag-region
          className="absolute top-0 inset-x-0 h-9 flex items-center justify-end gap-1 pr-2 z-40 select-none
                     bg-white dark:bg-black border-b border-neutral-200/70 dark:border-neutral-800/70">
          <button aria-label={t('shell.minimize')} onClick={minimize}
            className="h-7 w-7 rounded-pill flex items-center justify-center text-neutral-500 hover:bg-neutral-200 hover:text-neutral-900 dark:hover:bg-neutral-800 dark:hover:text-neutral-100 transition-colors">
            <Icon name="minus" size={13} />
          </button>
          <button aria-label={t('shell.maximize')} onClick={toggleMaximize}
            className="h-7 w-7 rounded-pill flex items-center justify-center text-neutral-500 hover:bg-neutral-200 hover:text-neutral-900 dark:hover:bg-neutral-800 dark:hover:text-neutral-100 transition-colors">
            <Icon name="square" size={12} />
          </button>
          <button aria-label={t('shell.close')} onClick={close}
            className="h-7 w-7 rounded-pill flex items-center justify-center text-neutral-500 hover:bg-red-600 hover:text-white dark:hover:bg-red-600 dark:hover:text-white transition-colors">
            <Icon name="x" size={14} />
          </button>
        </div>

        <Sidebar />
        <div className="flex-1 flex flex-col min-w-0">
          <Topbar />
          <main className="flex-1 overflow-auto">
            <div className="px-5 md:px-8 py-6 md:py-8 max-w-[1400px] w-full mx-auto">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
    </SafeRender>
  );
};

export default AppShell;
