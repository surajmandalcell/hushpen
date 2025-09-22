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

const ComponentDemo: React.FC = () => {
  const [toggleChecked, setToggleChecked] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [dropdownValue, setDropdownValue] = useState("");
  const [progress, setProgress] = useState(65);
  const [contextMenuOpen, setContextMenuOpen] = useState(false);
  const [contextMenuPosition, setContextMenuPosition] = useState({ x: 0, y: 0 });
  const [modalOpen, setModalOpen] = useState(false);
  const [segmentedValue, setSegmentedValue] = useState("option1");

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
    </div>
  );
};

export default ComponentDemo;
