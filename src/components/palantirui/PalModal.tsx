import React, { forwardRef, HTMLAttributes, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../utils/cn';
import PalText from './PalText';

export interface PalModalProps extends HTMLAttributes<HTMLDivElement> {
  isOpen: boolean;
  onClose: () => void;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  withGlow?: boolean;
  withCornerMarkers?: boolean;
  closeOnBackdropClick?: boolean;
  preventBodyScroll?: boolean;
}

const PalModal = forwardRef<HTMLDivElement, PalModalProps>(
  ({ 
    className, 
    isOpen,
    onClose,
    size = 'md',
    withGlow = false,
    withCornerMarkers = false,
    closeOnBackdropClick = true,
    preventBodyScroll = true,
    children, 
    ...props 
  }, ref) => {
    // Handle body scroll lock
    useEffect(() => {
      if (isOpen && preventBodyScroll) {
        document.body.style.overflow = 'hidden';
      } else {
        document.body.style.overflow = '';
      }
      
      return () => {
        document.body.style.overflow = '';
      };
    }, [isOpen, preventBodyScroll]);
    
    // Handle escape key
    useEffect(() => {
      const handleEscape = (e: KeyboardEvent) => {
        if (e.key === 'Escape' && isOpen) {
          onClose();
        }
      };
      
      document.addEventListener('keydown', handleEscape);
      return () => document.removeEventListener('keydown', handleEscape);
    }, [isOpen, onClose]);
    
    if (!isOpen) return null;
    
    const handleBackdropClick = (e: React.MouseEvent) => {
      if (e.target === e.currentTarget && closeOnBackdropClick) {
        onClose();
      }
    };
    
    const handleBackdropKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        if (closeOnBackdropClick) {
          onClose();
        }
      }
    };
    
    const modalContent = (
      <div className="pal-modal">
        <button 
          type="button"
          className="pal-modal-backdrop" 
          onClick={handleBackdropClick}
          onKeyDown={handleBackdropKeyDown}
          aria-label="Close modal"
        />
        <div className="flex items-center justify-center min-h-screen p-4">
          <div
            ref={ref}
            className={cn(
              'pal-modal-content relative w-full',
              {
                'max-w-sm': size === 'sm',
                'max-w-md': size === 'md',
                'max-w-lg': size === 'lg',
                'max-w-2xl': size === 'xl',
              },
              withGlow && 'pal-glow-medium',
              withCornerMarkers && 'pal-corner-markers',
              className
            )}
            {...props}
          >
            {children}
          </div>
        </div>
      </div>
    );
    
    return createPortal(modalContent, document.body);
  }
);

PalModal.displayName = 'PalModal';

export interface PalModalHeaderProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  subtitle?: string;
  showCloseButton?: boolean;
  onClose?: () => void;
}

export const PalModalHeader = forwardRef<HTMLDivElement, PalModalHeaderProps>(
  ({ 
    className, 
    title,
    subtitle,
    showCloseButton = true,
    onClose,
    children, 
    ...props 
  }, ref) => {
    const baseClasses = 'flex items-center justify-between pb-4 mb-4 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700';
    
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
        <div className="flex flex-col">
          {title && (
            <PalText size="lg" weight="semibold">
              {title}
            </PalText>
          )}
          {subtitle && (
            <PalText size="sm" variant="muted">
              {subtitle}
            </PalText>
          )}
        </div>
        <div className="flex items-center gap-2">
          {children}
          {showCloseButton && (
            <button
              type="button"
              onClick={onClose}
              className="p-1 rounded-pal text-palantir-zinc-500 hover:text-palantir-zinc-700 hover:bg-palantir-zinc-100 dark:text-palantir-zinc-400 dark:hover:text-palantir-zinc-200 dark:hover:bg-palantir-zinc-800 transition-colors duration-200"
              aria-label="Close modal"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <title>Close</title>
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>
      </div>
    );
  }
);

PalModalHeader.displayName = 'PalModalHeader';

export interface PalModalContentProps extends HTMLAttributes<HTMLDivElement> {}

export const PalModalContent = forwardRef<HTMLDivElement, PalModalContentProps>(
  ({ className, children, ...props }, ref) => {
    const baseClasses = 'py-4';
    
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
        {children}
      </div>
    );
  }
);

PalModalContent.displayName = 'PalModalContent';

export interface PalModalFooterProps extends HTMLAttributes<HTMLDivElement> {}

export const PalModalFooter = forwardRef<HTMLDivElement, PalModalFooterProps>(
  ({ className, children, ...props }, ref) => {
    const baseClasses = 'flex items-center justify-end gap-3 pt-4 mt-4 border-t border-palantir-zinc-200 dark:border-palantir-zinc-700';
    
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
        {children}
      </div>
    );
  }
);

PalModalFooter.displayName = 'PalModalFooter';

export default PalModal;