import React, { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { HashRouter } from 'react-router-dom';
import App from './App';
import './styles/output.css';
import './styles/input.css';
// 插件 bootstrap（注册所有 builtin）
import '@plugins/builtin';
// 初始化日志
import { useLogger } from '@logger/index';

const log = useLogger('main:root');
log.info('========== PLocalSwitch (React) 启动 ==========');
log.info(`Logo signature: <logo.png @ ${window.location.origin}/logo.png>`);

const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('#root not found');

createRoot(rootEl).render(
  <StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
);
