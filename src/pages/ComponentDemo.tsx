import React, { useState } from "react";
import { Search, User, Settings, Trash2, Edit, Copy } from "lucide-react";
import {
  MacButton,
  MacToggle,
  MacInput,
  MacProgress,
  MacDropdown,
  MacTrafficLights,
  MacScrollbar,
  MacContextMenu,
  MacModal,
  MacSegmented,
} from "../components/macos";
import { useToast } from "../services/toast";
import { useRecording } from "../services/recordingManager";
import { useFileQueue } from "../services/fileQueue";
import { useTrayNotifications } from "../services/trayNotifications";

const ComponentDemo: React.FC = () => {
  const [toggleChecked, setToggleChecked] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [dropdownValue, setDropdownValue] = useState("");
  const [progress, setProgress] = useState(65);
  const [contextMenuOpen, setContextMenuOpen] = useState(false);
  const [contextMenuPosition, setContextMenuPosition] = useState({ x: 0, y: 0 });
  const [modalOpen, setModalOpen] = useState(false);
  const [segmentedValue, setSegmentedValue] = useState("option1");

  const { info, success, warning, error } = useToast();
  const { state: recordingState, start, stop, cancel, updateProgress, completeTranscription } = useRecording();
  const { files, stats, addFiles, removeFile, clearQueue, processNext, cancelProcessing } = useFileQueue();
  const { notifications, handleEvent } = useTrayNotifications();

  const dropdownOptions = [
    { value: "option1", label: "First Option" },
    { value: "option2", label: "Second Option" },
    { value: "option3", label: "Third Option" },
    { value: "disabled", label: "Disabled Option", disabled: true },
  ];

  const contextMenuItems = [
    { id: "edit", label: "Edit", icon: <Edit size={14} />, onClick: () => console.log("Edit") },
    { id: "copy", label: "Copy", icon: <Copy size={14} />, onClick: () => console.log("Copy") },
    { id: "separator1", label: "", separator: true },
    { id: "settings", label: "Settings", icon: <Settings size={14} />, onClick: () => console.log("Settings") },
    { id: "separator2", label: "", separator: true },
    { id: "delete", label: "Delete", icon: <Trash2 size={14} />, onClick: () => console.log("Delete") },
  ];

  const segmentedOptions = [
    { value: "option1", label: "Option 1" },
    { value: "option2", label: "Option 2" },
    { value: "option3", label: "Option 3" },
  ];

  const handleContextMenu = (event: React.MouseEvent) => {
    event.preventDefault();
    setContextMenuPosition({ x: event.clientX, y: event.clientY });
    setContextMenuOpen(true);
  };

  return (
    <div style={{ padding: "20px", maxWidth: "600px" }}>
      <h2 style={{ color: "var(--primary-text)", marginBottom: "24px" }}>
        macOS Components Demo
      </h2>

      {/* Traffic Lights */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Traffic Lights
        </h3>
        <MacTrafficLights showLabels />
      </section>

      {/* Buttons */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Buttons
        </h3>
        <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
          <MacButton variant="primary">Primary Button</MacButton>
          <MacButton variant="secondary">Secondary Button</MacButton>
          <MacButton variant="destructive">Delete</MacButton>
          <MacButton variant="primary" size="small">
            Small
          </MacButton>
          <MacButton variant="primary" size="large">
            Large Button
          </MacButton>
        </div>
      </section>

      {/* Toggle Switches */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Toggle Switches
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <MacToggle
            checked={toggleChecked}
            onChange={setToggleChecked}
            label="Enable notifications"
            size="medium"
          />
          <MacToggle
            checked={true}
            onChange={() => {}}
            label="Small toggle"
            size="small"
          />
          <MacToggle
            checked={false}
            onChange={() => {}}
            label="Large toggle"
            size="large"
          />
          <MacToggle
            checked={false}
            onChange={() => {}}
            label="Disabled toggle"
            disabled
          />
        </div>
      </section>

      {/* Input Fields */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Input Fields
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <MacInput
            label="Standard Input"
            placeholder="Enter text here..."
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
          />
          <MacInput
            label="Search Input"
            variant="search"
            placeholder="Search..."
            leftIcon={<Search size={16} />}
          />
          <MacInput
            label="Input with Icon"
            placeholder="Username"
            leftIcon={<User size={16} />}
            rightIcon={<Settings size={16} />}
          />
          <MacInput
            label="Error State"
            placeholder="Invalid input"
            error="This field is required"
            value="invalid"
            onChange={() => {}}
          />
        </div>
      </section>

      {/* Dropdown */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Dropdown
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <MacDropdown
            label="Select Option"
            options={dropdownOptions}
            value={dropdownValue}
            onChange={setDropdownValue}
            placeholder="Choose an option..."
          />
          <MacDropdown
            label="Small Dropdown"
            options={dropdownOptions}
            value=""
            onChange={() => {}}
            size="small"
            placeholder="Small size..."
          />
        </div>
      </section>

      {/* Progress Bars */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Progress Bars
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <MacProgress
            value={progress}
            showLabel
            label="Download Progress"
            color="blue"
          />
          <MacProgress value={85} color="green" size="small" />
          <MacProgress value={45} color="orange" size="large" showLabel />
          <MacProgress
            value={0}
            variant="indeterminate"
            color="blue"
            label="Loading..."
          />
        </div>
        <div style={{ marginTop: "12px" }}>
          <MacButton
            onClick={() => setProgress(Math.min(progress + 10, 100))}
            size="small"
          >
            +10%
          </MacButton>
        </div>
      </section>

      {/* Segmented Control */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Segmented Control
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <MacSegmented
            options={segmentedOptions}
            value={segmentedValue}
            onChange={setSegmentedValue}
            size="medium"
          />
          <MacSegmented
            options={segmentedOptions}
            value="option2"
            onChange={() => {}}
            size="small"
          />
          <MacSegmented
            options={segmentedOptions}
            value="option3"
            onChange={() => {}}
            size="large"
          />
        </div>
      </section>

      {/* Scrollable Area */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Scrollable Area
        </h3>
        <div style={{ height: "200px", border: "1px solid var(--border-color)", borderRadius: "8px" }}>
          <MacScrollbar
            autoHide
            thin
          >
          <div style={{ padding: "16px" }}>
            {Array.from({ length: 20 }, (_, i) => (
              <div key={i} style={{ padding: "8px 0", borderBottom: "1px solid var(--border-color)" }}>
                Item {i + 1} - This is a scrollable content area with macOS-style scrollbars
              </div>
            ))}
          </div>
          </MacScrollbar>
        </div>
      </section>

      {/* Context Menu Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Context Menu
        </h3>
        <div
          style={{
            padding: "20px",
            border: "1px solid var(--border-color)",
            borderRadius: "8px",
            background: "var(--card-background)",
            cursor: "context-menu",
          }}
          onContextMenu={handleContextMenu}
        >
          Right-click here to see the macOS-style context menu
        </div>
      </section>

      {/* Modal Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Modal Dialog
        </h3>
        <MacButton onClick={() => setModalOpen(true)}>
          Open Modal
        </MacButton>
      </section>

      {/* Context Menu Component */}
      <MacContextMenu
        items={contextMenuItems}
        isOpen={contextMenuOpen}
        onClose={() => setContextMenuOpen(false)}
        x={contextMenuPosition.x}
        y={contextMenuPosition.y}
      />

      {/* Modal Component */}
      <MacModal
        isOpen={modalOpen}
        onClose={() => setModalOpen(false)}
        title="macOS Modal Dialog"
        size="medium"
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <p style={{ color: "var(--secondary-text)", margin: 0 }}>
            This is a macOS-style modal dialog with blur effects and native styling.
          </p>
          <div style={{ display: "flex", gap: "12px", justifyContent: "flex-end" }}>
            <MacButton variant="secondary" onClick={() => setModalOpen(false)}>
              Cancel
            </MacButton>
            <MacButton variant="primary" onClick={() => setModalOpen(false)}>
              OK
            </MacButton>
          </div>
        </div>
      </MacModal>

      {/* Toast Notifications Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Toast Notifications
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <MacButton
            variant="primary"
            onClick={() => info("Information", "This is an informational message.")}
          >
            Info Toast
          </MacButton>

          <MacButton
            variant="primary"
            onClick={() => success("Success!", "Operation completed successfully.")}
          >
            Success Toast
          </MacButton>

          <MacButton
            variant="secondary"
            onClick={() => warning("Warning", "Please review the settings before continuing.")}
          >
            Warning Toast
          </MacButton>

          <MacButton
            variant="destructive"
            onClick={() => error("Error", "An error occurred while processing your request.")}
          >
            Error Toast
          </MacButton>

          <MacButton
            variant="secondary"
            onClick={() => info(
              "Persistent Toast",
              "This toast won't auto-dismiss and has actions.",
              {
                persistent: true,
                duration: 0,
                actions: [
                  {
                    label: "Retry",
                    onClick: () => console.log("Retry clicked"),
                    variant: "primary"
                  },
                  {
                    label: "Cancel",
                    onClick: () => console.log("Cancel clicked"),
                    variant: "secondary"
                  }
                ]
              }
            )}
          >
            Persistent Toast with Actions
          </MacButton>
        </div>
      </section>

      {/* Recording Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Recording System
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          {!recordingState.isActive && !recordingState.isTranscribing && (
            <MacButton
              variant="primary"
              onClick={start}
            >
              Start Recording
            </MacButton>
          )}

          {recordingState.isActive && (
            <MacButton
              variant="secondary"
              onClick={stop}
            >
              Stop Recording & Transcribe
            </MacButton>
          )}

          {(recordingState.isActive || recordingState.isTranscribing) && (
            <MacButton
              variant="destructive"
              onClick={cancel}
            >
              Cancel
            </MacButton>
          )}

          {recordingState.isTranscribing && (
            <>
              <MacButton
                variant="secondary"
                onClick={() => updateProgress(recordingState.transcriptionProgress + 10, "Processing chunk...")}
              >
                Update Progress +10%
              </MacButton>

              <MacButton
                variant="primary"
                onClick={completeTranscription}
              >
                Complete Transcription
              </MacButton>
            </>
          )}

          <div style={{
            padding: "16px",
            background: "var(--card-background)",
            borderRadius: "8px",
            border: "1px solid var(--border-color)"
          }}>
            <h4 style={{ color: "var(--primary-text)", margin: "0 0 8px 0" }}>
              Recording Status
            </h4>
            <p style={{ color: "var(--secondary-text)", margin: "0 0 8px 0" }}>
              Active: {recordingState.isActive ? 'Yes' : 'No'}
            </p>
            <p style={{ color: "var(--secondary-text)", margin: "0 0 8px 0" }}>
              Transcribing: {recordingState.isTranscribing ? 'Yes' : 'No'}
            </p>
            <p style={{ color: "var(--secondary-text)", margin: "0 0 8px 0" }}>
              Progress: {recordingState.transcriptionProgress}%
            </p>
            <p style={{ color: "var(--secondary-text)", margin: "0" }}>
              Status: {recordingState.statusText}
            </p>
          </div>
        </div>
      </section>

      {/* File Queue Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          File Queue System
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
            <MacButton
              variant="primary"
              onClick={() => {
                // Create mock files for demo
                const mockFiles = [
                  new File(['mock audio content'], 'demo1.wav', { type: 'audio/wav' }),
                  new File(['mock audio content 2'], 'demo2.mp3', { type: 'audio/mp3' }),
                  new File(['mock audio content 3'], 'demo3.m4a', { type: 'audio/m4a' }),
                ];
                addFiles(mockFiles);
                success('Files Added', '3 demo files added to queue');
              }}
            >
              Add Demo Files
            </MacButton>

            <MacButton
              variant="secondary"
              onClick={processNext}
              disabled={stats.processing > 0 || stats.pending === 0}
            >
              Process Next
            </MacButton>

            <MacButton
              variant="destructive"
              onClick={clearQueue}
              disabled={stats.total === 0}
            >
              Clear Queue
            </MacButton>
          </div>

          {stats.processing > 0 && (
            <MacButton
              variant="destructive"
              onClick={cancelProcessing}
            >
              Cancel Processing
            </MacButton>
          )}

          {/* Queue Stats */}
          <div style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(100px, 1fr))",
            gap: "12px",
            padding: "16px",
            background: "var(--card-background)",
            borderRadius: "8px",
            border: "1px solid var(--border-color)"
          }}>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: "24px", fontWeight: "bold", color: "var(--accent-blue)" }}>
                {stats.total}
              </div>
              <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>Total</div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: "24px", fontWeight: "bold", color: "var(--warning-yellow)" }}>
                {stats.pending}
              </div>
              <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>Pending</div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: "24px", fontWeight: "bold", color: "var(--accent-blue)" }}>
                {stats.processing}
              </div>
              <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>Processing</div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: "24px", fontWeight: "bold", color: "var(--success-green)" }}>
                {stats.completed}
              </div>
              <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>Completed</div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: "24px", fontWeight: "bold", color: "var(--error-red)" }}>
                {stats.errors}
              </div>
              <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>Errors</div>
            </div>
          </div>

          {/* File List */}
          {files.length > 0 && (
            <div style={{
              maxHeight: "300px",
              border: "1px solid var(--border-color)",
              borderRadius: "8px",
              overflow: "hidden"
            }}>
              <MacScrollbar>
                <div style={{ padding: "16px" }}>
                  {files.map((file) => (
                    <div
                      key={file.id}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        padding: "12px",
                        marginBottom: "8px",
                        background: file.status === 'processing' ? 'rgba(0, 122, 204, 0.1)' :
                                   file.status === 'completed' ? 'rgba(76, 175, 80, 0.1)' :
                                   file.status === 'error' ? 'rgba(244, 67, 54, 0.1)' :
                                   'var(--card-background)',
                        borderRadius: "6px",
                        border: `1px solid ${file.status === 'processing' ? 'rgba(0, 122, 204, 0.3)' :
                                            file.status === 'completed' ? 'rgba(76, 175, 80, 0.3)' :
                                            file.status === 'error' ? 'rgba(244, 67, 54, 0.3)' :
                                            'var(--border-color)'}`
                      }}
                    >
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{
                          fontWeight: "500",
                          color: "var(--primary-text)",
                          marginBottom: "4px"
                        }}>
                          {file.name}
                        </div>
                        <div style={{ fontSize: "12px", color: "var(--secondary-text)" }}>
                          {(file.size / 1024).toFixed(1)} KB • Added {new Date(file.addedAt).toLocaleTimeString()}
                        </div>
                        {file.status === 'processing' && (
                          <div style={{ marginTop: "8px" }}>
                            <MacProgress value={file.progress} size="small" color="blue" showLabel />
                          </div>
                        )}
                        {file.result && (
                          <div style={{
                            marginTop: "8px",
                            fontSize: "12px",
                            color: "var(--success-green)",
                            fontStyle: "italic"
                          }}>
                            {file.result}
                          </div>
                        )}
                        {file.error && (
                          <div style={{
                            marginTop: "8px",
                            fontSize: "12px",
                            color: "var(--error-red)",
                            fontStyle: "italic"
                          }}>
                            Error: {file.error}
                          </div>
                        )}
                      </div>
                      <div style={{ marginLeft: "12px" }}>
                        <MacButton
                          size="small"
                          variant="destructive"
                          onClick={() => removeFile(file.id)}
                          disabled={file.status === 'processing'}
                        >
                          Remove
                        </MacButton>
                      </div>
                    </div>
                  ))}
                </div>
              </MacScrollbar>
            </div>
          )}
        </div>
      </section>

      {/* Tray Notifications Demo */}
      <section style={{ marginBottom: "32px" }}>
        <h3 style={{ color: "var(--primary-text)", marginBottom: "16px" }}>
          Tray Notifications
        </h3>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
            <MacButton
              variant="primary"
              onClick={() => handleEvent('recording-started')}
            >
              Recording Started
            </MacButton>

            <MacButton
              variant="primary"
              onClick={() => handleEvent('transcription-completed', { fileName: 'demo.mp3' })}
            >
              Transcription Completed
            </MacButton>

            <MacButton
              variant="secondary"
              onClick={() => handleEvent('model-loaded', { modelName: 'Whisper Base' })}
            >
              Model Loaded
            </MacButton>

            <MacButton
              variant="destructive"
              onClick={() => handleEvent('error', { message: 'Test error notification' })}
            >
              Error Notification
            </MacButton>
          </div>

          <div style={{
            padding: "16px",
            background: "var(--card-background)",
            borderRadius: "8px",
            border: "1px solid var(--border-color)"
          }}>
            <h4 style={{ color: "var(--primary-text)", margin: "0 0 8px 0" }}>
              Notification Status
            </h4>
            <p style={{ color: "var(--secondary-text)", margin: "0 0 8px 0" }}>
              Active Notifications: {notifications.length}
            </p>
            <p style={{ color: "var(--secondary-text)", margin: "0" }}>
              Types: Info({notifications.filter(n => n.type === 'info').length}), Success({notifications.filter(n => n.type === 'success').length}), Error({notifications.filter(n => n.type === 'error').length})
            </p>
          </div>
        </div>
      </section>
    </div>
  );
};

export default ComponentDemo;
