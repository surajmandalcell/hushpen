import React, { forwardRef, HTMLAttributes } from 'react';
import { cn } from '../../utils/cn';
import PalText from './PalText';

export interface PalSidebarProps extends HTMLAttributes<HTMLDivElement> {
  collapsed?: boolean;
  width?: 'sm' | 'md' | 'lg';
  withGlow?: boolean;
  withCornerMarkers?: boolean;
}

const PalSidebar = forwardRef<HTMLDivElement, PalSidebarProps>(
  ({
    className,
    collapsed = false,
    width = 'md',
    withGlow = false,
    withCornerMarkers = false,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'h-full flex flex-col transition-all duration-300 bg-zinc-900/50 border-r border-zinc-800/60 relative';
    
    const widthClasses = {
      sm: collapsed ? 'w-16' : 'w-48',
      md: collapsed ? 'w-16' : 'w-64',
      lg: collapsed ? 'w-16' : 'w-80',
    };
    
    const glowClasses = withGlow ? 'shadow-[0_0_20px_rgba(59,130,246,0.3)]' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'relative overflow-visible' : '';
    
    const classes = cn(
      baseClasses,
      widthClasses[width],
      glowClasses,
      cornerMarkerClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        {withCornerMarkers && (
          <>
            <div className="absolute top-0 left-0 w-2 h-2 border-t border-l border-zinc-600" />
            <div className="absolute top-0 right-0 w-2 h-2 border-t border-r border-zinc-600" />
            <div className="absolute bottom-0 left-0 w-2 h-2 border-b border-l border-zinc-600" />
            <div className="absolute bottom-0 right-0 w-2 h-2 border-b border-r border-zinc-600" />
          </>
        )}
        {children}
      </div>
    );
  }
);

PalSidebar.displayName = 'PalSidebar';

export interface PalSidebarItemProps extends HTMLAttributes<HTMLDivElement> {
  active?: boolean;
  icon?: React.ReactNode;
  label?: string;
  collapsed?: boolean;
}

export const PalSidebarItem = forwardRef<HTMLDivElement, PalSidebarItemProps>(
  ({
    className,
    active = false,
    icon,
    label,
    collapsed = false,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'cursor-pointer transition-all duration-200 px-3 py-2 rounded-md mx-2 my-1';
    
    const activeClasses = active
      ? 'bg-zinc-800/60 text-zinc-100 border-l-2 border-cyan-500'
      : 'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/40';
    
    const classes = cn(
      baseClasses,
      activeClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        <div className="flex items-center gap-3">
          {icon && (
            <div className={cn('flex-shrink-0', collapsed && 'mx-auto')}>
              {icon}
            </div>
          )}
          {!collapsed && label && (
            <PalText size="sm" weight="medium" className="flex-1">
              {label}
            </PalText>
          )}
          {!collapsed && children}
        </div>
      </div>
    );
  }
);

PalSidebarItem.displayName = 'PalSidebarItem';

export interface PalSidebarSectionProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  collapsed?: boolean;
}

export const PalSidebarSection = forwardRef<HTMLDivElement, PalSidebarSectionProps>(
  ({
    className,
    title,
    collapsed = false,
    children,
    ...props
  }, ref) => {
    const baseClasses = 'py-2';
    
    const classes = cn(
      baseClasses,
      className
    );
    
    return (
      <div
        className={classes}
        ref={ref}
        {...props}
      >
        {!collapsed && title && (
          <div className="px-5 py-1">
            <PalText size="xs" variant="muted" weight="medium" className="uppercase tracking-wider">
              {title}
            </PalText>
          </div>
        )}
        <div className="space-y-1">
          {children}
        </div>
      </div>
    );
  }
);

PalSidebarSection.displayName = 'PalSidebarSection';

export default PalSidebar;