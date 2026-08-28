import React from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@utils/cn';
import Icon from '@icons/index';

export type ToastType = 'info' | 'success' | 'warning' | 'error';
export interface PillToastProps {
  type?: ToastType;
  title?: React.ReactNode;
  message?: React.ReactNode;
  show?: boolean;
  onClose?: () => void;
  className?: string;
}

const clsMap: Record<ToastType, { wrap: string; iconName: 'info' | 'check-circle' | 'alert-triangle' | 'alert-circle' }> = {
  info:    { wrap: 'border-neutral-300/70 text-neutral-800 dark:border-neutral-700 dark:text-neutral-100', iconName: 'info' },
  success: { wrap: 'border-neutral-900/40 text-neutral-900 dark:border-neutral-100/50 dark:text-neutral-100', iconName: 'check-circle' },
  warning: { wrap: 'border-neutral-500/40 text-neutral-800 dark:border-neutral-400/40 dark:text-neutral-100', iconName: 'alert-triangle' },
  error:   { wrap: 'border-black/40 text-black dark:border-white/50 dark:text-white', iconName: 'alert-circle' },
};

export const PillToast: React.FC<PillToastProps> = ({
  type = 'info', title, message, show = true, onClose, className,
}) => {
  const { t } = useTranslation();
  if (!show) return null;
  const cfg = clsMap[type];
  return (
    <div
      role="status"
      className={cn(
        'min-w-[260px] max-w-sm rounded-pill px-4 py-3',
        'flex items-start gap-3 shadow-card backdrop-blur border',
        'bg-white/90 dark:bg-neutral-950/90',
        cfg.wrap,
        'animate-[fadeInUp_.28s_ease]',
        className,
      )}
    >
      <span className="shrink-0 mt-0.5"><Icon name={cfg.iconName} size={18}/></span>
      <div className="flex-1 min-w-0">
        {title && <div className="font-semibold text-sm">{title}</div>}
        {message && <div className="text-xs mt-0.5 opacity-80 leading-relaxed">{message}</div>}
      </div>
      {onClose && (
        <button
          type="button"
          onClick={onClose}
          aria-label={t('common.close')}
          className="shrink-0 rounded-full p-1 hover:bg-black/5 dark:hover:bg-white/10 transition-colors opacity-70 hover:opacity-100"
        >
          <Icon name="x" size={14}/>
        </button>
      )}
    </div>
  );
};

export default PillToast;
