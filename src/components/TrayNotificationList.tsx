import React from 'react';
import { X, CheckCircle, AlertCircle, AlertTriangle, Info, Clock } from 'lucide-react';
import { useTrayNotifications } from '../services/trayNotifications';
import './TrayNotificationList.css';

const TrayNotificationList: React.FC = () => {
  const { notifications, removeNotification, markAsRead } = useTrayNotifications();

  const getNotificationIcon = (type: string) => {
    switch (type) {
      case 'success':
        return <CheckCircle size={16} />;
      case 'warning':
        return <AlertTriangle size={16} />;
      case 'error':
        return <AlertCircle size={16} />;
      default:
        return <Info size={16} />;
    }
  };

  const getNotificationStyles = (type: string) => {
    switch (type) {
      case 'success':
        return { borderLeftColor: '#4caf50' };
      case 'warning':
        return { borderLeftColor: '#ff9800' };
      case 'error':
        return { borderLeftColor: '#f44336' };
      default:
        return { borderLeftColor: '#2196f3' };
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffInMinutes = Math.floor((now.getTime() - date.getTime()) / (1000 * 60));

    if (diffInMinutes < 1) return 'Just now';
    if (diffInMinutes < 60) return `${diffInMinutes}m ago`;
    if (diffInMinutes < 1440) return `${Math.floor(diffInMinutes / 60)}h ago`;
    return date.toLocaleDateString();
  };

  if (notifications.length === 0) {
    return null;
  }

  return (
    <div className="tray-notification-list">
      <div className="tray-notification-list__header">
        <h3 className="tray-notification-list__title">
          Notifications ({notifications.length})
        </h3>
        <button
          className="tray-notification-list__clear"
          onClick={() => notifications.forEach(n => markAsRead(n.id))}
          aria-label="Mark all as read"
        >
          Mark all read
        </button>
      </div>

      <div className="tray-notification-list__content">
        {notifications.map((notification) => (
          <div
            key={notification.id}
            className="tray-notification-item"
            style={getNotificationStyles(notification.type)}
          >
            <div className="tray-notification-item__icon">
              {getNotificationIcon(notification.type)}
            </div>

            <div className="tray-notification-item__content">
              <div className="tray-notification-item__header">
                <h4 className="tray-notification-item__title">
                  {notification.title}
                </h4>
                <div className="tray-notification-item__meta">
                  <span className="tray-notification-item__timestamp">
                    <Clock size={12} />
                    {formatTimestamp(notification.timestamp)}
                  </span>
                  <button
                    className="tray-notification-item__close"
                    onClick={() => removeNotification(notification.id)}
                    aria-label="Remove notification"
                  >
                    <X size={14} />
                  </button>
                </div>
              </div>

              {notification.message && (
                <p className="tray-notification-item__message">
                  {notification.message}
                </p>
              )}

              {notification.actions && notification.actions.length > 0 && (
                <div className="tray-notification-item__actions">
                  {notification.actions.map((action, index) => (
                    <button
                      key={index}
                      className="tray-notification-item__action"
                      onClick={action.onClick}
                    >
                      {action.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default TrayNotificationList;
