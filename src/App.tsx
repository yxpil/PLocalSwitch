import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import AppRouter from './router';
import { useThemeStore } from './stores/theme';
import { useAppStore } from './stores/app';
import { SafeRender, ErrorBoundary } from '@plugins/index';

const App: React.FC = () => {
  // 初始化主题（尽早）
  const themeInit = useThemeStore(s => s.init);
  const appInit   = useAppStore(s => s.initApp);
  const error     = useAppStore(s => s.error);
  const { t } = useTranslation();

  useEffect(() => { themeInit(); }, [themeInit]);
  useEffect(() => { appInit(); },   [appInit]);

  void t; // 保留以触发 suspense（若未来使用）

  return (
    <ErrorBoundary moduleName="app.root">
      <SafeRender moduleName="app">
        <AppRouter/>
        {/* 全局初始化错误 banner（不打断流程） */}
        {error && (
          <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[60]
                          rounded-pill px-4 py-2 bg-black/90 dark:bg-white/90
                          text-white dark:text-black text-xs shadow-card max-w-lg">
            <b>Init warn: </b>{error}
          </div>
        )}
      </SafeRender>
    </ErrorBoundary>
  );
};

export default App;
