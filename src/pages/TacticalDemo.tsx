import React, { useState } from 'react';
import { PalPanel, PalPanelHeader, PalPanelContent, PalText, PalCard, PalButton } from '../components/palantirui';
import { TacticalWindowControls } from '../components/TacticalWindowControls';
import { TacticalToggle } from '../components/TacticalToggle';

const TacticalDemo: React.FC = () => {
  const [toggleChecked, setToggleChecked] = useState(false);
  const [progress, setProgress] = useState(30);

  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="Tactical UI Components"
        subtitle="Compact, tactical-styled interface components for ASR Pro"
        withBorder={false}
      />

      {/* Window Controls */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Window Controls</PalText>
        <div className="flex items-center gap-4 p-3 bg-palantir-zinc-100 dark:bg-palantir-zinc-800 rounded-pal">
          <TacticalWindowControls />
          <PalText size="sm" variant="muted">
            Tactical window controls with compact design
          </PalText>
        </div>
      </PalCard>

      {/* Tactical Buttons */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Tactical Buttons</PalText>
        <div className="flex gap-2 flex-wrap">
          <PalButton variant="primary">Primary Action</PalButton>
          <PalButton variant="secondary">Secondary</PalButton>
          <PalButton variant="accent">Accent</PalButton>
          <PalButton variant="muted">Muted</PalButton>
        </div>
      </PalCard>

      {/* Tactical Toggles */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Tactical Toggles</PalText>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <PalText>Feature enabled</PalText>
            <TacticalToggle
              checked={toggleChecked}
              onChange={setToggleChecked}
              size="md"
            />
          </div>
          <div className="flex items-center justify-between">
            <PalText>Secondary option</PalText>
            <TacticalToggle checked={true} size="md" />
          </div>
          <div className="flex items-center justify-between">
            <PalText>Disabled state</PalText>
            <TacticalToggle checked={false} size="md" disabled />
          </div>
        </div>
      </PalCard>

      {/* Tactical Progress */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Progress Indicators</PalText>
        <div className="space-y-3">
          <div className="space-y-2">
            <div className="flex justify-between items-center">
              <PalText size="sm">Processing Progress</PalText>
              <PalText size="sm" variant="muted">{progress}%</PalText>
            </div>
            <div className="w-full bg-palantir-zinc-200 dark:bg-palantir-zinc-700 rounded-full h-2">
              <div 
                className="bg-palantir-accent-blue h-2 rounded-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
          </div>
          <div className="flex gap-2">
            <PalButton
              variant="secondary"
              onClick={() => setProgress(Math.max(0, progress - 10))}
              size="sm"
            >
              -10%
            </PalButton>
            <PalButton
              variant="primary"
              onClick={() => setProgress(Math.min(100, progress + 10))}
              size="sm"
            >
              +10%
            </PalButton>
          </div>
        </div>
      </PalCard>

      {/* Status Indicators */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Status Indicators</PalText>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-green-500"></div>
            <PalText size="sm">System Online</PalText>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-yellow-500"></div>
            <PalText size="sm">Processing</PalText>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-red-500"></div>
            <PalText size="sm">Error Detected</PalText>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-blue-500"></div>
            <PalText size="sm">Recording Active</PalText>
          </div>
        </div>
      </PalCard>

      {/* Compact Layout Example */}
      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Compact Layout Example</PalText>
        <div className="bg-palantir-zinc-100 dark:bg-palantir-zinc-800 p-4 rounded-pal">
          <div className="flex items-center justify-between mb-3">
            <PalText weight="medium">Audio Processing</PalText>
            <TacticalToggle checked={true} size="sm" />
          </div>
          <div className="space-y-2">
            <div className="flex justify-between text-sm">
              <PalText variant="muted">Model Status</PalText>
              <PalText size="sm">Loaded</PalText>
            </div>
            <div className="flex justify-between text-sm">
              <PalText variant="muted">Accuracy</PalText>
              <PalText size="sm">94.2%</PalText>
            </div>
            <div className="flex justify-between text-sm">
              <PalText variant="muted">Processing Speed</PalText>
              <PalText size="sm">1.2x</PalText>
            </div>
          </div>
          <div className="flex gap-2 mt-3">
            <PalButton variant="primary" size="sm">Start</PalButton>
            <PalButton variant="secondary" size="sm">Configure</PalButton>
          </div>
        </div>
      </PalCard>
    </div>
  );
};

export default TacticalDemo;