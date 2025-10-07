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
    const baseClasses = 'pal-sidebar h-full flex flex-col transition-all duration-300';
    
    const widthClasses = {
      sm: collapsed ? 'w-16' : 'w-48',
      md: collapsed ? 'w-16' : 'w-64',
      lg: collapsed ? 'w-16' : 'w-80',
    };
    
    const glowClasses = withGlow ? 'pal-glow-subtle' : '';
    const cornerMarkerClasses = withCornerMarkers ? 'pal-corner-markers' : '';
    
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
    const baseClasses = 'pal-sidebar-item cursor-pointer transition-all duration-200';
    
    const activeClasses = active ? 'pal-sidebar-item.active' : '';
    
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
          <div className="px-3 py-1">
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