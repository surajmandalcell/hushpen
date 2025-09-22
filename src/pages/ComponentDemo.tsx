import React, { useState } from "react";
import { Search, User, Settings } from "lucide-react";
import {
  MacButton,
  MacToggle,
  MacInput,
  MacProgress,
  MacDropdown,
  MacTrafficLights,
} from "../components/macos";

const ComponentDemo: React.FC = () => {
  const [toggleChecked, setToggleChecked] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [dropdownValue, setDropdownValue] = useState("");
  const [progress, setProgress] = useState(65);

  const dropdownOptions = [
    { value: "option1", label: "First Option" },
    { value: "option2", label: "Second Option" },
    { value: "option3", label: "Third Option" },
    { value: "disabled", label: "Disabled Option", disabled: true },
  ];

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
    </div>
  );
};

export default ComponentDemo;
