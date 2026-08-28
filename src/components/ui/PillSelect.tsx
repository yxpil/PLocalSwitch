import React from 'react';
import { cn } from '@utils/cn';
import Icon from '@icons/index';

export interface PillSelectOption {
  value: string | number;
  label: string;
  disabled?: boolean;
  group?: string;
}

export interface PillSelectProps {
  value?: string | number;
  onChange?: (v: string | number) => void;
  options: PillSelectOption[];
  placeholder?: string;
  size?: 'sm' | 'md' | 'lg';
  disabled?: boolean;
  label?: string;
  hint?: string;
  className?: string;
}

const sizeCls = {
  sm: '!px-4 !py-1.5 text-xs pr-9',
  md: '!px-5 !py-2.5 text-sm pr-11',
  lg: '!px-6 !py-3.5 text-base pr-12',
};

export const PillSelect: React.FC<PillSelectProps> = ({
  value, onChange, options, placeholder, size = 'md', disabled, label, hint, className,
}) => {
  const groups = new Map<string | undefined, PillSelectOption[]>();
  for (const o of options) {
    const k = o.group ?? '__none__';
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k)!.push(o);
  }

  return (
    <div className="p-field">
      {label && <label className="p-label">{label}</label>}
      <div className={cn('relative w-full', className)}>
        <select
          value={value ?? ''}
          disabled={disabled}
          onChange={(e) => onChange?.(e.target.value)}
          className={cn(
            'pill-input appearance-none w-full cursor-pointer',
            'bg-white dark:bg-neutral-950 border border-neutral-200 dark:border-neutral-800',
            'text-neutral-900 dark:text-neutral-100',
            'hover:border-neutral-900/40 dark:hover:border-neutral-200/60',
            'focus:border-neutral-900 dark:focus:border-neutral-200 focus:ring-4 focus:ring-neutral-900/10 dark:focus:ring-neutral-100/15',
            disabled && 'opacity-60 cursor-not-allowed bg-neutral-100 dark:bg-neutral-900',
            sizeCls[size],
          )}
        >
          {placeholder && <option value="" disabled>{placeholder}</option>}
          {Array.from(groups.entries()).map(([grp, list]) => (
            grp !== '__none__' ? (
              <optgroup key={grp} label={grp}>
                {list.map(o => (
                  <option key={String(o.value)} value={o.value} disabled={o.disabled}>{o.label}</option>
                ))}
              </optgroup>
            ) : list.map(o => (
              <option key={String(o.value)} value={o.value} disabled={o.disabled}>{o.label}</option>
            ))
          ))}
        </select>
        <span className="pointer-events-none absolute right-4 top-1/2 -translate-y-1/2 text-neutral-500">
          <Icon name="chevron-down" size={16}/>
        </span>
      </div>
      {hint && <span className="p-hint">{hint}</span>}
    </div>
  );
};

export default PillSelect;
