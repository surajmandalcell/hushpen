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
    console.log("Exit button clicked - attempting to close application");
    if (onClose) {
      console.log("Using custom onClose handler");
      onClose();
    } else {
      try {
        console.log("Using Tauri API to close window");
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const window = getCurrentWindow();
        console.log("Window object obtained:", window);
        await window.close();
        console.log("Window close command executed");
      } catch (error) {
        console.error("Failed to close window:", error);
        // Fallback: try using Tauri command
        try {
          console.log("Attempting fallback to Tauri quit_app command");
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("quit_app");
          console.log("quit_app command executed");
        } catch (fallbackError) {
          console.error("Fallback also failed:", fallbackError);
        }
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
    <div className="pal-traffic-lights">
      <button
        type="button"
        className="pal-traffic-light pal-traffic-light-close"
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
        type="button"
        className="pal-traffic-light pal-traffic-light-minimize"
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
        type="button"
        className="pal-traffic-light pal-traffic-light-maximize"
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
