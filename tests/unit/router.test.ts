import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const { createRouter } = require("../../electron/ipc/router.cjs");
const { registerSettingsIpc } = require("../../electron/ipc/settings.cjs");
const { createSettingsRegistry } = require("../../electron/settings/registry.cjs");
const { createLog } = require("../../electron/core/log.cjs");
const { v } = require("../../electron/ipc/validate.cjs");

const appUrl = "file:///app/dist/index.html";

type Handler = (event: unknown, payload?: unknown) => Promise<{ ok: boolean; value?: unknown; error?: { code: string; params?: unknown; detail?: string } }>;

let dir: string;
afterEach(() => rmSync(dir, { recursive: true, force: true }));

function setup() {
  dir = mkdtempSync(join(tmpdir(), "asrpro-router-"));
  const log = createLog({ dir });
  const handlers = new Map<string, Handler>();
  const listeners = new Map<string, (event: unknown, payload?: unknown) => void>();
  const ipc = {
    handle: (channel: string, handler: Handler) => handlers.set(channel, handler),
    on: (channel: string, handler: (event: unknown, payload?: unknown) => void) => listeners.set(channel, handler),
  };
  const mainFrame = { url: appUrl };
  const mainWebContents = { id: 7, mainFrame };
  const ctx = {
    log,
    windows: { main: { isDestroyed: () => false, webContents: mainWebContents }, overlay: undefined },
    pushToMain: vi.fn(),
    state: {},
  };
  const trusted = { sender: mainWebContents, senderFrame: mainFrame };
  const router = createRouter({ ctx, ipc, appUrl });
  return { ctx, router, handlers, listeners, trusted, log, call: (channel: string, event: unknown, payload?: unknown) => (handlers.get(channel) as Handler)(event, payload) };
}

describe("IPC router", () => {
  it("wraps a handler result in an ok envelope", async () => {
    const { router, trusted, call } = setup();
    router.handle("app:platform", v.none(), () => ({ platform: "darwin" }));

    expect(await call("app:platform", trusted)).toEqual({ ok: true, value: { platform: "darwin" } });
  });

  it("turns a thrown AppError into an error envelope with its code and params", async () => {
    const { router, trusted, call } = setup();
    const { AppError } = require("../../electron/core/errors.cjs");
    router.handle("engine:get-state", v.none(), () => {
      throw new AppError("MODEL_MISSING", { modelId: "whisper-base-en" });
    });

    expect(await call("engine:get-state", trusted)).toEqual({
      ok: false,
      error: { code: "MODEL_MISSING", params: { modelId: "whisper-base-en" } },
    });
  });

  it("maps an unexpected throw to INTERNAL", async () => {
    const { router, trusted, call } = setup();
    router.handle("engine:get-state", v.none(), () => {
      throw new Error("kaboom");
    });

    const reply = await call("engine:get-state", trusted);
    expect(reply.ok).toBe(false);
    expect(reply.error?.code).toBe("INTERNAL");
  });

  it("rejects a sender that is not the main window with FORBIDDEN_SENDER and never runs the handler", async () => {
    const { router, call } = setup();
    const handler = vi.fn();
    router.handle("app:platform", v.none(), handler);

    const foreign = { sender: { id: 99, mainFrame: {} }, senderFrame: {} };
    expect((await call("app:platform", foreign)).error?.code).toBe("FORBIDDEN_SENDER");
    expect((await call("app:platform", {})).error?.code).toBe("FORBIDDEN_SENDER");
    expect(handler).not.toHaveBeenCalled();
  });

  it("rejects the main window itself while it shows a page that is not the app", async () => {
    const { router, call, ctx } = setup();
    const handler = vi.fn();
    router.handle("app:platform", v.none(), handler);
    const mainWebContents = ctx.windows.main.webContents;

    for (const url of ["https://example.com/", "file:///assets/fixtures/speech-short.wav", "about:blank", ""]) {
      const frame = { url };
      mainWebContents.mainFrame = frame;
      expect((await call("app:platform", { sender: mainWebContents, senderFrame: frame })).error?.code).toBe("FORBIDDEN_SENDER");
    }
    expect(handler).not.toHaveBeenCalled();
  });

  it("rejects a frame inside the main window", async () => {
    const { router, call, ctx } = setup();
    const handler = vi.fn();
    router.handle("app:platform", v.none(), handler);

    const subframe = { url: appUrl };
    expect((await call("app:platform", { sender: ctx.windows.main.webContents, senderFrame: subframe })).error?.code).toBe("FORBIDDEN_SENDER");
    expect(handler).not.toHaveBeenCalled();
  });

  it("rejects a second window that loads the app page and the same preload", async () => {
    const { router, call } = setup();
    const handler = vi.fn();
    router.handle("settings:get-all", v.none(), handler);

    const frame = { url: appUrl };
    const hidden = { sender: { id: 8, mainFrame: frame }, senderFrame: frame };
    expect((await call("settings:get-all", hidden)).error?.code).toBe("FORBIDDEN_SENDER");
    expect(handler).not.toHaveBeenCalled();
  });

  it("applies the same sender check to fire-and-forget channels", () => {
    const { router, listeners, trusted } = setup();
    const handler = vi.fn();
    router.on("recording:waveform-frame", v.numberArray(), handler);
    const listener = listeners.get("recording:waveform-frame") as (event: unknown, payload?: unknown) => void;

    const frame = { url: "https://example.com/" };
    listener({ sender: { id: 7, mainFrame: frame }, senderFrame: frame }, [0.1]);
    listener({ sender: { id: 99, mainFrame: {} }, senderFrame: {} }, [0.1]);
    expect(handler).not.toHaveBeenCalled();

    listener(trusted, [0.1]);
    expect(handler).toHaveBeenCalledWith([0.1], trusted);
  });

  it("rejects an invalid payload with INVALID_ARGUMENT", async () => {
    const { router, trusted, call } = setup();
    const handler = vi.fn();
    router.handle("recording:set", v.object({ active: v.boolean() }), handler);

    expect((await call("recording:set", trusted, { active: "yes" })).error?.code).toBe("INVALID_ARGUMENT");
    expect((await call("recording:set", trusted, { active: true, extra: 1 })).error?.code).toBe("INVALID_ARGUMENT");
    expect((await call("recording:set", trusted)).error?.code).toBe("INVALID_ARGUMENT");
    expect(handler).not.toHaveBeenCalled();
  });

  it("refuses channels that are not declared in shared/ipc-channels.json", () => {
    const { router } = setup();
    expect(() => router.handle("made:up", v.none(), () => undefined)).toThrow(/ipc-channels/);
  });

  it("logs the code and channel of a failure but never the payload", async () => {
    const { router, trusted, call, log } = setup();
    router.handle("transcript:open-text", v.object({ title: v.string({ max: 5 }) }), () => undefined);

    await call("transcript:open-text", trusted, { title: "this is a secret transcript sentence" });

    const written = readFileSync(log.filePath, "utf8");
    expect(written).toContain("INVALID_ARGUMENT");
    expect(written).toContain("transcript:open-text");
    expect(written).not.toContain("secret transcript sentence");
  });
});

