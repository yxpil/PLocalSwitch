import React from 'react';
import { cn } from '@utils/cn';

export interface PillSwitchProps {
  checked: boolean;
  onChange?: (v: boolean) => void;
  size?: 'sm' | 'md' | 'lg';
  disabled?: boolean;
  label?: React.ReactNode;
  description?: React.ReactNode;
  className?: string;
}

const trackCls = { sm: 'w-8 h-5',  md: 'w-11 h-6',  lg: 'w-14 h-8'  };
const thumbCls = { sm: 'w-3.5 h-3.5', md: 'w-5 h-5', lg: 'w-6 h-6' };
const thumbActiveLeft = {
  sm: 'left-[calc(100%-14px-2px)]',
  md: 'left-[calc(100%-20px-2px)]',
  lg: 'left-[calc(100%-24px-3px)]',
};
const thumbBaseLeft = { sm: 'left-[2px]', md: 'left-[2px]', lg: 'left-[3px]' };

export const PillSwitch: React.FC<PillSwitchProps> = ({
  checked, onChange, size = 'md', disabled, label, description, className,
}) => {
  return (
    <label
      className={cn(
        'inline-flex items-start gap-3 select-none',
        disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer',
        className,
      )}
    >
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => !disabled && onChange?.(!checked)}
        className={cn(
          'relative rounded-full transition-all duration-300 ease-PILL shrink-0',
          'shadow-inner bg-neutral-200 dark:bg-neutral-800',
          trackCls[size],
          checked && 'bg-neutral-900 shadow-pill dark:bg-white',
        )}
      >
        <span
          className={cn(
            'absolute top-1/2 -translate-y-1/2 rounded-full bg-white dark:bg-black shadow-md',
            'transition-all duration-300 ease-PILL',
            thumbCls[size],
            checked ? thumbActiveLeft[size] : thumbBaseLeft[size],
          )}
        />
      </button>

      {(label || description) && (
        <div className="flex flex-col min-w-0">
          {label && <span className="text-sm font-medium text-neutral-900 dark:text-neutral-100">{label}</span>}
          {description && <span className="text-xs text-neutral-500 dark:text-neutral-400 mt-0.5">{description}</span>}
        </div>
      )}
    </label>
  );
};

export default PillSwitch;
