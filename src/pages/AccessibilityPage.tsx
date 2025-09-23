import React, { useState } from "react";
import { useAccessibility } from "../services/accessibility";
import { platformService } from "../services/platform";

const AccessibilityPage: React.FC = () => {
  const { settings, updateSettings, announce } = useAccessibility();
  const [testMessage, setTestMessage] = useState("");

  const handleSettingChange = (key: keyof typeof settings, value: any) => {
    updateSettings({ [key]: value });
    announce({
      message: `Setting ${key} changed to ${value}`,
      priority: "medium",
      category: "settings",
    });
  };

  const handleTestAnnouncement = () => {
    if (testMessage.trim()) {
      announce({
        message: testMessage,
        priority: "medium",
        category: "test",
      });
      setTestMessage("");
    }
  };

  const fontSizeOptions = [
    { id: "small", name: "Small (12px)" },
    { id: "medium", name: "Medium (14px)" },
    { id: "large", name: "Large (16px)" },
    { id: "extra-large", name: "Extra Large (18px)" },
  ];

  const platformInfo = platformService.getPlatformInfo();

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Accessibility Settings</h1>
        <p className="page-description">
          Configure accessibility features to make ASR Pro more usable for
          everyone. Platform: {platformInfo.platform.toUpperCase()}
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Visual Preferences</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">High Contrast Mode</h3>
            <p className="setting-description">
              Increase contrast for better visibility
            </p>
          </div>
          <div className="setting-control">
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.highContrast}
                onChange={(e) =>
                  handleSettingChange("highContrast", e.target.checked)
                }
              />
              <span className="slider"></span>
            </label>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Font Size</h3>
            <p className="setting-description">
              Adjust text size for better readability
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={settings.fontSize}
              onChange={(e) =>
                handleSettingChange("fontSize", e.target.value as any)
              }
            >
              {fontSizeOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Focus Outline</h3>
            <p className="setting-description">
              Show focus indicators for keyboard navigation
            </p>
          </div>
          <div className="setting-control">
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.focusOutline}
                onChange={(e) =>
                  handleSettingChange("focusOutline", e.target.checked)
                }
              />
              <span className="slider"></span>
            </label>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Motion & Animation</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Reduced Motion</h3>
            <p className="setting-description">
              Disable or reduce animations and transitions
            </p>
          </div>
          <div className="setting-control">
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.reducedMotion}
                onChange={(e) =>
                  handleSettingChange("reducedMotion", e.target.checked)
                }
              />
              <span className="slider"></span>
            </label>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Screen Reader Support</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Announcements</h3>
            <p className="setting-description">
              Enable spoken announcements for screen readers
            </p>
          </div>
          <div className="setting-control">
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.announcements}
                onChange={(e) =>
                  handleSettingChange("announcements", e.target.checked)
                }
              />
              <span className="slider"></span>
            </label>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Test Announcement</h3>
            <p className="setting-description">
              Test screen reader announcements with custom message
            </p>
          </div>
          <div className="setting-control">
            <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
              <input
                type="text"
                className="input"
                placeholder="Enter test message..."
                value={testMessage}
                onChange={(e) => setTestMessage(e.target.value)}
                style={{ flex: 1 }}
              />
              <button
                className="button"
                onClick={handleTestAnnouncement}
                disabled={!testMessage.trim()}
              >
                Announce
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Keyboard Navigation</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Keyboard Shortcuts</h3>
            <ul className="keyboard-shortcuts">
              <li>
                <kbd>Tab</kbd> - Navigate between elements
              </li>
              <li>
                <kbd>Space</kbd> - Start/stop recording
              </li>
              <li>
                <kbd>Esc</kbd> - Cancel/close dialogs
              </li>
              <li>
                <kbd>{platformInfo.getCommandKey()}</kbd> + <kbd>,</kbd> - Open
                preferences
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Accessibility Tips</h2>

        <div className="setting-row">
          <div className="setting-info">
            <ul className="accessibility-tips">
              <li>
                Use a screen reader like NVDA (Windows), VoiceOver (macOS), or
                Orca (Linux)
              </li>
              <li>Enable high contrast mode for better visibility</li>
              <li>Reduce motion if animations cause discomfort</li>
              <li>Increase font size if text is hard to read</li>
              <li>Use keyboard navigation instead of mouse when possible</li>
            </ul>
          </div>
        </div>
      </div>

      <style
        dangerouslySetInnerHTML={{
          __html: `
        .keyboard-shortcuts {
          list-style: none;
          padding: 0;
          margin: 0;
        }

        .keyboard-shortcuts li {
          padding: 8px 0;
          display: flex;
          align-items: center;
          gap: 12px;
        }

        .keyboard-shortcuts kbd {
          background: var(--secondary-bg);
          border: 1px solid var(--border-color);
          border-radius: 4px;
          padding: 4px 8px;
          font-family: monospace;
          font-size: 12px;
          box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
        }

        .accessibility-tips {
          list-style: none;
          padding: 0;
          margin: 0;
        }

        .accessibility-tips li {
          padding: 8px 0;
          position: relative;
          padding-left: 24px;
        }

        .accessibility-tips li:before {
          content: "♿";
          position: absolute;
          left: 0;
          top: 8px;
          font-size: 16px;
        }
      `,
        }}
      />
    </div>
  );
};

export default AccessibilityPage;
