/**
 * ============================================================
 *  SVG 图标库（零 emoji，纯黑白描边风格）
 *  设计参考 Lucide Icons：24×24 视口、2px 描边、圆角线帽/线接
 *  用法： <Icon name="home" size={18} />
 * ============================================================
 */
import React from 'react';
import { clsx } from 'clsx';

export type IconName =
  | 'home' | 'refresh-cw' | 'folder' | 'settings' | 'info'
  | 'search' | 'sun' | 'moon' | 'bell' | 'plus' | 'minus'
  | 'x' | 'check' | 'chevron-right' | 'chevron-left' | 'chevron-down'
  | 'user' | 'globe' | 'upload' | 'download' | 'clock'
  | 'shield' | 'zap' | 'database' | 'bar-chart-3' | 'sliders-horizontal'
  | 'eye' | 'eye-off' | 'power' | 'power-off' | 'book'
  | 'file-text' | 'edit-3' | 'trash-2' | 'plus-circle' | 'help-circle'
  | 'alert-triangle' | 'alert-circle' | 'check-circle' | 'x-circle'
  | 'more-horizontal' | 'filter' | 'layers'
  | 'layout-dashboard' | 'network' | 'receipt' | 'git-branch'
  | 'server' | 'activity' | 'key' | 'wallet' | 'link'
  | 'trending-up' | 'list' | 'terminal' | 'copy' | 'hash' | 'unlock'
  | 'play' | 'pause'
  | 'menu' | 'search-x' | 'rotate-ccw' | 'chevron-up' | 'message-circle'
  | 'shield-alert' | 'plug' | 'save' | 'square';

export interface IconProps extends React.SVGAttributes<SVGSVGElement> {
  name: IconName;
  size?: number | string;
  strokeWidth?: number;
}

