import React from 'react';
import { cn } from '@utils/cn';

export interface PillCardProps {
  hoverable?: boolean;
  padding?: 'none' | 'sm' | 'md' | 'lg';
  bordered?: boolean;
  className?: string;
  style?: React.CSSProperties;
  header?: React.ReactNode;
  footer?: React.ReactNode;
  children?: React.ReactNode;
}

const padCls = {
  none: '!p-0',
  sm:   '!p-3',
  md:   '!p-5',
  lg:   '!p-8',
};

export const PillCard: React.FC<PillCardProps> = ({
  hoverable = true, padding = 'md', bordered = true,
  className, style, header, footer, children,
}) => (
  <div
    style={style}
    className={cn(
      'rounded-softer bg-white dark:bg-neutral-950 transition-all duration-300 ease-PILL',
      padCls[padding],
      bordered && 'border border-neutral-200/70 dark:border-neutral-800/70',
      hoverable ? 'shadow-card hover:shadow-card-hover hover:-translate-y-0.5' : 'shadow-soft',
      className,
    )}
  >
    {header && <div className="mb-4">{header}</div>}
    {children}
    {footer && (
      <div className="mt-5 pt-5 border-t border-neutral-200/70 dark:border-neutral-800/70">{footer}</div>
    )}
  </div>
);

export default PillCard;
