import React, { useEffect, useState } from 'react';
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useToast } from '../services/toast';
import { PalCard, PalButton, PalText } from './palantirui';

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

  return (
    <div className="fixed top-4 right-4 z-50 space-y-3 pointer-events-none">
      {toasts.map((toast, index) => (
        <PalCard
          key={toast.id}
          variant="elevated"
          padding="md"
          withGlow={true}
          withCornerMarkers={true}
          className={`pointer-events-auto max-w-sm ${
            toast.type === 'success' ? 'border-palantir-accent-green' :
            toast.type === 'warning' ? 'border-palantir-accent-orange' :
            toast.type === 'error' ? 'border-palantir-accent-red' :
            'border-palantir-accent-blue'
          }`}
          style={{
            transform: `translateY(${index * 4}px)`,
          }}
        >
          <div className="flex items-start space-x-3">
            <div className="flex-shrink-0 mt-0.5 text-current">
              {getToastIcon(toast.type)}
            </div>

            <div className="flex-1 min-w-0">
              <PalText weight="medium" className="text-palantir-zinc-900 dark:text-palantir-zinc-100">
                {toast.title}
              </PalText>
              {toast.message && (
                <PalText size="sm" variant="muted" className="mt-1">
                  {toast.message}
                </PalText>
              )}
            </div>

            <PalButton
              variant="ghost"
              size="sm"
              onClick={() => dismiss(toast.id)}
              aria-label="Close notification"
              className="flex-shrink-0 p-1 -m-1"
            >
              <X size={16} />
            </PalButton>
          </div>

          {toast.actions && toast.actions.length > 0 && (
            <div className="flex space-x-2 mt-3 pt-3 border-t border-palantir-zinc-200 dark:border-palantir-zinc-700">
              {toast.actions.map((action, actionIndex) => (
                <PalButton
                  key={`${action.label}-${actionIndex}`}
                  variant={action.variant === 'secondary' ? 'secondary' : 'primary'}
                  size="sm"
                  onClick={action.onClick}
                >
                  {action.label}
                </PalButton>
              ))}
            </div>
          )}
        </PalCard>
      ))}
    </div>
  );
};

export default ToastContainer;
