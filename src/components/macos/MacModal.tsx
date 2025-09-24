import React, { useEffect, useRef } from "react";
import { X } from "lucide-react";


export interface MacModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  size?: "small" | "medium" | "large" | "fullscreen";
  showCloseButton?: boolean;
  closeOnOverlayClick?: boolean;
  closeOnEscape?: boolean;
  className?: string;
}

const MacModal: React.FC<MacModalProps> = ({
  isOpen,
  onClose,
  title,
  children,
  size = "medium",
  showCloseButton = true,
  closeOnOverlayClick = true,
  closeOnEscape = true,
  className = "",
}) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (isOpen) {
      // Store the previously focused element
      previousActiveElement.current = document.activeElement as HTMLElement;
      
      // Focus the modal
      if (modalRef.current) {
        modalRef.current.focus();
      }
    } else {
      // Restore focus to the previously focused element
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
      }
    }
  }, [isOpen]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isOpen || !closeOnEscape) return;

      if (event.key === "Escape") {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
      // Prevent body scroll
      document.body.style.overflow = "hidden";
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      // Restore body scroll
      document.body.style.overflow = "unset";
    };
  }, [isOpen, closeOnEscape, onClose]);

  const handleOverlayClick = (event: React.MouseEvent) => {
    if (closeOnOverlayClick && event.target === event.currentTarget) {
      onClose();
    }
  };

  if (!isOpen) return null;

  const modalClasses = [
    "mac-modal",
    `mac-modal--${size}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="mac-modal-overlay" onClick={handleOverlayClick}>
      <div
        ref={modalRef}
        className={modalClasses}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby={title ? "mac-modal-title" : undefined}
      >
        {title && (
          <div className="mac-modal-header">
            <h2 id="mac-modal-title" className="mac-modal-title">
              {title}
            </h2>
            {showCloseButton && (
              <button
                className="mac-modal-close"
                onClick={onClose}
                aria-label="Close modal"
              >
                <X size={16} />
              </button>
            )}
          </div>
        )}
        
        <div className="mac-modal-content">
          {children}
        </div>
      </div>
    </div>
  );
};

export default MacModal;
