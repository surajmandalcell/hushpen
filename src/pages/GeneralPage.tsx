import React from 'react';
import { MacToggle } from '../components/macos';
import { useSettings } from '../services/settings';
import { PalPanel, PalPanelHeader, PalPanelContent, PalText, PalCard, PalButton } from '../components/palantirui';

const GeneralPage: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="General Settings"
        subtitle="Configure general application behavior and startup options."
        withBorder={false}
      />

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-6"
      >
        <div className="space-y-4">
          <PalText size="lg" weight="semibold">Startup & Behavior</PalText>
          
          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Launch at Login</PalText>
              <PalText size="sm" variant="muted">
                Automatically start ASR Pro when you log into your system
              </PalText>
            </div>
            <MacToggle
              checked={settings.launchAtLogin}
              onChange={(checked) => updateSetting('launchAtLogin', checked)}
              size="medium"
            />
          </div>

          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Start Minimized</PalText>
              <PalText size="sm" variant="muted">
                Start the application minimized to the system tray
              </PalText>
            </div>
            <MacToggle
              checked={settings.startMinimized}
              onChange={(checked) => updateSetting('startMinimized', checked)}
              size="medium"
            />
          </div>

          <div className="flex items-center justify-between py-3">
            <div>
              <PalText weight="medium">Auto-unload Model</PalText>
              <PalText size="sm" variant="muted">
                Automatically unload AI models after 30 minutes of inactivity to save memory
              </PalText>
            </div>
            <MacToggle
              checked={settings.autoUnloadModel}
              onChange={(checked) => updateSetting('autoUnloadModel', checked)}
              size="medium"
            />
          </div>
        </div>
      </PalCard>

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Application Info</PalText>
        
        <div className="space-y-3">
          <div className="flex justify-between py-2">
            <PalText weight="medium">Version</PalText>
            <PalText variant="muted">ASR Pro v1.0.0</PalText>
          </div>

          <div className="flex justify-between py-2">
            <PalText weight="medium">Storage Location</PalText>
            <PalText size="sm" variant="muted" className="max-w-xs text-right">
              Models and configuration are stored in your user data directory
            </PalText>
          </div>
        </div>
      </PalCard>

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Debug Controls</PalText>
        
        <div className="space-y-3">
          <PalText weight="medium">Test Tauri Window Controls</PalText>
          <div className="flex gap-2 flex-wrap">
            <PalButton
              variant="primary"
              onClick={async () => {
                console.log("Test close button clicked");
                try {
                  const { getCurrentWindow } = await import('@tauri-apps/api/window');
                  const window = getCurrentWindow();
                  console.log("Closing window via Tauri API");
                  await window.close();
                } catch (error) {
                  console.error("Failed to close window:", error);
                }
              }}
            >
              Close Window
            </PalButton>
            
            <PalButton
              variant="secondary"
              onClick={async () => {
                console.log("Test minimize button clicked");
                try {
                  const { getCurrentWindow } = await import('@tauri-apps/api/window');
                  const window = getCurrentWindow();
                  console.log("Minimizing window via Tauri API");
                  await window.minimize();
                } catch (error) {
                  console.error("Failed to minimize window:", error);
                }
              }}
            >
              Minimize Window
            </PalButton>
            
            <PalButton
              variant="accent"
              onClick={async () => {
                console.log("Test maximize button clicked");
                try {
                  const { getCurrentWindow } = await import('@tauri-apps/api/window');
                  const window = getCurrentWindow();
                  const isMaximized = await window.isMaximized();
                  console.log("Window maximized state:", isMaximized);
                  if (isMaximized) {
                    await window.unmaximize();
                  } else {
                    await window.maximize();
                  }
                } catch (error) {
                  console.error("Failed to toggle maximize:", error);
                }
              }}
            >
              Toggle Maximize
            </PalButton>
          </div>
        </div>
      </PalCard>
    </div>
  );
};

export default GeneralPage;
