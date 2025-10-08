import React, { useState } from 'react';
import { PalPanelHeader, PalText, PalCard, PalButton, PalInput } from '../components/palantirui';
import { TacticalToggle } from '../components/TacticalToggle';

const KeyboardPage: React.FC = () => {
  const [hotkey, setHotkey] = useState('Ctrl+Shift+R');
  const [enableOverlay, setEnableOverlay] = useState(true);
  const [autoPaste, setAutoPaste] = useState(true);

  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="Keyboard & Hotkeys"
        subtitle="Configure global hotkeys and keyboard shortcuts for quick access."
        withBorder={false}
      />

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-6">
        <PalText size="lg" weight="semibold">Global Hotkey</PalText>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Record Hotkey</PalText>
            <PalText size="sm" variant="muted">
              Global keyboard shortcut to start/stop recording
            </PalText>
          </div>
          <PalInput
            value={hotkey}
            onChange={(e) => setHotkey(e.target.value)}
            className="w-40"
          />
        </div>

        <div className="flex items-center justify-between py-3">
          <div>
            <PalText weight="medium">Test Hotkey</PalText>
            <PalText size="sm" variant="muted">
              Test if the hotkey is working properly
            </PalText>
          </div>
          <PalButton variant="primary" size="sm">Test</PalButton>
        </div>
      </PalCard>

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
        <PalText size="lg" weight="semibold">Recording Options</PalText>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Enable Overlay</PalText>
            <PalText size="sm" variant="muted">
              Show visual overlay during recording to indicate recording status
            </PalText>
          </div>
          <TacticalToggle
            checked={enableOverlay}
            onChange={(checked) => setEnableOverlay(checked)}
            size="md"
          />
        </div>

        <div className="flex items-center justify-between py-3">
          <div>
            <PalText weight="medium">Auto-paste</PalText>
            <PalText size="sm" variant="muted">
              Automatically paste transcribed text after recording
            </PalText>
          </div>
          <TacticalToggle
            checked={autoPaste}
            onChange={(checked) => setAutoPaste(checked)}
            size="md"
          />
        </div>
      </PalCard>

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
        <PalText size="lg" weight="semibold">Hotkey Status</PalText>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Registration Status</PalText>
            <PalText size="sm" variant="muted">
              Current status of global hotkey registration
            </PalText>
          </div>
          <span className="text-palantir-accent-green flex items-center gap-1">
            <span>✓</span>
            <span>Active</span>
          </span>
        </div>

        <div className="flex items-center justify-between py-3">
          <div>
            <PalText weight="medium">Conflicts</PalText>
            <PalText size="sm" variant="muted">
              Check for conflicts with other applications
            </PalText>
          </div>
          <PalText className="text-palantir-accent-green">None detected</PalText>
        </div>
      </PalCard>
    </div>
  );
};

export default KeyboardPage;
