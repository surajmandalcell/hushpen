import React, { useEffect, useState } from 'react';
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useToast } from '../services/toast';
import './ToastContainer.css';

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

  const getToastStyles = (type: string) => {
    switch (type) {
      case 'success':
        return { background: 'rgba(76, 175, 80, 0.9)', border: 'rgba(76, 175, 80, 0.3)' };
      case 'warning':
        return { background: 'rgba(255, 152, 0, 0.9)', border: 'rgba(255, 152, 0, 0.3)' };
      case 'error':
        return { background: 'rgba(244, 67, 54, 0.9)', border: 'rgba(244, 67, 54, 0.3)' };
      default:
        return { background: 'rgba(33, 150, 243, 0.9)', border: 'rgba(33, 150, 243, 0.3)' };
    }
  };

  return (
    <div className="toast-container">
      {toasts.map((toast, index) => (
        <div
          key={toast.id}
          className={`toast ${toast.type}`}
          style={{
            transform: `translateY(${index * 10}px)`,
            ...getToastStyles(toast.type),
          }}
        >
          <div className="toast__content">
            <div className="toast__icon">
              {getToastIcon(toast.type)}
            </div>

            <div className="toast__text">
              <div className="toast__title">{toast.title}</div>
              {toast.message && (
                <div className="toast__message">{toast.message}</div>
              )}
            </div>

            <button
              className="toast__close"
              onClick={() => dismiss(toast.id)}
              aria-label="Close notification"
            >
              <X size={16} />
            </button>
          </div>

          {toast.actions && toast.actions.length > 0 && (
            <div className="toast__actions">
              {toast.actions.map((action, actionIndex) => (
                <button
                  key={actionIndex}
                  className={`toast__action toast__action--${action.variant || 'primary'}`}
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
