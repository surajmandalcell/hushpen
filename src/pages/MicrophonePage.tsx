import React, { useState } from "react";
import { platformAudioService } from "../services/platformAudio";

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
    <div>
      <div className="page-header">
        <h1 className="page-title">Microphone Settings</h1>
        <p className="page-description">
          Configure audio input settings and test microphone functionality.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Audio Input Device</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Microphone</h3>
            <p className="setting-description">
              Select the audio input device to use for recording
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={selectedDevice}
              onChange={(e) => setSelectedDevice(e.target.value)}
            >
              {audioDevices.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Permission Status</h3>
            <p className="setting-description">
              Microphone access permission status
            </p>
          </div>
          <div className="setting-control">
            <span style={{ color: "var(--success-green)" }}>✓ Granted</span>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Test Microphone</h3>
            <p className="setting-description">
              Test your microphone to ensure it's working properly
            </p>
          </div>
          <div className="setting-control">
            <button className="button">Test</button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">{audioHints.title}</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Platform Tips</h3>
            <ul className="platform-tips">
              {audioHints.hints.map((hint, index) => (
                <li key={index}>{hint}</li>
              ))}
            </ul>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Audio Quality</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Sample Rate</h3>
            <p className="setting-description">
              Audio sampling rate (higher = better quality, larger files)
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={sampleRate}
              onChange={(e) => setSampleRate(e.target.value)}
            >
              {sampleRateOptions.map((rate) => (
                <option key={rate.id} value={rate.id}>
                  {rate.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Channels</h3>
            <p className="setting-description">
              Number of audio channels to record
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={channels}
              onChange={(e) => setChannels(e.target.value)}
            >
              {channelOptions.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>

      <style
        dangerouslySetInnerHTML={{
          __html: `
        .platform-tips {
          list-style: none;
          padding: 0;
          margin: 0;
        }

        .platform-tips li {
          padding: 8px 0;
          border-bottom: 1px solid var(--border-color);
          position: relative;
          padding-left: 20px;
        }

        .platform-tips li:before {
          content: "💡";
          position: absolute;
          left: 0;
          top: 8px;
        }

        .platform-tips li:last-child {
          border-bottom: none;
        }
      `,
        }}
      />
    </div>
  );
};

export default MicrophonePage;
