import { createRequire } from "node:module";
import { describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const { EXTERNAL_TARGETS, FOLDER_TARGETS, createOpenTargets, isAllowedExternalUrl } = require("../../electron/shell/openTargets.cjs") as {
  EXTERNAL_TARGETS: Record<string, string>;
  FOLDER_TARGETS: string[];
  createOpenTargets: (options: {
    shell: { openExternal: (url: string) => Promise<void>; openPath: (path: string) => Promise<string> };
    ctx: { dataDir: string; layout: { logsDir: string } };
    mkdir?: (path: string, options: { recursive: true }) => void;
  }) => { open: (target: string) => Promise<void> };
  isAllowedExternalUrl: (url: unknown) => boolean;
};
const channels = require("../../shared/ipc-channels.json") as { channels: Record<string, unknown> };

function setup(openPathReply = "") {
  const shell = {
    openExternal: vi.fn(async (_url: string) => {}),
    openPath: vi.fn(async (_path: string) => openPathReply),
  };
  const ctx = { dataDir: "/data", layout: { logsDir: "/data/logs" } };
  const mkdir = vi.fn();
  return { shell, mkdir, ...createOpenTargets({ shell, ctx, mkdir }) };
}

describe("shell:open targets", () => {
  it("is a declared channel", () => {
    expect(channels.channels["shell:open"]).toEqual({ kind: "invoke", roles: ["main-window"] });
  });

  it("opens the named GitHub pages in the browser", async () => {
    const { open, shell } = setup();

    await open("repo");
    await open("issues");
    await open("releases");

    expect(shell.openExternal.mock.calls.map(([url]) => url)).toEqual([
      "https://github.com/surajmandalcell/asrpro",
      "https://github.com/surajmandalcell/asrpro/issues/new",
      "https://github.com/surajmandalcell/asrpro/releases",
    ]);
    expect(shell.openPath).not.toHaveBeenCalled();
  });

  it("opens the data and log folders in the file manager", async () => {
    const { open, shell } = setup();

    await open("data-folder");
    await open("log-folder");

    expect(shell.openPath.mock.calls.map(([target]) => target)).toEqual(["/data", "/data/logs"]);
    expect(shell.openExternal).not.toHaveBeenCalled();
  });

  it("creates a folder before opening it, so a fresh install has a log folder to show", async () => {
    const { open, shell, mkdir } = setup();
    shell.openPath.mockImplementation(async (target: string) => {
      expect(mkdir).toHaveBeenCalledWith(target, { recursive: true });
      return "";
    });

    await open("log-folder");

    expect(shell.openPath).toHaveBeenCalledTimes(1);
  });

  it("rejects an unknown target without opening anything", async () => {
    const { open, shell } = setup();

    await expect(open("file:///etc/passwd")).rejects.toMatchObject({ code: "INVALID_ARGUMENT" });
    await expect(open("https://example.com")).rejects.toMatchObject({ code: "INVALID_ARGUMENT" });
    await expect(open("__proto__")).rejects.toMatchObject({ code: "INVALID_ARGUMENT" });
    expect(shell.openExternal).not.toHaveBeenCalled();
    expect(shell.openPath).not.toHaveBeenCalled();
  });

  it("reports a folder the system could not open", async () => {
    const { open } = setup("Failed to open path");

    await expect(open("data-folder")).rejects.toMatchObject({ code: "INTERNAL" });
  });

  it("only allows the project pages on github.com over https", () => {
    for (const url of Object.values(EXTERNAL_TARGETS)) expect(isAllowedExternalUrl(url)).toBe(true);

    expect(isAllowedExternalUrl("https://github.com/surajmandalcell/asrpro/issues/1")).toBe(true);
    expect(isAllowedExternalUrl("http://github.com/surajmandalcell/asrpro")).toBe(false);
    expect(isAllowedExternalUrl("https://github.com/other/asrpro")).toBe(false);
    expect(isAllowedExternalUrl("https://github.com/surajmandalcell/asrpro-evil")).toBe(false);
    expect(isAllowedExternalUrl("https://github.com.evil.test/surajmandalcell/asrpro")).toBe(false);
    expect(isAllowedExternalUrl("https://user@github.com/surajmandalcell/asrpro")).toBe(false);
    expect(isAllowedExternalUrl("file:///etc/passwd")).toBe(false);
    expect(isAllowedExternalUrl("javascript:alert(1)")).toBe(false);
    expect(isAllowedExternalUrl("https://example.com")).toBe(false);
    expect(isAllowedExternalUrl(undefined)).toBe(false);
  });

  it("lists the folder targets it can open", () => {
    expect(FOLDER_TARGETS).toEqual(["data-folder", "log-folder"]);
  });
});
