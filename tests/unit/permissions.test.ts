import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const { isAllowedPermission, isTrustedMediaPermission } = require("../../electron/windows/mediaPermissions.cjs") as {
  isAllowedPermission: (webContents: Contents, permission: string, details: Details, appUrl: string) => boolean;
  isTrustedMediaPermission: (webContents: Contents, permission: string, details: Details, appUrl: string) => boolean;
};

type Contents = { getURL?: () => string } | undefined;
type Details = { mediaType?: string; mediaTypes?: string[]; requestingUrl?: string; requestingOrigin?: string; securityOrigin?: string };

const filePage = "file:///app/dist/index.html";
const devServerUrl = "http://127.0.0.1:4270";

describe("media permission check", () => {
  it("allows audio media for the app page and the dev server", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: filePage, mediaType: "audio" }, filePage)).toBe(true);
    expect(isTrustedMediaPermission(undefined, "media", { requestingOrigin: devServerUrl }, devServerUrl)).toBe(true);
    expect(isTrustedMediaPermission({ getURL: () => filePage }, "media", {}, filePage)).toBe(true);
  });

  it("denies non-audio media types", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: filePage, mediaType: "video" }, filePage)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: filePage, mediaTypes: ["video", "audio"] }, filePage)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: filePage, mediaTypes: ["audio"] }, filePage)).toBe(true);
  });

  it("denies other pages, including other local files", () => {
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "https://example.com" }, filePage)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", { requestingUrl: "file:///tmp/evil.html" }, filePage)).toBe(false);
    expect(isTrustedMediaPermission(undefined, "media", {}, filePage)).toBe(false);
  });
});

describe("permission policy", () => {
  it("allows the clipboard write that auto-copy needs for the app page", () => {
    expect(isAllowedPermission(undefined, "clipboard-sanitized-write", { requestingUrl: filePage }, filePage)).toBe(true);
    expect(isAllowedPermission({ getURL: () => filePage }, "clipboard-sanitized-write", {}, filePage)).toBe(true);
    expect(isAllowedPermission(undefined, "clipboard-sanitized-write", { requestingOrigin: devServerUrl }, devServerUrl)).toBe(true);
  });

  it("denies the clipboard write for any other page", () => {
    expect(isAllowedPermission(undefined, "clipboard-sanitized-write", { requestingUrl: "https://example.com" }, filePage)).toBe(false);
    expect(isAllowedPermission(undefined, "clipboard-sanitized-write", {}, filePage)).toBe(false);
  });

  it("keeps clipboard read and every other permission denied", () => {
    for (const permission of ["clipboard-read", "geolocation", "notifications", "midi", "fullscreen", "openExternal", "display-capture"]) {
      expect(isAllowedPermission(undefined, permission, { requestingUrl: filePage }, filePage)).toBe(false);
    }
  });

  it("allows audio capture for the app page", () => {
    expect(isAllowedPermission(undefined, "media", { requestingUrl: filePage, mediaType: "audio" }, filePage)).toBe(true);
  });
});
