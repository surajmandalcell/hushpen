import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const CSP_DIRECTIVES = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "media-src 'self' blob: asrpro-media:",
  "font-src 'self' data:",
  "connect-src 'self' asrpro-media:",
  "worker-src 'self' blob:",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
];

export const CSP_POLICY = CSP_DIRECTIVES.join("; ");

// Build only: the dev server needs inline scripts and websockets for hot reload.
// The tag goes right after <meta charset> so it applies before any other element loads.
export function cspMetaPlugin(): Plugin {
  return {
    name: "asrpro-csp-meta",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler(html) {
        const charset = html.match(/<meta\s+charset=[^>]*>/i);
        if (!charset) throw new Error("index.html needs a <meta charset> tag for the CSP tag to follow.");
        const tag = `<meta http-equiv="Content-Security-Policy" content="${CSP_POLICY}" />`;
        return html.replace(charset[0], `${charset[0]}\n    ${tag}`);
      },
    },
  };
}

export default defineConfig(async () => ({
  base: "./",
  plugins: [react(), cspMetaPlugin()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 4270,
    strictPort: true,
    watch: {
      ignored: ["**/release/**", "**/tmp/**"],
    },
  },
}));
