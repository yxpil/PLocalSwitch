import React, { Suspense } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import AppShell from '../components/layout/AppShell';
import NotFound from '../pages/NotFound';
import { getRoutes } from '@plugins/index';
import { SafeRender } from '@plugins/index';

/**
 * 路由：所有业务页面都通过插件注册（getRoutes），
 * 以便新增/禁用功能时只改插件注册，不侵入路由配置本身。
 */
const AppRouter: React.FC = () => {
  const routes = getRoutes();
  return (
    <SafeRender moduleName="router">
      <Routes>
        <Route element={<AppShell/>}>
          {routes.map((r) => {
            const Comp = r.component as React.ComponentType;
            return (
              <Route key={r.path} path={r.path} element={
                <Suspense fallback={
                  <div className="animate-pulse h-60 rounded-softer bg-neutral-100 dark:bg-neutral-900 border border-dashed border-neutral-200 dark:border-neutral-800"/>
                }>
                  <Comp />
                </Suspense>
              }/>
            );
          })}
          {/* 404 兜底 */}
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
    </SafeRender>
  );
};

export default AppRouter;
