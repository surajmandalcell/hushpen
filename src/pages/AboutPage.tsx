import React from 'react';
import { PalPanel, PalPanelHeader, PalText, PalCard, PalButton } from '../components/palantirui';

const AboutPage: React.FC = () => {
  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="About ASR Pro"
        subtitle="Information about ASR Pro and its capabilities."
        withBorder={false}
      />

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Application Information</PalText>
        
        <div className="space-y-3">
          <div className="flex justify-between py-2">
            <PalText weight="medium">Version</PalText>
            <PalText variant="muted">ASR Pro v1.0.0</PalText>
          </div>

          <div className="flex justify-between py-2">
            <PalText weight="medium">Build</PalText>
            <PalText variant="muted">Built with React + Tauri</PalText>
          </div>

          <div className="flex justify-between py-2">
            <PalText weight="medium">AI Models</PalText>
            <PalText variant="muted" className="max-w-xs text-right">
              Powered by OpenAI Whisper and NVIDIA Parakeet models
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
        <PalText size="lg" weight="semibold">Features</PalText>
        
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">🎤</div>
              <PalText weight="medium">Real-time Transcription</PalText>
              <PalText size="sm" variant="muted">Live speech-to-text with global hotkeys</PalText>
            </div>
          </PalCard>
          
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">📁</div>
              <PalText weight="medium">File Processing</PalText>
              <PalText size="sm" variant="muted">Batch transcription of audio files</PalText>
            </div>
          </PalCard>
          
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">🧠</div>
              <PalText weight="medium">Multiple AI Models</PalText>
              <PalText size="sm" variant="muted">Choose from various Whisper and Parakeet models</PalText>
            </div>
          </PalCard>
          
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">🌐</div>
              <PalText weight="medium">Multi-language Support</PalText>
              <PalText size="sm" variant="muted">Support for 10+ languages with auto-detection</PalText>
            </div>
          </PalCard>
          
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">⚡</div>
              <PalText weight="medium">Hardware Acceleration</PalText>
              <PalText size="sm" variant="muted">GPU acceleration with CUDA and Metal support</PalText>
            </div>
          </PalCard>
          
          <PalCard variant="default" padding="md" withGlow={true} withCornerMarkers={true}>
            <div className="text-center space-y-2">
              <div className="text-2xl">🎨</div>
              <PalText weight="medium">Dark Theme</PalText>
              <PalText size="sm" variant="muted">Beautiful dark interface with smooth animations</PalText>
            </div>
          </PalCard>
        </div>
      </PalCard>

      <PalCard
        variant="default"
        padding="lg"
        withGlow={true}
        withCornerMarkers={true}
        className="space-y-4"
      >
        <PalText size="lg" weight="semibold">Support</PalText>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Documentation</PalText>
              <PalText size="sm" variant="muted">
                Visit our documentation for help and tutorials
              </PalText>
            </div>
            <PalButton variant="primary" size="sm" withGlow={true} withCornerMarkers={true}>
              Open Docs
            </PalButton>
          </div>

          <div className="flex items-center justify-between py-3">
            <div>
              <PalText weight="medium">Report Issues</PalText>
              <PalText size="sm" variant="muted">
                Found a bug? Report it on our GitHub repository
              </PalText>
            </div>
            <PalButton variant="secondary" size="sm" withGlow={true} withCornerMarkers={true}>
              Report Bug
            </PalButton>
          </div>
        </div>
      </PalCard>
    </div>
  );
};

export default AboutPage;
