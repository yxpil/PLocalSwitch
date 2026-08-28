import React, { useEffect, useState } from 'react';
import Icon, { type IconName } from '@icons/index';

// 本机网关地址 + 管理 token：菜单所有操作都走 HTTP 直连网关（不依赖 Tauri IPC）
const GW = 'http://127.0.0.1:8787';
const TOKEN = 'pls-local-manage';

// 菜单文案（独立页自含，不依赖主界面 i18n）
const TRAY_LABELS: Record<string, Record<string, string>> = {
  'zh-CN': { show: '显示主窗口', feedback: '反馈', pause_gateway: '暂停网关', resume_gateway: '继续网关', quit: '退出', close: '关闭' },
  'zh-TW': { show: '顯示主視窗', feedback: '回饋', pause_gateway: '暫停閘道', resume_gateway: '繼續閘道', quit: '結束', close: '關閉' },
  'en-US': { show: 'Show main window', feedback: 'Feedback', pause_gateway: 'Pause gateway', resume_gateway: 'Resume gateway', quit: 'Quit', close: 'Close' },
  'ja-JP': { show: 'メインウィンドウを表示', feedback: 'フィードバック', pause_gateway: 'ゲートウェイを一時停止', resume_gateway: 'ゲートウェイを再開', quit: '終了', close: '閉じる' },
  'ko-KR': { show: '메인 창 표시', feedback: '피드백', pause_gateway: '게이트웨이 일시정지', resume_gateway: '게이트웨이 재개', quit: '종료', close: '닫기' },
  'ru-RU': { show: 'Показать главное окно', feedback: 'Обратная связь', pause_gateway: 'Приостановить шлюз', resume_gateway: 'Возобновить шлюз', quit: 'Выход', close: 'Закрыть' },
  'de-DE': { show: 'Hauptfenster anzeigen', feedback: 'Feedback', pause_gateway: 'Gateway pausieren', resume_gateway: 'Gateway fortsetzen', quit: 'Beenden', close: 'Schließen' },
  'fr-FR': { show: 'Afficher la fenêtre principale', feedback: 'Commentaire', pause_gateway: 'Mettre en pause le passerelle', resume_gateway: 'Reprendre le passerelle', quit: 'Quitter', close: 'Fermer' },
  'es-ES': { show: 'Mostrar ventana principal', feedback: 'Comentarios', pause_gateway: 'Pausar gateway', resume_gateway: 'Reanudar gateway', quit: 'Salir', close: 'Cerrar' },
};

function tr(key: string): string {
  let lng = 'zh-CN';
  try { lng = (typeof localStorage !== 'undefined' && localStorage.getItem('pls-locale')) || 'zh-CN'; } catch { /* noop */ }
  const pool = TRAY_LABELS[lng] || TRAY_LABELS['zh-CN'];
  return pool[key.split('.').pop() || key] || key;
}

async function gwPost(action: string): Promise<any> {
  const res = await fetch(GW + '/manage/lifecycle', {
    method: 'POST',
    headers: { 'X-Manage-Token': TOKEN, 'Content-Type': 'application/json' },
    body: JSON.stringify({ action }),
  });
  if (!res.ok) throw new Error(`网关响应 ${res.status}`);
  return res.json();
}
async function gwStatus(): Promise<any> {
  const res = await fetch(GW + '/manage/lifecycle', {
    headers: { 'X-Manage-Token': TOKEN },
  });
  if (!res.ok) throw new Error(`网关响应 ${res.status}`);
  return res.json();
}

interface Item {
  key: string;
  icon?: IconName;
  labelKey?: string;
  action?: string;   // HTTP action
  toggle?: boolean;  // 暂停/继续 二合一
}

const ITEMS: Item[] = [
  { key: 'show',     icon: 'layout-dashboard', labelKey: 'tray.show',     action: 'show' },
  { key: 'feedback', icon: 'info',             labelKey: 'tray.feedback', action: 'feedback' },
  { key: 'separator' },
  { key: 'gateway',  toggle: true },
  { key: 'quit',     icon: 'power',            labelKey: 'tray.quit',     action: 'quit' },
];

