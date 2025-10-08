import React from 'react';
import { useSettings } from '../services/settings';
import { PalPanelHeader, PalText, PalCard } from '../components/palantirui';
import { TacticalToggle } from '../components/TacticalToggle';

const GeneralPage: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  return (
    <div className="pal-section pal-container pal-card-spacing">
      <PalPanelHeader
        title="General Settings"
        subtitle="Configure general application behavior and startup options."
        withBorder={false}
        className="pal-mb-xl"
      />

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="pal-p-lg"
      >
        <div className="pal-card-spacing">
          <PalText size="lg" weight="semibold" className="pal-mb-lg">Startup & Behavior</PalText>
          
          <div className="pal-form-row pal-p-md border-b">
            <div>
              <PalText weight="medium">Launch at Login</PalText>
              <PalText size="sm" variant="muted">
                Automatically start ASR Pro when you log into your system
              </PalText>
            </div>
            <TacticalToggle
              checked={settings.launchAtLogin}
              onChange={(checked) => updateSetting('launchAtLogin', checked)}
              size="md"
            />
          </div>

          <div className="pal-form-row pal-p-md border-b">
            <div>
              <PalText weight="medium">Start Minimized</PalText>
              <PalText size="sm" variant="muted">
                Start the application minimized to the system tray
              </PalText>
            </div>
            <TacticalToggle
              checked={settings.startMinimized}
              onChange={(checked) => updateSetting('startMinimized', checked)}
              size="md"
            />
          </div>

          <div className="pal-form-row pal-p-md">
            <div>
              <PalText weight="medium">Auto-unload Model</PalText>
              <PalText size="sm" variant="muted">
                Automatically unload AI models after 30 minutes of inactivity to save memory
              </PalText>
            </div>
            <TacticalToggle
              checked={settings.autoUnloadModel}
              onChange={(checked) => updateSetting('autoUnloadModel', checked)}
              size="md"
            />
          </div>
        </div>
      </PalCard>

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="pal-p-lg"
      >
        <PalText size="lg" weight="semibold" className="pal-mb-lg">Application Info</PalText>
        
        <div className="pal-list-spacing">
          <div className="pal-form-row pal-p-sm">
            <PalText weight="medium">Version</PalText>
            <PalText variant="muted">ASR Pro v1.0.0</PalText>
          </div>

          <div className="pal-form-row pal-p-sm">
            <PalText weight="medium">Storage Location</PalText>
            <PalText size="sm" variant="muted" className="max-w-xs text-right">
              Models and configuration are stored in your user data directory
            </PalText>
          </div>
        </div>
      </PalCard>
    </div>
  );
};

export default GeneralPage;
