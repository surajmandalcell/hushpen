import { afterEach, describe, expect, it, vi } from "vitest";
import { AppError, bridge, isAppError } from "./bridge";
import { getErrorMessage, getRecordingErrorTitle } from "./errors";

afterEach(() => {
  window.asrpro = undefined;
});

function transport(reply: unknown) {
  const invoke = vi.fn().mockResolvedValue(reply);
  window.asrpro = { invoke, send: vi.fn(), on: vi.fn() };
  return invoke;
}

describe("bridge", () => {
  it("unwraps an ok envelope", async () => {
    const invoke = transport({ ok: true, value: { name: "ASR Pro", version: "2.0.0" } });

    await expect(bridge.getAppInfo()).resolves.toEqual({ name: "ASR Pro", version: "2.0.0" });
    expect(invoke).toHaveBeenCalledWith("app:info", undefined);
  });

  it("throws an AppError carrying the code and params from an error envelope", async () => {
    transport({ ok: false, error: { code: "MODEL_MISSING", params: { modelId: "whisper-base-en" }, detail: "ggml-base.en.bin" } });

    const error = await bridge.getEngineState().catch((caught: unknown) => caught);

    expect(isAppError(error)).toBe(true);
    expect(error).toBeInstanceOf(AppError);
    expect(error).toMatchObject({ code: "MODEL_MISSING", params: { modelId: "whisper-base-en" }, detail: "ggml-base.en.bin" });
  });

  it("maps a code that is not in shared/error-codes.json to INTERNAL", async () => {
    transport({ ok: false, error: { code: "TOTALLY_NEW" } });

    await expect(bridge.getEngineState()).rejects.toMatchObject({ code: "INTERNAL" });
  });

  it("turns a transport failure or a malformed reply into INTERNAL", async () => {
    window.asrpro = { invoke: vi.fn().mockRejectedValue(new Error("Error invoking remote method 'x': boom")), send: vi.fn(), on: vi.fn() };
    await expect(bridge.getAppInfo()).rejects.toMatchObject({ code: "INTERNAL" });

    transport("not an envelope");
    await expect(bridge.getAppInfo()).rejects.toMatchObject({ code: "INTERNAL" });
  });

  it("throws INTERNAL when the desktop bridge is missing", async () => {
    await expect(bridge.getAppInfo()).rejects.toMatchObject({ code: "INTERNAL" });
    expect(bridge.isAvailable()).toBe(false);
  });

  it("sends settings and the legacy import through the declared channels", async () => {
    const invoke = transport({ ok: true, value: { values: {} } });

    await bridge.setSetting("output.autoCopy", false);
    await bridge.importLegacySettings({ selectedModelName: "Whisper Base English", audioInputId: "usb-mic" });

    expect(invoke).toHaveBeenNthCalledWith(1, "settings:set", { key: "output.autoCopy", value: false });
    expect(invoke).toHaveBeenNthCalledWith(2, "settings:import-legacy", { selectedModelName: "Whisper Base English", audioInputId: "usb-mic" });
  });
});

describe("error messages come from codes", () => {
  it("never reads the English detail", () => {
    const error = new AppError("ENGINE_LOAD_FAILED", undefined, "Cannot find module whisper.node");

    expect(getErrorMessage(error)).toBe("Native Whisper engine could not load. Reinstall dependencies, then restart ASR Pro.");
  });
});

describe("recording error titles", () => {
  it("keeps the 1.x titles per code", () => {
    expect(getRecordingErrorTitle("ENGINE_NOT_READY")).toBe("Engine needs restart");
    expect(getRecordingErrorTitle("MODEL_DOWNLOAD_FAILED")).toBe("Engine unavailable");
    expect(getRecordingErrorTitle("OFFLINE")).toBe("Engine unavailable");
    expect(getRecordingErrorTitle("ENGINE_LOAD_FAILED")).toBe("Recording failed");
    expect(getRecordingErrorTitle(undefined)).toBe("Recording failed");
  });
});
