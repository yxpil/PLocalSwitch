import React from 'react';
import { cn } from '@utils/cn';

export interface PillTabItem {
  key: string | number;
  label: React.ReactNode;
  disabled?: boolean;
  badge?: React.ReactNode;
}

export interface PillTabsProps {
  value: string | number;
  onChange: (v: string | number) => void;
  items: PillTabItem[];
  size?: 'sm' | 'md' | 'lg';
  variant?: 'solid' | 'soft' | 'ghost';
  className?: string;
}

const sizeCls = { sm: 'text-xs px-3 py-1.5', md: 'text-sm px-4 py-2', lg: 'text-base px-5 py-2.5' };

const wrapMap = {
  solid: 'p-1 bg-neutral-100 dark:bg-neutral-900 rounded-pill shadow-inner gap-1',
  soft:  'p-0 gap-1',
  ghost: 'p-0 gap-2',
};

export const PillTabs: React.FC<PillTabsProps> = ({
  value, onChange, items, size = 'md', variant = 'solid', className,
}) => {
  return (
    <div role="tablist" className={cn('inline-flex items-center flex-wrap', wrapMap[variant], className)}>
      {items.map((it) => {
        const active = it.key === value;
        return (
          <button
            key={it.key}
            type="button"
            role="tab"
            aria-selected={active}
            disabled={it.disabled}
            onClick={() => !it.disabled && onChange(it.key)}
            className={cn(
              'rounded-pill font-medium transition-all duration-PILL ease-PILL inline-flex items-center gap-2',
              sizeCls[size],
              it.disabled ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer',
              variant === 'solid' && [
                active
                  ? 'bg-neutral-900 text-white shadow-pill dark:bg-white dark:text-black'
                  : 'text-neutral-600 hover:bg-neutral-100 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-900 dark:hover:text-white',
              ],
              variant === 'soft' && [
                active
                  ? 'bg-neutral-900 text-white shadow-pill dark:bg-white dark:text-black'
                  : 'text-neutral-600 hover:bg-neutral-100 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-900 dark:hover:text-white',
              ],
              variant === 'ghost' && [
                active
                  ? 'bg-neutral-900 text-white shadow-pill dark:bg-white dark:text-black'
                  : 'text-neutral-600 hover:bg-neutral-100 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-900 dark:hover:text-white',
              ],
            )}
          >
            <span>{it.label}</span>
            {it.badge !== undefined && (
              <span className={cn(
                'ml-1 text-[10px] px-2 py-0.5 rounded-full',
                active ? 'bg-black/10 dark:bg-white/15 text-current' : 'bg-neutral-200 dark:bg-neutral-800 text-current',
              )}>
                {it.badge}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
};

export default PillTabs;