describe("settings:set and settings:import-legacy", () => {
  function settingsSetup() {
    const base = setup();
    const settings = createSettingsRegistry({ configDir: join(dir, "config"), log: base.log });
    const ctx = { ...base.ctx, settings };
    const router = createRouter({ ctx, appUrl, ipc: { handle: (channel: string, handler: Handler) => base.handlers.set(channel, handler), on: () => undefined } });
    const overlay = { position: vi.fn() };
    const startup = { apply: (value: boolean) => ({ "startup.launchAtLogin": value }), getState: () => ({ supported: true, enabled: false }) };
    registerSettingsIpc({ router, ctx, overlay, startup });
    return { ...base, settings };
  }

  it("rejects an invalid setting value with SETTINGS_INVALID and keeps the stored value", async () => {
    const { call, trusted, settings } = settingsSetup();

    const reply = await call("settings:set", trusted, { key: "output.autoCopy", value: "yes" });

    expect(reply.error?.code).toBe("SETTINGS_INVALID");
    expect(settings.get("output.autoCopy")).toBe(true);
  });

  it("imports the legacy model and microphone once and reports imported: false afterwards", async () => {
    const { call, trusted, settings } = settingsSetup();

    const first = await call("settings:import-legacy", trusted, { selectedModelName: "Whisper Small English", audioInputId: "usb-mic" });
    expect(first.value).toEqual(expect.objectContaining({ imported: true }));
    expect(settings.get("transcription.modelId")).toBe("whisper-small-en");
    expect(settings.get("recording.audioInputId")).toBe("usb-mic");
    expect(settings.get("migrations.legacyLocalStorage")).toBe(true);

    const second = await call("settings:import-legacy", trusted, { selectedModelName: "Whisper Base English", audioInputId: "other-mic" });
    expect(second.value).toEqual(expect.objectContaining({ imported: false }));
    expect(settings.get("transcription.modelId")).toBe("whisper-small-en");
    expect(settings.get("recording.audioInputId")).toBe("usb-mic");
  });

  it("closes the import on the first attempt even when an unknown legacy model name is rejected", async () => {
    const { call, trusted, settings } = settingsSetup();

    const reply = await call("settings:import-legacy", trusted, { selectedModelName: "Not A Model" });

    expect(reply.value).toEqual(expect.objectContaining({ imported: false }));
    expect(settings.get("transcription.modelId")).toBe("whisper-base-en");
    expect(settings.get("migrations.legacyLocalStorage")).toBe(true);
  });

  it("closes the import on the first attempt when the payload is empty", async () => {
    const { call, trusted, settings } = settingsSetup();

    const reply = await call("settings:import-legacy", trusted, {});

    expect(reply.value).toEqual(expect.objectContaining({ imported: false }));
    expect(settings.get("migrations.legacyLocalStorage")).toBe(true);
  });

  it("keeps a model and microphone chosen after a rejected first import", async () => {
    const { call, trusted, settings } = settingsSetup();

    await call("settings:import-legacy", trusted, { selectedModelName: "Not A Model" });
    await call("settings:set", trusted, { key: "transcription.modelId", value: "whisper-small-en" });
    await call("settings:set", trusted, { key: "recording.audioInputId", value: "desk-mic" });

    const again = await call("settings:import-legacy", trusted, { selectedModelName: "Whisper Base English", audioInputId: "old-mic" });

    expect(again.value).toEqual(expect.objectContaining({ imported: false }));
    expect(settings.get("transcription.modelId")).toBe("whisper-small-en");
    expect(settings.get("recording.audioInputId")).toBe("desk-mic");
  });
});
