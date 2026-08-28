import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@utils/cn';
import Icon from '@icons/index';

export interface PillModalProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full';
  closeOnMask?: boolean;
  showClose?: boolean;
  footer?: React.ReactNode | ((ctx: { close: () => void }) => React.ReactNode);
  children?: React.ReactNode;
}

const sizeCls = {
  sm:   'max-w-sm',
  md:   'max-w-md',
  lg:   'max-w-2xl',
  xl:   'max-w-4xl',
  full: 'max-w-[95vw]',
};

export const PillModal: React.FC<PillModalProps> = ({
  open, onClose, title, size = 'md', closeOnMask = true, showClose = true, footer, children,
}) => {
  const { t } = useTranslation();
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', h);
    const prev = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      window.removeEventListener('keydown', h);
      document.body.style.overflow = prev;
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
    >
      {/* Mask */}
      <div
        aria-hidden
        onClick={() => closeOnMask && onClose()}
        className="absolute inset-0 bg-black/50 dark:bg-black/70 backdrop-blur-sm animate-[fadeIn_.2s_ease]"
      />
      {/* Panel */}
      <div
        className={cn(
          'relative w-full rounded-softer bg-white dark:bg-neutral-950 overflow-hidden',
          'shadow-[0_30px_80px_-20px_rgba(0,0,0,0.4)]',
          'border border-neutral-200/60 dark:border-neutral-800/60',
          sizeCls[size],
          'animate-[popIn_.26s_cubic-bezier(0.34,1.56,0.64,1)_both]',
        )}
      >
        {(title || showClose) && (
          <div className="flex items-center justify-between px-6 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
            <h3 className="font-semibold text-lg text-neutral-900 dark:text-neutral-100">{title}</h3>
            {showClose && (
              <button
                type="button"
                aria-label={t('common.close')}
                onClick={onClose}
                className="rounded-full p-2 text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-100
                           hover:bg-neutral-100 dark:hover:bg-neutral-900 transition-all"
              >
                <Icon name="x" size={18}/>
              </button>
            )}
          </div>
        )}
        <div className="px-6 py-5 max-h-[70vh] overflow-y-auto">{children}</div>
        {footer && (
          <div className="px-6 py-4 border-t border-neutral-200/70 dark:border-neutral-800/70 flex flex-wrap items-center justify-end gap-3">
            {typeof footer === 'function' ? footer({ close: onClose }) : footer}
          </div>
        )}
      </div>
    </div>
  );
};

export default PillModal;
