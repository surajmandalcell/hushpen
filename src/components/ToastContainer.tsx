import React, { useEffect, useState } from 'react';
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useToast } from '../services/toast';

const ToastContainer: React.FC = () => {
  const { toasts, dismiss } = useToast();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) return null;

  const getToastIcon = (type: string) => {
    switch (type) {
      case 'success':
        return <CheckCircle size={18} />;
      case 'warning':
        return <AlertTriangle size={18} />;
      case 'error':
        return <AlertCircle size={18} />;
      default:
        return <Info size={18} />;
    }
  };

  const getToastClasses = (type: string) => {
    switch (type) {
      case 'success':
        return 'toast-success';
      case 'warning':
        return 'toast-warning';
      case 'error':
        return 'toast-error';
      default:
        return 'toast-info';
    }
  };

  return (
    <div className="fixed top-4 right-4 z-50 space-y-3 pointer-events-none">
      {toasts.map((toast, index) => (
        <div
          key={toast.id}
          className={`${getToastClasses(toast.type)} pointer-events-auto`}
          style={{
            transform: `translateY(${index * 4}px)`,
          }}
        >
          <div className="flex items-start space-x-3">
            <div className="flex-shrink-0 mt-0.5 text-current">
              {getToastIcon(toast.type)}
            </div>

            <div className="flex-1 min-w-0">
              <div className="font-medium text-macos-text dark:text-macos-text-dark">
                {toast.title}
              </div>
              {toast.message && (
                <div className="text-sm text-macos-text-secondary mt-1">
                  {toast.message}
                </div>
              )}
            </div>

            <button
              className="flex-shrink-0 p-1 -m-1 rounded-macos hover:bg-black hover:bg-opacity-10 transition-colors duration-150"
              onClick={() => dismiss(toast.id)}
              aria-label="Close notification"
            >
              <X size={16} className="text-macos-text-secondary" />
            </button>
          </div>

          {toast.actions && toast.actions.length > 0 && (
            <div className="flex space-x-2 mt-3 pt-3 border-t border-current border-opacity-20">
              {toast.actions.map((action, actionIndex) => (
                <button
                  key={actionIndex}
                  className={`px-3 py-1.5 text-xs font-medium rounded-macos transition-all duration-150 ${
                    action.variant === 'secondary'
                      ? 'bg-black bg-opacity-10 hover:bg-opacity-20'
                      : 'bg-current text-white hover:opacity-90'
                  }`}
                  onClick={action.onClick}
                >
                  {action.label}
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

export default ToastContainer;