const TrayMenu: React.FC = () => {
  const [running, setRunning] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    gwStatus().then((d) => setRunning(!!d.running)).catch(() => {});
  }, []);

  // 关闭菜单：优先让网关隐藏窗口，失败则前端兜底
  const close = () => {
    try {
      if ((window as any).__TAURI_INTERNALS__) {
        void import('@tauri-apps/api/window').then((m) => m.getCurrentWindow().hide());
      } else {
        window.close();
      }
    } catch { /* noop */ }
  };
  const hideMenu = () => { gwPost('hide').catch(() => {}); close(); };

  const run = async (item: Item) => {
    if (busy) return;
    setBusy(true);
    try {
      const action = item.toggle ? (running ? 'stop' : 'start') : (item.action || '');
      await gwPost(action);
      if (item.toggle) {
        const d = await gwStatus();
        setRunning(!!d.running);
      }
    } catch { /* 网关不可达忽略 */ }
    setBusy(false);
    close();
  };

  return (
    <>
      <style>{`
        .tm{width:100%;box-sizing:border-box;padding:14px}
        .tm-card{width:100%;box-sizing:border-box;background:#fff;color:#111;
          padding:8px;border-radius:20px;border:1px solid rgba(0,0,0,.08);
          box-shadow:0 20px 60px -14px rgba(0,0,0,.5)}
        .tm-head{display:flex;align-items:center;gap:10px;padding:8px 10px 10px;
          border-bottom:1px solid rgba(0,0,0,.06);margin-bottom:6px}
        .tm-head img{width:26px;height:26px;border-radius:7px;background:#0a0a0a}
        .tm-head-txt{flex:1;min-width:0}
        .tm-head span{font-size:10px;color:#8a8a8a;font-family:ui-monospace,Menlo,Consolas,monospace}
        .tm-b{font-size:13px;font-weight:700;letter-spacing:.2px}
        .tm-x{margin-left:auto;flex:none;width:24px;height:24px;border:none;background:transparent;
          border-radius:8px;cursor:pointer;color:#8a8a8a;display:flex;align-items:center;justify-content:center}
        .tm-x:hover{background:rgba(0,0,0,.06);color:#111}
        .tm-mi{display:flex;align-items:center;gap:10px;width:100%;box-sizing:border-box;
          padding:10px;border:none;background:transparent;border-radius:12px;cursor:pointer;
          font-size:13px;font-weight:500;color:#111;text-align:left}
        .tm-mi:hover:not(:disabled){background:rgba(0,0,0,.05)}
        .tm-mi:disabled{opacity:.5;cursor:default}
        .tm-mi svg{flex:none;opacity:.85}
        .tm-mi .grow{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
        .tm-sep{height:1px;margin:5px 8px;background:rgba(0,0,0,.07)}
        .tm-status{width:8px;height:8px;border-radius:50%;flex:none}
        .tm-quit .tm-mi{color:#d03050}
        @media (prefers-color-scheme:dark){
          .tm-card{background:#141414;color:#f4f4f4;border-color:rgba(255,255,255,.08);
            box-shadow:0 20px 60px -14px rgba(0,0,0,.8)}
          .tm-head{border-color:rgba(255,255,255,.08)}
          .tm-head span{color:#7a7a7a}
          .tm-x{color:#7a7a7a}
          .tm-x:hover{background:rgba(255,255,255,.1);color:#fff}
          .tm-mi{color:#f4f4f4}
          .tm-mi:hover:not(:disabled){background:rgba(255,255,255,.09)}
          .tm-sep{background:rgba(255,255,255,.09)}
          .tm-quit .tm-mi{color:#ff7b8f}
        }
      `}</style>
      <div className="tm">
        <div className="tm-card">
          <div className="tm-head">
            <img src="/logo.png" alt="logo" />
            <div className="tm-head-txt">
              <div className="tm-b">PLocalSwitch</div>
              <div><span>Gateway</span></div>
            </div>
            <button className="tm-x" aria-label={tr('tray.close')} onClick={hideMenu}>
              <Icon name="x" size={14} />
            </button>
          </div>
          {ITEMS.map((item) => {
            if (item.key === 'separator') return <div key="sep" className="tm-sep" />;
            const isT = !!item.toggle;
            const icon = isT ? (running === true ? 'pause' : 'play') : (item.icon as IconName);
            const label = isT ? (running === true ? tr('tray.pause_gateway') : tr('tray.resume_gateway')) : tr(item.labelKey!);
            return (
              <div key={item.key} className={item.key === 'quit' ? 'tm-quit' : ''}>
                <button className="tm-mi" disabled={busy} onClick={() => run(item)}>
                  <Icon name={icon} size={15} />
                  <span className="grow">{label}</span>
                  {isT && running === true && <span className="tm-status" style={{ background: '#2dd4a7' }} />}
                  {isT && running === false && <span className="tm-status" style={{ background: '#8a8a8a' }} />}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
};

export default TrayMenu;
