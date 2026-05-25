import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  base: "./",
  plugins: [react()],
  publicDir: "public",
  root: "docs-site",
  build: {
    emptyOutDir: true,
    outDir: "../docs",
  },
  server: {
    host: "127.0.0.1",
    port: 48231,
    strictPort: true,
    watch: {
      ignored: ["**/release/**", "**/tmp/**"],
    },
  },
  preview: {
    host: "127.0.0.1",
    port: 48232,
    strictPort: true,
  },
});
