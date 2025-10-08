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
    <div className="pal-section pal-container pal-card-spacing">
      <PalPanelHeader
        title="Microphone Settings"
        subtitle="Configure audio input settings and test microphone functionality."
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
          <PalText size="lg" weight="semibold" className="pal-mb-lg">Audio Input Device</PalText>

          <div className="pal-form-row pal-p-md border-b">
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

          <div className="pal-form-row pal-p-md border-b">
            <div>
              <PalText weight="medium">Permission Status</PalText>
              <PalText size="sm" variant="muted">
                Microphone access permission status
              </PalText>
            </div>
            <span className="text-pal-accent-green">✓ Granted</span>
          </div>

          <div className="pal-form-row pal-p-md">
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
        className="pal-p-lg"
      >
        <div className="pal-card-spacing">
          <PalText size="lg" weight="semibold" className="pal-mb-lg">{audioHints.title}</PalText>

          <div>
            <PalText weight="medium" className="pal-mb-md">Platform Tips</PalText>
            <div className="pal-list-spacing">
              {audioHints.hints.map((hint) => (
                <div key={`hint-${hint.substring(0, 20)}`} className="flex items-start pal-gap-sm pal-p-sm bg-pal-bg-secondary rounded">
                  <span className="text-pal-accent-orange">💡</span>
                  <PalText size="sm">{hint}</PalText>
                </div>
              ))}
            </div>
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
        <div className="pal-card-spacing">
          <PalText size="lg" weight="semibold" className="pal-mb-lg">Audio Quality</PalText>

          <div className="pal-form-col">
            <div className="pal-form-row pal-p-md border-b">
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

            <div className="pal-form-row pal-p-md">
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
        </div>
      </PalCard>
    </div>
  );
};

export default MicrophonePage;
