import React, { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

const SystemTray: React.FC = () => {
  useEffect(() => {
    const setupTrayMenu = async () => {
      try {
        // Create the tray menu
        await invoke("create_tray_menu");
      } catch (error) {
        console.error("Failed to setup system tray:", error);
      }
    };

    setupTrayMenu();
  }, []);

  // This component doesn't render anything visible
  // It just sets up the system tray functionality
  return null;
};

export default SystemTray;
