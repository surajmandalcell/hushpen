import { createRequire } from "node:module";
import { describe, expect, it } from "vitest";

const require = createRequire(import.meta.url);
const identity = require("../electron/identity.cjs");

describe("Electron app identity", () => {
  it("uses ASR Pro branding for runtime name and about panel metadata", () => {
    expect(identity.APP_ID).toBe("com.surajmandal.asrpro");
    expect(identity.APP_NAME).toBe("ASR Pro");
    expect(identity.buildAboutPanelOptions("0.1.0")).toEqual({
      applicationName: "ASR Pro",
      applicationVersion: "0.1.0",
      version: "0.1.0",
      copyright: "(c) 2026 Suraj Mandal",
      authors: ["Suraj Mandal"],
    });
  });
});
