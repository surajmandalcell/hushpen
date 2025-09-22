import React from 'react';

const AboutPage: React.FC = () => {
  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">About ASR Pro</h1>
        <p className="page-description">
          Information about ASR Pro and its capabilities.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Application Information</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Version</h3>
            <p className="setting-description">ASR Pro v1.0.0</p>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Build</h3>
            <p className="setting-description">Built with React + Tauri</p>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">AI Models</h3>
            <p className="setting-description">
              Powered by OpenAI Whisper and NVIDIA Parakeet models
            </p>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Features</h2>
        
        <div className="feature-list">
          <div className="feature-item">
            <h3>🎤 Real-time Transcription</h3>
            <p>Live speech-to-text with global hotkeys</p>
          </div>
          
          <div className="feature-item">
            <h3>📁 File Processing</h3>
            <p>Batch transcription of audio files</p>
          </div>
          
          <div className="feature-item">
            <h3>🧠 Multiple AI Models</h3>
            <p>Choose from various Whisper and Parakeet models</p>
          </div>
          
          <div className="feature-item">
            <h3>🌐 Multi-language Support</h3>
            <p>Support for 10+ languages with auto-detection</p>
          </div>
          
          <div className="feature-item">
            <h3>⚡ Hardware Acceleration</h3>
            <p>GPU acceleration with CUDA and Metal support</p>
          </div>
          
          <div className="feature-item">
            <h3>🎨 Dark Theme</h3>
            <p>Beautiful dark interface with smooth animations</p>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Support</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Documentation</h3>
            <p className="setting-description">
              Visit our documentation for help and tutorials
            </p>
          </div>
          <div className="setting-control">
            <button className="button">Open Docs</button>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Report Issues</h3>
            <p className="setting-description">
              Found a bug? Report it on our GitHub repository
            </p>
          </div>
          <div className="setting-control">
            <button className="button">Report Bug</button>
          </div>
        </div>
      </div>

      <style dangerouslySetInnerHTML={{
        __html: `
        .feature-list {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
          gap: 16px;
          margin-top: 16px;
        }

        .feature-item {
          background-color: var(--secondary-bg);
          border-radius: var(--border-radius);
          padding: 20px;
          border: 1px solid var(--border-color);
        }

        .feature-item h3 {
          color: var(--primary-text);
          font-size: 16px;
          font-weight: 600;
          margin: 0 0 8px 0;
        }

        .feature-item p {
          color: var(--secondary-text);
          font-size: 14px;
          margin: 0;
          line-height: 1.4;
        }
        `
      }} />
    </div>
  );
};

export default AboutPage;
