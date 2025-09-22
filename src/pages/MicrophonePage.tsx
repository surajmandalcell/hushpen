import React, { useState } from 'react';

const MicrophonePage: React.FC = () => {
  const [selectedDevice, setSelectedDevice] = useState('default');
  const [sampleRate, setSampleRate] = useState('16000');
  const [channels, setChannels] = useState('1');

  const audioDevices = [
    { id: 'default', name: 'Default Microphone' },
    { id: 'device1', name: 'Built-in Microphone' },
    { id: 'device2', name: 'USB Microphone' },
  ];

  const sampleRates = [
    { id: '16000', name: '16 kHz (Recommended)' },
    { id: '22050', name: '22.05 kHz' },
    { id: '44100', name: '44.1 kHz' },
    { id: '48000', name: '48 kHz' },
  ];

  const channelOptions = [
    { id: '1', name: 'Mono (1 channel)' },
    { id: '2', name: 'Stereo (2 channels)' },
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
              {audioDevices.map(device => (
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
            <span style={{ color: 'var(--success-green)' }}>✓ Granted</span>
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
              {sampleRates.map(rate => (
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
              {channelOptions.map(channel => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>
    </div>
  );
};

export default MicrophonePage;
