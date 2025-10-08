import React, { useState } from "react";
import { platformAudioService } from "../services/platformAudio";
import { PalPanelHeader, PalText, PalCard, PalButton, PalSelect } from "../components/palantirui";

const MicrophonePage: React.FC = () => {
  const [selectedDevice, setSelectedDevice] = useState("default");
  const [sampleRate, setSampleRate] = useState("16000");
  const [channels, setChannels] = useState("1");

  // Platform-specific data
  const audioHints = platformAudioService.getPlatformAudioHints();
  const sampleRateOptions = platformAudioService.getAudioQualityOptions();

  const audioDevices = [
    { id: "default", name: "Default Microphone" },
    { id: "device1", name: "Built-in Microphone" },
    { id: "device2", name: "USB Microphone" },
  ];

  const channelOptions = [
    { id: "1", name: "Mono (1 channel)" },
    { id: "2", name: "Stereo (2 channels)" },
  ];

  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="Microphone Settings"
        subtitle="Configure audio input settings and test microphone functionality."
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
          <PalText size="lg" weight="semibold">Audio Input Device</PalText>

          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Microphone</PalText>
              <PalText size="sm" variant="muted">
                Select the audio input device to use for recording
              </PalText>
            </div>
            <PalSelect
              value={selectedDevice}
              onChange={(e) => setSelectedDevice(e.target.value)}
            >
              {audioDevices.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.name}
                </option>
              ))}
            </PalSelect>
          </div>

          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Permission Status</PalText>
              <PalText size="sm" variant="muted">
                Microphone access permission status
              </PalText>
            </div>
            <span className="text-palantir-accent-green">✓ Granted</span>
          </div>

          <div className="flex items-center justify-between py-3">
            <div>
              <PalText weight="medium">Test Microphone</PalText>
              <PalText size="sm" variant="muted">
                Test your microphone to ensure it's working properly
              </PalText>
            </div>
            <PalButton variant="primary" size="sm" withGlow={true} withCornerMarkers={true}>
              Test
            </PalButton>
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
        <PalText size="lg" weight="semibold">{audioHints.title}</PalText>

        <div>
          <PalText weight="medium" className="mb-3">Platform Tips</PalText>
          <div className="space-y-2">
            {audioHints.hints.map((hint, index) => (
              <div key={`hint-${index}`} className="flex items-start gap-2 p-2 bg-palantir-zinc-50 dark:bg-palantir-zinc-800 rounded">
                <span className="text-palantir-accent-orange">💡</span>
                <PalText size="sm">{hint}</PalText>
              </div>
            ))}
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
        <PalText size="lg" weight="semibold">Audio Quality</PalText>

        <div className="space-y-4">
          <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
            <div>
              <PalText weight="medium">Sample Rate</PalText>
              <PalText size="sm" variant="muted">
                Audio sampling rate (higher = better quality, larger files)
              </PalText>
            </div>
            <PalSelect
              value={sampleRate}
              onChange={(e) => setSampleRate(e.target.value)}
            >
              {sampleRateOptions.map((rate) => (
                <option key={rate.id} value={rate.id}>
                  {rate.name}
                </option>
              ))}
            </PalSelect>
          </div>

          <div className="flex items-center justify-between py-3">
            <div>
              <PalText weight="medium">Channels</PalText>
              <PalText size="sm" variant="muted">
                Number of audio channels to record
              </PalText>
            </div>
            <PalSelect
              value={channels}
              onChange={(e) => setChannels(e.target.value)}
            >
              {channelOptions.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))}
            </PalSelect>
          </div>
        </div>
      </PalCard>
    </div>
  );
};

export default MicrophonePage;
