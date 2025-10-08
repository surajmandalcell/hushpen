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
      <div className="fixed inset-0 z-50 flex items-center justify-center">
        {/* Backdrop */}
        <button
          type="button"
          className="absolute inset-0 bg-black/60 backdrop-blur-sm cursor-default"
          onClick={handleBackdropClick}
          onKeyDown={handleBackdropKeyDown}
          aria-label="Close modal"
        />
        
        {/* Modal Content */}
        <div
          ref={ref}
          className={cn(
            'relative w-full max-h-[90vh] overflow-auto bg-zinc-900/95 border border-zinc-800/60 shadow-2xl animate-fade-in',
            {
              'max-w-sm': size === 'sm',
              'max-w-md': size === 'md',
              'max-w-lg': size === 'lg',
              'max-w-2xl': size === 'xl',
            },
            withGlow && 'shadow-[0_0_30px_rgba(59,130,246,0.3)]',
            className
          )}
          role="dialog"
          aria-modal="true"
          {...props}
        >
          {withCornerMarkers && (
            <>
              <div className="absolute top-0 left-0 w-3 h-3 border-t border-l border-zinc-600" />
              <div className="absolute top-0 right-0 w-3 h-3 border-t border-r border-zinc-600" />
              <div className="absolute bottom-0 left-0 w-3 h-3 border-b border-l border-zinc-600" />
              <div className="absolute bottom-0 right-0 w-3 h-3 border-b border-r border-zinc-600" />
            </>
          )}
          {children}
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
    const baseClasses = 'flex items-center justify-between px-6 py-4 border-b border-zinc-800/60';
    
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
              className="p-1 rounded text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/50 transition-colors duration-200"
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
    const baseClasses = 'px-6 py-4';
    
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
    const baseClasses = 'flex items-center justify-end gap-3 px-6 py-4 border-t border-zinc-800/60';
    
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