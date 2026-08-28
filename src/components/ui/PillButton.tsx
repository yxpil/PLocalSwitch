import React from 'react';
import { cn } from '@utils/cn';

type Variant = 'primary' | 'ghost' | 'soft' | 'danger';
type Size    = 'sm' | 'md' | 'lg';

export interface PillButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  full?: boolean;
  loading?: boolean;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
}

const variantCls: Record<Variant, string> = {
  primary: 'pill-variant-primary',
  ghost:   'pill-variant-ghost',
  soft:    'pill-variant-soft',
  danger:  'pill-variant-danger',
};
const sizeCls: Record<Size, string> = {
  sm: 'px-3.5 py-1.5 text-xs',
  md: 'px-5   py-2.5 text-sm',
  lg: 'px-7   py-3   text-base',
};

export const PillButton: React.FC<PillButtonProps> = ({
  variant = 'primary', size = 'md', full, loading, disabled,
  leftIcon, rightIcon, className, children, onClick, type = 'button', ...rest
}) => {
  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (disabled || loading) return;
    onClick?.(e);
  };
  return (
    <button
      type={type}
      disabled={disabled || loading}
      onClick={handleClick}
      className={cn(
        'pill-btn cursor-pointer',
        variantCls[variant],
        sizeCls[size],
        full && 'w-full',
        (disabled || loading) && 'opacity-60 cursor-not-allowed !translate-y-0 !shadow-pill-active',
        className,
      )}
      {...rest}
    >
      {loading && (
        <span className="h-4 w-4 -ml-1 rounded-full border-2 border-current/30 border-t-current animate-spin" />
      )}
      {!loading && leftIcon}
      <span className="inline-flex items-center gap-2">{children}</span>
      {rightIcon}
    </button>
  );
};

export default PillButton;
