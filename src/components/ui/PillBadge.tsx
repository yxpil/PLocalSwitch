import React from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@utils/cn';
import Icon from '@icons/index';

type Variant = 'pass' | 'warn' | 'fail' | 'muted' | 'neutral';
const variantCls: Record<Variant, string> = {
  pass:    'p-badge-pass',
  warn:    'p-badge-warn',
  fail:    'p-badge-fail',
  muted:   'p-badge-muted',
  neutral: 'p-badge-neutral',
};

export interface PillBadgeProps {
  variant?: Variant;
  size?: 'sm' | 'md';
  dot?: boolean;
  closable?: boolean;
  onClose?: () => void;
  className?: string;
  children?: React.ReactNode;
}

export const PillBadge: React.FC<PillBadgeProps> = ({
  variant = 'neutral', size = 'md', dot, closable, onClose, className, children,
}) => {
  const { t } = useTranslation();
  return (<span className={cn(
    variantCls[variant],
    size === 'sm' && '!text-[10px] !px-2 !py-0.5',
    className,
  )}>
    {dot && <span className="inline-block w-1.5 h-1.5 rounded-full bg-current mr-1 align-middle" />}
    <span>{children}</span>
    {closable && (
      <button
        type="button"
        aria-label={t('common.close')}
        onClick={onClose}
        className="ml-1 -mr-1 rounded-full p-0.5 hover:bg-black/10 dark:hover:bg-white/15 transition-colors"
      >
        <Icon name="x" size={10}/>
      </button>
    )}
  </span>);
};

export default PillBadge;
