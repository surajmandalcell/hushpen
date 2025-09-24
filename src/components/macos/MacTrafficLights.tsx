import React, { useState } from "react";
import { X, Minus, Square } from "lucide-react";

interface MacTrafficLightsProps {
  onClose?: () => void;
  onMinimize?: () => void;
  onMaximize?: () => void;
  showLabels?: boolean;
}

const MacTrafficLights: React.FC<MacTrafficLightsProps> = ({
  onClose,
  onMinimize,
  onMaximize,
  showLabels = false,
}) => {
  const [hoveredButton, setHoveredButton] = useState<string | null>(null);

  const handleClose = async () => {
    if (onClose) {
      onClose();
    } else {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().close();
      } catch (error) {
        console.error("Failed to close window:", error);
      }
    }
  };

  const handleMinimize = async () => {
    if (onMinimize) {
      onMinimize();
    } else {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().minimize();
      } catch (error) {
        console.error("Failed to minimize window:", error);
      }
    }
  };

  const handleMaximize = async () => {
    if (onMaximize) {
      onMaximize();
    } else {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const window = getCurrentWindow();
        const isMaximized = await window.isMaximized();
        if (isMaximized) {
          await window.unmaximize();
        } else {
          await window.maximize();
        }
      } catch (error) {
        console.error("Failed to toggle maximize:", error);
      }
    }
  };

  return (
    <div className="traffic-lights">
      <button
        className="traffic-light close"
        onClick={handleClose}
        onMouseEnter={() => setHoveredButton("close")}
        onMouseLeave={() => setHoveredButton(null)}
        aria-label="Close"
      >
        {(hoveredButton === "close" || showLabels) && (
          <X size={8} className="text-white" />
        )}
      </button>

      <button
        className="traffic-light minimize"
        onClick={handleMinimize}
        onMouseEnter={() => setHoveredButton("minimize")}
        onMouseLeave={() => setHoveredButton(null)}
        aria-label="Minimize"
      >
        {(hoveredButton === "minimize" || showLabels) && (
          <Minus size={8} className="text-white" />
        )}
      </button>

      <button
        className="traffic-light maximize"
        onClick={handleMaximize}
        onMouseEnter={() => setHoveredButton("maximize")}
        onMouseLeave={() => setHoveredButton(null)}
        aria-label="Maximize"
      >
        {(hoveredButton === "maximize" || showLabels) && (
          <Square size={6} className="text-white" />
        )}
      </button>
    </div>
  );
};

export default MacTrafficLights;
