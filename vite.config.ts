import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(async () => ({
  base: "./",
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 4270,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/release/**", "**/tmp/**"],
    },
  },
}));
