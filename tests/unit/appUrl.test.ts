import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { isAppUrl, resolveAppUrl } = require("../../electron/windows/appUrl.cjs") as {
  isAppUrl: (candidate: unknown, appUrl: string) => boolean;
  resolveAppUrl: (options: { isPackaged: boolean; devServerUrl: string; distIndexPath: string }) => string;
};

describe("app URL", () => {
  it("is the built index file when packaged and the dev server URL otherwise", () => {
    const options = { devServerUrl: "http://127.0.0.1:4270", distIndexPath: "/opt/asr/resources/app.asar/dist/index.html" };

    expect(resolveAppUrl({ ...options, isPackaged: true })).toBe("file:///opt/asr/resources/app.asar/dist/index.html");
    expect(resolveAppUrl({ ...options, isPackaged: false })).toBe("http://127.0.0.1:4270");
  });

  describe("for a file page", () => {
    const appUrl = "file:///app/dist/index.html";

    it("accepts the page itself, with or without a hash or query", () => {
      expect(isAppUrl("file:///app/dist/index.html", appUrl)).toBe(true);
      expect(isAppUrl("file:///app/dist/index.html#/about", appUrl)).toBe(true);
      expect(isAppUrl("file:///app/dist/index.html?x=1", appUrl)).toBe(true);
    });

    it("accepts the bare file origin that permission checks report", () => {
      expect(isAppUrl("file:///", appUrl)).toBe(true);
    });

    it("rejects other files, other schemes, and malformed values", () => {
      expect(isAppUrl("file:///assets/fixtures/speech-short.wav", appUrl)).toBe(false);
      expect(isAppUrl("file:///app/dist/other.html", appUrl)).toBe(false);
      expect(isAppUrl("https://example.com/app/dist/index.html", appUrl)).toBe(false);
      expect(isAppUrl("javascript:alert(1)", appUrl)).toBe(false);
      expect(isAppUrl("data:text/html,hi", appUrl)).toBe(false);
      expect(isAppUrl("not a url", appUrl)).toBe(false);
      expect(isAppUrl(undefined, appUrl)).toBe(false);
      expect(isAppUrl("", appUrl)).toBe(false);
    });
  });

  describe("for a dev server", () => {
    const appUrl = "http://127.0.0.1:4270";

    it("accepts any path on the same origin", () => {
      expect(isAppUrl("http://127.0.0.1:4270/", appUrl)).toBe(true);
      expect(isAppUrl("http://127.0.0.1:4270/src/main.tsx", appUrl)).toBe(true);
    });

    it("rejects other origins, ports, and schemes", () => {
      expect(isAppUrl("http://127.0.0.1:4271/", appUrl)).toBe(false);
      expect(isAppUrl("http://localhost:4270/", appUrl)).toBe(false);
      expect(isAppUrl("https://127.0.0.1:4270/", appUrl)).toBe(false);
      expect(isAppUrl("file:///app/dist/index.html", appUrl)).toBe(false);
    });
  });
});
