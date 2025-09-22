import React, { useState } from 'react';

const KeyboardPage: React.FC = () => {
  const [hotkey, setHotkey] = useState('Ctrl+Shift+R');
  const [enableOverlay, setEnableOverlay] = useState(true);
  const [autoPaste, setAutoPaste] = useState(true);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Keyboard & Hotkeys</h1>
        <p className="page-description">
          Configure global hotkeys and keyboard shortcuts for quick access.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Global Hotkey</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Record Hotkey</h3>
            <p className="setting-description">
              Global keyboard shortcut to start/stop recording
            </p>
          </div>
          <div className="setting-control">
            <input 
              type="text" 
              className="input"
              value={hotkey}
              onChange={(e) => setHotkey(e.target.value)}
              style={{ minWidth: '150px' }}
            />
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Test Hotkey</h3>
            <p className="setting-description">
              Test if the hotkey is working properly
            </p>
          </div>
          <div className="setting-control">
            <button className="button">Test</button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Recording Options</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Enable Overlay</h3>
            <p className="setting-description">
              Show visual overlay during recording to indicate recording status
            </p>
          </div>
          <div className="setting-control">
            <div 
              className={`toggle-switch ${enableOverlay ? 'active' : ''}`}
              onClick={() => setEnableOverlay(!enableOverlay)}
            />
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Auto-paste</h3>
            <p className="setting-description">
              Automatically paste transcribed text after recording
            </p>
          </div>
          <div className="setting-control">
            <div 
              className={`toggle-switch ${autoPaste ? 'active' : ''}`}
              onClick={() => setAutoPaste(!autoPaste)}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Hotkey Status</h2>
        
        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Registration Status</h3>
            <p className="setting-description">
              Current status of global hotkey registration
            </p>
          </div>
          <div className="setting-control">
            <span style={{ color: 'var(--success-green)' }}>✓ Active</span>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Conflicts</h3>
            <p className="setting-description">
              Check for conflicts with other applications
            </p>
          </div>
          <div className="setting-control">
            <span style={{ color: 'var(--success-green)' }}>None detected</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default KeyboardPage;
