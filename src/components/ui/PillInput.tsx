import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@utils/cn';
import Icon from '@icons/index';

export interface PillInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size' | 'prefix'> {
  size?: 'sm' | 'md' | 'lg';
  error?: string;
  hint?: string;
  label?: string;
  clearable?: boolean;
  onClear?: () => void;
  wrapperClassName?: string;
  prefix?: React.ReactNode;
  suffix?: React.ReactNode;
  leftIcon?: React.ReactNode;
}

const sizeCls = {
  sm: '!px-4 !py-1.5 text-xs',
  md: '!px-5 !py-2.5 text-sm',
  lg: '!px-6 !py-3.5 text-base',
};

export const PillInput: React.FC<PillInputProps> = ({
  size = 'md', error, hint, label,
  clearable, onClear, className, wrapperClassName,
  prefix, suffix, leftIcon,
  type = 'text', value, defaultValue, disabled, readOnly,
  onChange, ...rest
}) => {
  const { t } = useTranslation();
  const [focused, setFocused] = useState(false);
  const [showPw, setShowPw] = useState(false);
  const isControlled = value !== undefined;
  const [inner, setInner] = useState<React.ReactNode>(defaultValue ?? '');
  const current = isControlled ? value : inner;
  const hasValue = String(current ?? '').length > 0 && current !== 0;

  const resolvedType = type === 'password' && showPw ? 'text' : type;

  return (
    <div className="p-field">
      {label && <label className="p-label">{label}</label>}
      <div className={cn(
        'pill-input flex items-center gap-2 w-full bg-white dark:bg-neutral-950',
        error
          ? 'border-black dark:border-white focus-within:ring-4 focus-within:ring-black/10 dark:focus-within:ring-white/15'
          : 'border-neutral-200 dark:border-neutral-800 focus-within:border-neutral-900 dark:focus-within:border-neutral-200 focus-within:ring-4 focus-within:ring-neutral-900/10 dark:focus-within:ring-neutral-100/15',
        (disabled || readOnly) && 'opacity-60 cursor-not-allowed bg-neutral-100 dark:bg-neutral-900',
        sizeCls[size],
        wrapperClassName,
      )}>
        {leftIcon ? (
          <span className="text-neutral-500 shrink-0">{leftIcon}</span>
        ) : null}
        {prefix ? (
          <span className="text-neutral-500 shrink-0">{prefix}</span>
        ) : null}
        <input
          type={resolvedType}
          value={value}
          defaultValue={defaultValue}
          disabled={disabled}
          readOnly={readOnly}
          onFocus={(e) => { setFocused(true); rest.onFocus?.(e); }}
          onBlur={(e)  => { setFocused(false); rest.onBlur?.(e); }}
          onChange={(e) => {
            if (!isControlled) setInner(e.target.value);
            onChange?.(e);
          }}
          className={cn(
            'flex-1 bg-transparent outline-none border-none min-w-0',
            'placeholder:text-neutral-400 dark:placeholder:text-neutral-500',
            'text-neutral-900 dark:text-neutral-100',
            className,
          )}
          {...(rest as any)}
        />

        {clearable && hasValue && !disabled && !readOnly && (
          <button
            type="button"
            aria-label={t('common.clear')}
            className="text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 transition-colors
                       rounded-full p-1 hover:bg-neutral-100 dark:hover:bg-neutral-800"
            onClick={() => {
              if (!isControlled) setInner('');
              const synthetic = { target: { value: '' } } as unknown as React.ChangeEvent<HTMLInputElement>;
              onChange?.(synthetic);
              onClear?.();
            }}
          >
            <Icon name="x" size={14}/>
          </button>
        )}

        {type === 'password' && !disabled && !readOnly && (
          <button
            type="button"
            aria-label={showPw ? '隐藏密码' : '显示密码'}
            onClick={() => setShowPw(v => !v)}
            className="text-neutral-400 hover:text-neutral-900 dark:hover:text-neutral-100 transition-colors
                       rounded-full p-1 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <Icon name={showPw ? 'eye-off' : 'eye'} size={16}/>
          </button>
        )}

        {suffix ? (
          <span className="text-neutral-500 shrink-0">{suffix}</span>
        ) : null}
      </div>
      {error && <span className="p-error">{error}</span>}
      {!error && hint && <span className="p-hint">{hint}</span>}
    </div>
  );
};

export default PillInput;