/* ============ SVG paths 定义（24x24, stroke） ============ */
const PATHS: Record<IconName, React.ReactNode> = {
  'home':             (<><path d="M3 9.5L12 3l9 6.5V20a1 1 0 0 1-1 1h-5v-7h-6v7H4a1 1 0 0 1-1-1V9.5z"/></>),
  'refresh-cw':       (<><path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/></>),
  'rotate-ccw':       (<><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></>),
  'folder':           (<><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></>),
  'settings':         (<><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></>),
  'info':             (<><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></>),
  'search':           (<><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></>),
  'search-x':         (<><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/><path d="m9 9 4 4M13 9l-4 4"/></>),
  'sun':              (<><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></>),
  'moon':             (<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>),
  'bell':             (<><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/></>),
  'plus':             (<><path d="M12 5v14M5 12h14"/></>),
  'minus':            (<path d="M5 12h14"/>),
  'x':                (<><path d="M18 6 6 18M6 6l12 12"/></>),
  'check':            (<path d="m5 12 5 5L20 7"/>),
  'chevron-right':    (<path d="m9 6 6 6-6 6"/>),
  'chevron-left':     (<path d="m15 6-6 6 6 6"/>),
  'chevron-down':     (<path d="m6 9 6 6 6-6"/>),
  'chevron-up':       (<path d="m18 15-6-6-6 6"/>),
  'user':             (<><path d="M20 21a8 8 0 1 0-16 0"/><circle cx="12" cy="7" r="4"/></>),
  'globe':            (<><circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20"/></>),
  'upload':           (<><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m17 8-5-5-5 5M12 3v12"/></>),
  'download':         (<><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5M12 15V3"/></>),
  'clock':            (<><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></>),
  'shield':           (<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>),
  'zap':              (<path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/>),
  'database':         (<><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5v6c0 1.7 4 3 9 3s9-1.3 9-3V5M3 11v6c0 1.7 4 3 9 3s9-1.3 9-3v-6M3 17v6c0 1.7 4 3 9 3s9-1.3 9-3v-6"/></>),
  'bar-chart-3':      (<><path d="M3 3v18h18"/><path d="M7 16V10M12 16V6M17 16v-4"/></>),
  'sliders-horizontal':(<><path d="M4 8h10M18 8h2M4 16h4M12 16h8"/><circle cx="16" cy="8" r="2"/><circle cx="10" cy="16" r="2"/></>),
  'eye':              (<><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></>),
  'eye-off':          (<><path d="M17.9 17.9A10.9 10.9 0 0 1 12 19C5.5 19 2 12 2 12a18.5 18.5 0 0 1 5.1-5.9M9.9 5.1A10.9 10.9 0 0 1 12 5c6.5 0 10 7 10 7a18.5 18.5 0 0 1-3.2 4.2M1 1l22 22M9.9 9.9a3 3 0 0 0 4.2 4.2"/></>),
  'power':            (<><path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.8 0"/></>),
  'power-off':        (<><path d="M18.4 6.6a9 9 0 0 1 2.6 9.7M20.2 12A8.4 8.4 0 0 1 19 15.3M6.6 6.6C3.1 10.2 2.5 15.1 5.6 18.4a9 9 0 0 0 12.8 0c.9-1 1.6-2.1 2-3.4M12 2v5M2 2l20 20"/></>),
  'play':             (<path d="M7 4.5v15l12-7.5-12-7.5z"/>),
  'pause':            (<><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></>),
  'book':             (<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V2H6.5A2.5 2.5 0 0 0 4 4.5v15zM4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5H6.5A2.5 2.5 0 0 0 4 19.5z"/>),
  'file-text':        (<><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/><path d="M14 2v6h6M8 13h8M8 17h8M8 9h2"/></>),
  'edit-3':           (<><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z"/></>),
  'trash-2':          (<><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6"/></>),
  'plus-circle':      (<><circle cx="12" cy="12" r="10"/><path d="M12 8v8M8 12h8"/></>),
  'help-circle':      (<><circle cx="12" cy="12" r="10"/><path d="M9.1 9a3 3 0 1 1 5.8 1c0 2-3 3-3 3M12 17h.01"/></>),
  'alert-triangle':   (<><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0zM12 9v4M12 17h.01"/></>),
  'alert-circle':     (<><circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/></>),
  'check-circle':     (<><circle cx="12" cy="12" r="10"/><path d="m8 12 3 3L16 9"/></>),
  'x-circle':         (<><circle cx="12" cy="12" r="10"/><path d="m15 9-6 6M9 9l6 6"/></>),
  'more-horizontal':  (<><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5"  cy="12" r="1"/></>),
  'filter':           (<path d="M22 3H2l8 9.5V19l4 2v-8.5L22 3z"/>),
  'layers':           (<><path d="m12 2 10 6-10 6L2 8l10-6zM2 16l10 6 10-6M2 12l10 6 10-6"/></>),
  /* 网关管理新增（Lucide 风格） */
  'layout-dashboard': (<><rect x="3"  y="3"  width="7" height="9" rx="1.5"/><rect x="14" y="3"  width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3"  y="16" width="7" height="5" rx="1.5"/></>),
  'network':          (<><circle cx="12" cy="12" r="2"/><circle cx="4"  cy="6"  r="2"/><circle cx="20" cy="6"  r="2"/><circle cx="4"  cy="18" r="2"/><circle cx="20" cy="18" r="2"/><path d="M6 6l4 4M18 6l-4 4M6 18l4-4M18 18l-4-4"/></>),
  'receipt':          (<path d="M4 2v20l3-2 3 2 3-2 3 2 3-2 1 2V2zM8 7h8M8 11h8M8 15h5"/>),
  'git-branch':       (<><circle cx="6" cy="6" r="2"/><circle cx="6" cy="18" r="2"/><circle cx="18" cy="6" r="2"/><path d="M6 8v8M18 8c0 6-6 4-6 10M6 14c6 0 6-6 12-6"/></>),
  'server':           (<><rect x="3" y="3"  width="18" height="7" rx="1.5"/><rect x="3" y="14" width="18" height="7" rx="1.5"/><path d="M7 7h.01M7 18h.01"/></>),
  'activity':         (<path d="M22 12h-4l-3 9-6-18-3 9H2"/>),
  'key':              (<><circle cx="8" cy="15" r="4"/><path d="m11 12 10-10M17 6l3 3M15 8l3 3"/></>),
  'wallet':           (<><path d="M21 12V8a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-4h-5a2 2 0 1 1 0-4h5z"/><circle cx="17" cy="12" r="1"/></>),
  'link':             (<><path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/><path d="M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/></>),
  /* 新增：Billing / Traces / 网关配置 */
  'trending-up':      (<><polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/><polyline points="16 7 22 7 22 13"/></>),
  'list':             (<><path d="M8 6h13M8 12h13M8 18h13"/><circle cx="3.5" cy="6"  r="1"/><circle cx="3.5" cy="12" r="1"/><circle cx="3.5" cy="18" r="1"/></>),
  'terminal':         (<><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></>),
  'copy':             (<><rect x="9"  y="9"  width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></>),
  'hash':             (<><line x1="4" y1="9"  x2="20" y2="9"/><line x1="4" y1="15" x2="20" y2="15"/><line x1="10" y1="3" x2="8"  y2="21"/><line x1="16" y1="3" x2="14" y2="21"/></>),
  'unlock':           (<><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></>),
  'menu':             (<><line x1="4" y1="6"  x2="20" y2="6"/><line x1="4" y1="12" x2="20" y2="12"/><line x1="4" y1="18" x2="20" y2="18"/></>),
  'shield-alert':     (<><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M12 8v4M12 16h.01"/></>),
  'plug':             (<><path d="M12 22v-5"/><path d="M9 8V2M15 8V2"/><path d="M18 8H6v4a6 6 0 0 0 6 6 6 6 0 0 0 6-6V8z"/></>),
  'message-circle':   (<path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>),
  'save':             (<><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/></>),
  'square':           (<rect x="4" y="4" width="16" height="16" rx="2"/>),
};

/**
 * 通用 Icon 组件
 */
export const Icon: React.FC<IconProps> = ({
  name, size = 18, strokeWidth = 2, className, ...rest
}) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={strokeWidth}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={clsx('inline-block shrink-0', className)}
    aria-hidden="true"
    {...rest}
  >
    {PATHS[name]}
  </svg>
);

export default Icon;
