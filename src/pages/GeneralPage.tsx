import React from 'react';
import { MacToggle } from '../components/macos';
import { useSettings } from '../services/settings';

const GeneralPage: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">General Settings</h1>
        <p className="page-description">
          Configure general application behavior and startup options.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Startup & Behavior</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Launch at Login</h3>
            <p className="setting-description">
              Automatically start ASR Pro when you log into your system
            </p>
          </div>
          <div className="setting-control">
            <MacToggle
              checked={settings.launchAtLogin}
              onChange={(checked) => updateSetting('launchAtLogin', checked)}
              size="medium"
            />
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Start Minimized</h3>
            <p className="setting-description">
              Start the application minimized to the system tray
            </p>
          </div>
          <div className="setting-control">
            <MacToggle
              checked={settings.startMinimized}
              onChange={(checked) => updateSetting('startMinimized', checked)}
              size="medium"
            />
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Auto-unload Model</h3>
            <p className="setting-description">
              Automatically unload AI models after 30 minutes of inactivity to save memory
            </p>
          </div>
          <div className="setting-control">
            <MacToggle
              checked={settings.autoUnloadModel}
              onChange={(checked) => updateSetting('autoUnloadModel', checked)}
              size="medium"
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Application Info</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Version</h3>
            <p className="setting-description">ASR Pro v1.0.0</p>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Storage Location</h3>
            <p className="setting-description">
              Models and configuration are stored in your user data directory
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default GeneralPage;
