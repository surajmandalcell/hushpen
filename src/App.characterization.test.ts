import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppError } from "./lib/bridge";
import { getErrorMessage } from "./lib/errors";
import { countWords, formatByteCount, formatDuration, formatHistoryGroupLabel, formatHomeRelativePath } from "./lib/format";
import { buildHistoryTitle } from "./lib/history";
import { formatShortcutParts } from "./lib/shortcut";
import {
  loadTranscriptHistory,
  normalizeTranscriptHistoryRow,
  saveTranscriptHistory,
} from "./features/history/historyRepository";
import { convertBlobToWav, dataUrlToBlob, encodePcm16Wav, mixAudioBufferToMono, resamplePcm } from "./lib/wav";
import {
  buildReactiveWaveformFrame,
  idleWaveformFrame,
  sendOverlayWaveformFrame,
  toOverlayWaveformSamples,
  waveformBaseBars,
} from "./lib/waveform";
import type { TranscriptHistoryRow } from "./types/history";

describe("encodePcm16Wav", () => {
  it("writes a 44-byte RIFF header for 16 kHz mono PCM16", () => {
    const wav = encodePcm16Wav(new Float32Array([0, 0.5]), 16000);
    const view = new DataView(wav);

    expect(wav.byteLength).toBe(48);
    expect(String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3))).toBe("RIFF");
    expect(view.getUint32(4, true)).toBe(40);
    expect(String.fromCharCode(view.getUint8(8), view.getUint8(9), view.getUint8(10), view.getUint8(11))).toBe("WAVE");
    expect(view.getUint32(16, true)).toBe(16);
    expect(view.getUint16(20, true)).toBe(1);
    expect(view.getUint16(22, true)).toBe(1);
    expect(view.getUint32(24, true)).toBe(16000);
    expect(view.getUint32(28, true)).toBe(32000);
    expect(view.getUint16(32, true)).toBe(2);
    expect(view.getUint16(34, true)).toBe(16);
    expect(view.getUint32(40, true)).toBe(4);
  });

  it("encodes samples as little-endian PCM16 with full-scale clamping", () => {
    const wav = encodePcm16Wav(new Float32Array([0, 0.5, -0.5, 1, -1, 2, -2]), 16000);
    const view = new DataView(wav);

    expect(view.getInt16(44, true)).toBe(0);
    expect(view.getInt16(46, true)).toBe(16384);
    expect(view.getInt16(48, true)).toBe(-16384);
    expect(view.getInt16(50, true)).toBe(32767);
    expect(view.getInt16(52, true)).toBe(-32768);
    expect(view.getInt16(54, true)).toBe(32767);
    expect(view.getInt16(56, true)).toBe(-32768);
  });
});

describe("mixAudioBufferToMono", () => {
  it("averages every channel into one Float32Array", () => {
    const fakeBuffer = {
      length: 3,
      numberOfChannels: 2,
      getChannelData: (channel: number) => (channel === 0
        ? new Float32Array([1, 0, -1])
        : new Float32Array([0, 1, 1])),
    } as unknown as AudioBuffer;

    expect(Array.from(mixAudioBufferToMono(fakeBuffer))).toEqual([0.5, 0.5, 0]);
  });
});

describe("resamplePcm", () => {
  it("returns the same samples when the rates match", () => {
    const samples = new Float32Array([1, 2, 3]);

    expect(resamplePcm(samples, 16000, 16000)).toBe(samples);
  });

  it("downsamples by linear interpolation", () => {
    const samples = new Float32Array([0, 1, 2, 3, 4, 5, 6, 7, 8]);

    expect(Array.from(resamplePcm(samples, 48000, 16000))).toEqual([0, 4, 8]);
  });
});

describe("convertBlobToWav", () => {
  it("passes a WAV blob through untouched", async () => {
    const blob = new Blob([new Uint8Array([1, 2, 3])], { type: "audio/wav" });

    expect(await convertBlobToWav(blob)).toBe(blob);
  });
});

describe("dataUrlToBlob", () => {
  it("decodes a base64 data URL with its mime type", () => {
    const blob = dataUrlToBlob(`data:audio/webm;base64,${window.btoa("abc")}`);

    expect(blob.type).toBe("audio/webm");
    expect(blob.size).toBe(3);
  });

  it("decodes a URL-encoded data URL", () => {
    const blob = dataUrlToBlob("data:text/plain,hello%20world");

    expect(blob.type).toBe("text/plain");
    expect(blob.size).toBe(11);
  });

  it("rejects strings that are not data URLs", () => {
    expect(() => dataUrlToBlob("https://example.com/audio.webm")).toThrow("Saved source audio could not be loaded.");
    expect(() => dataUrlToBlob("data:audio/webm")).toThrow("Saved source audio could not be loaded.");
  });
});

describe("formatDuration", () => {
  it.each([
    [0, "0:00"],
    [7, "0:07"],
    [59.4, "0:59"],
    [59.6, "1:00"],
    [61, "1:01"],
    [600, "10:00"],
    [-3, "0:00"],
  ])("formats %i seconds as %s", (seconds, expected) => {
    expect(formatDuration(seconds)).toBe(expected);
  });
});

describe("formatByteCount", () => {
  it.each([
    [undefined, "0 B"],
    [0, "0 B"],
    [-1, "0 B"],
    [Number.NaN, "0 B"],
    [512, "512 B"],
    [1024, "1.0 KiB"],
    [1536, "1.5 KiB"],
    [10240, "10 KiB"],
    [5 * 1024 * 1024, "5.0 MiB"],
    [1024 ** 4, "1.0 TiB"],
    [3 * 1024 ** 4, "3.0 TiB"],
  ])("formats %s bytes as %s", (bytes, expected) => {
    expect(formatByteCount(bytes)).toBe(expected);
  });
});

describe("formatHistoryGroupLabel", () => {
  const now = new Date(2026, 9, 4, 15, 0, 0).getTime();

  it("groups rows by calendar day relative to now", () => {
    expect(formatHistoryGroupLabel(new Date(2026, 9, 4, 8, 0, 0).getTime(), now)).toBe("Today");
    expect(formatHistoryGroupLabel(new Date(2026, 9, 3, 23, 0, 0).getTime(), now)).toBe("Yesterday");
    expect(formatHistoryGroupLabel(new Date(2026, 8, 29, 12, 0, 0).getTime(), now)).toBe("5 days ago");
    expect(formatHistoryGroupLabel(new Date(2026, 8, 5, 12, 0, 0).getTime(), now)).toBe("29 days ago");
    expect(formatHistoryGroupLabel(new Date(2026, 8, 4, 12, 0, 0).getTime(), now)).toBe("Sep 4");
  });

  it("treats future rows as today", () => {
    expect(formatHistoryGroupLabel(now + 86_400_000, now)).toBe("Today");
  });
});

describe("formatHomeRelativePath", () => {
  it("collapses the user home directory to ~", () => {
    expect(formatHomeRelativePath(undefined)).toBeUndefined();
    expect(formatHomeRelativePath("")).toBeUndefined();
    expect(formatHomeRelativePath("/Users/suraj/Library/Data")).toBe("~/Library/Data");
    expect(formatHomeRelativePath("/home/suraj")).toBe("~");
    expect(formatHomeRelativePath("/var/data")).toBe("/var/data");
    expect(formatHomeRelativePath("/Users")).toBe("/Users");
  });
});

describe("formatShortcutParts", () => {
  it("maps accelerator tokens to macOS glyphs", () => {
    expect(formatShortcutParts()).toEqual(["⌘", "`"]);
    expect(formatShortcutParts("")).toEqual(["⌘", "`"]);
    expect(formatShortcutParts("CommandOrControl+`")).toEqual(["⌘", "`"]);
    expect(formatShortcutParts("Command+Backquote")).toEqual(["⌘", "`"]);
    expect(formatShortcutParts("Meta+K")).toEqual(["⌘", "K"]);
    expect(formatShortcutParts("Control+Alt+Delete")).toEqual(["⌃", "⌥", "Delete"]);
    expect(formatShortcutParts("Shift+Escape")).toEqual(["⇧", "esc"]);
  });

  it("drops empty parts", () => {
    expect(formatShortcutParts("CommandOrControl++K")).toEqual(["⌘", "K"]);
  });
});

describe("countWords", () => {
  it("counts whitespace-separated words", () => {
    expect(countWords("hello world")).toBe(2);
    expect(countWords("  a  b   c ")).toBe(3);
    expect(countWords("")).toBe(0);
    expect(countWords("one")).toBe(1);
  });
});

describe("buildHistoryTitle", () => {
  it("falls back to a placeholder for empty text", () => {
    expect(buildHistoryTitle("")).toBe("Untitled dictation");
    expect(buildHistoryTitle("   \n  ")).toBe("Untitled dictation");
  });

  it("collapses whitespace", () => {
    expect(buildHistoryTitle("  hello   world\nagain ")).toBe("hello world again");
  });

  it("truncates long text to 89 characters plus an ellipsis", () => {
    expect(buildHistoryTitle("a".repeat(92))).toBe("a".repeat(92));
    expect(buildHistoryTitle("a".repeat(93))).toBe(`${"a".repeat(89)}...`);
  });
});

describe("getErrorMessage", () => {
  const downloadMessage = "Whisper model download failed. Check your connection and try again.";

  it("maps model download failure codes to the download message", () => {
    expect(getErrorMessage(new AppError("MODEL_DOWNLOAD_FAILED"))).toBe(downloadMessage);
    expect(getErrorMessage(new AppError("MODEL_DOWNLOAD_STALLED"))).toBe(downloadMessage);
    expect(getErrorMessage(new AppError("MODEL_CHECKSUM_MISMATCH"))).toBe(downloadMessage);
  });

  it("maps a not-ready engine to the restart message", () => {
    expect(getErrorMessage(new AppError("ENGINE_NOT_READY"))).toBe("Native Whisper engine needs restart. Restart ASR Pro, then try again.");
  });

  it("maps native addon load failures to the reinstall message", () => {
    expect(getErrorMessage(new AppError("ENGINE_LOAD_FAILED"))).toBe("Native Whisper engine could not load. Reinstall dependencies, then restart ASR Pro.");
  });

  it("maps an offline code to a generic load message", () => {
    expect(getErrorMessage(new AppError("OFFLINE"))).toBe("Failed to load.");
  });

  it("ignores English detail text when a code is present", () => {
    expect(getErrorMessage(new AppError("MODEL_DOWNLOAD_FAILED", undefined, "Cannot find module whisper.node"))).toBe(downloadMessage);
    expect(getErrorMessage(new AppError("INTERNAL", undefined, "failed to fetch"))).toBe("Something went wrong. Try again.");
  });

  it("does not map plain Error text to a code", () => {
    expect(getErrorMessage(new Error("Model download failed: 404"))).toBe("Model download failed: 404");
    expect(getErrorMessage(new Error("boom"))).toBe("boom");
  });

  it("falls back to the recording failure message for non-Error values", () => {
    expect(getErrorMessage("string error")).toBe("Recording failed");
    expect(getErrorMessage(undefined)).toBe("Recording failed");
  });
});

describe("normalizeTranscriptHistoryRow", () => {
  it("rejects non-object values", () => {
    expect(normalizeTranscriptHistoryRow(null)).toBeNull();
    expect(normalizeTranscriptHistoryRow("text")).toBeNull();
    expect(normalizeTranscriptHistoryRow(42)).toBeNull();
  });

  it("fills defaults for an empty row", () => {
    const row = normalizeTranscriptHistoryRow({});

    expect(row).not.toBeNull();
    expect(row?.id).toMatch(/^history-\d+$/);
    expect(row?.title).toBe("Untitled dictation");
    expect(row?.text).toBe("");
    expect(row?.kind).toBe("Dictation");
    expect(row?.model).toBe("Whisper Base English");
    expect(row?.durationSeconds).toBe(0);
    expect(row?.status).toBe("completed");
    expect(row?.recordingUrl).toBeUndefined();
    expect(row?.transcriptFilePath).toBeUndefined();
    expect(row?.error).toBeUndefined();
    expect(Number.isFinite(row?.createdAt)).toBe(true);
  });

  it("keeps valid fields and repairs invalid ones", () => {
    const row = normalizeTranscriptHistoryRow({
      id: "row-1",
      title: "  ",
      text: "hello world",
      kind: "File",
      status: "failed",
      model: "Whisper Base Multilingual",
      durationSeconds: 3.7,
      createdAt: 1000,
      recordingUrl: "data:audio/webm;base64,AAAA",
      transcriptFilePath: "/data/transcripts/row-1.txt",
      error: "boom",
    });

    expect(row?.id).toBe("row-1");
    expect(row?.title).toBe("hello world");
    expect(row?.kind).toBe("File");
    expect(row?.status).toBe("failed");
    expect(row?.durationSeconds).toBe(4);
    expect(row?.createdAt).toBe(1000);
    expect(row?.recordingUrl).toBe("data:audio/webm;base64,AAAA");
    expect(row?.transcriptFilePath).toBe("/data/transcripts/row-1.txt");
    expect(row?.error).toBe("boom");
  });

  it("repairs unknown enum values and non-numeric durations", () => {
    const row = normalizeTranscriptHistoryRow({ kind: "file", status: "cancelled", durationSeconds: "12" });

    expect(row?.kind).toBe("Dictation");
    expect(row?.status).toBe("completed");
    expect(row?.durationSeconds).toBe(0);
    expect(normalizeTranscriptHistoryRow({ durationSeconds: -5 })?.durationSeconds).toBe(0);
  });
});

describe("transcript history storage", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  const buildRow = (id: string): TranscriptHistoryRow => ({
    id,
    title: `Row ${id}`,
    text: `text ${id}`,
    kind: "Dictation",
    model: "Whisper Base English",
    durationSeconds: 1,
    createdAt: 1000,
    status: "completed",
  });

  it("caps saved history at 100 rows", () => {
    const rows = Array.from({ length: 105 }, (_, index) => buildRow(`r${index}`));

    saveTranscriptHistory(rows);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]") as TranscriptHistoryRow[];
    expect(stored).toHaveLength(100);
    expect(stored[0]?.id).toBe("r0");
    expect(stored[99]?.id).toBe("r99");
  });

  it("returns an empty list for missing, corrupt, or non-array storage", () => {
    expect(loadTranscriptHistory()).toEqual([]);

    window.localStorage.setItem("asrpro.transcriptHistory.v1", "not json{");
    expect(loadTranscriptHistory()).toEqual([]);

    window.localStorage.setItem("asrpro.transcriptHistory.v1", "{}");
    expect(loadTranscriptHistory()).toEqual([]);
  });

  it("drops invalid entries and keeps valid rows", () => {
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([null, 42, { id: "ok", text: "kept" }]));

    const rows = loadTranscriptHistory();

    expect(rows).toHaveLength(1);
    expect(rows[0]?.id).toBe("ok");
  });

  it("removes seeded screenshot rows outside screenshot mode", () => {
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      { id: "readme-history-1", title: "Product demo follow-up", text: "Summarize the product demo, send the follow-up notes, and schedule the model comparison review." },
      { id: "custom", title: "Roadmap voice note", text: "Keep the desktop release private first, tighten screenshot checks, and verify the packaged runtime before sharing." },
      { id: "real", title: "Real row", text: "mine", recordingUrl: "data:audio/webm;base64,AAAA" },
    ]));

    const rows = loadTranscriptHistory();

    expect(rows.map((row) => row.id)).toEqual(["real"]);
    expect(JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]")).toHaveLength(1);
  });
});

describe("waveform feed", () => {
  it("builds 76 base bars with bounded heights", () => {
    expect(waveformBaseBars).toHaveLength(76);
    for (const bar of waveformBaseBars) {
      expect(bar.baseHeight).toBeGreaterThanOrEqual(8);
      expect(bar.baseHeight).toBeLessThanOrEqual(46);
    }
    expect(idleWaveformFrame).toEqual(waveformBaseBars.map((bar) => bar.baseHeight));
  });

  it("keeps the idle frame for silence", () => {
    const frame = buildReactiveWaveformFrame(new Uint8Array(64), 0, 0, idleWaveformFrame);

    expect(frame).toEqual(idleWaveformFrame);
  });

  it("lifts bars with voice and caps them at 64", () => {
    const frequencies = new Uint8Array(64).fill(255);
    const frame = buildReactiveWaveformFrame(frequencies, 1, 1000, idleWaveformFrame);

    expect(frame).toHaveLength(76);
    for (const [index, value] of frame.entries()) {
      expect(value).toBeGreaterThanOrEqual(idleWaveformFrame[index] ?? 0);
      expect(value).toBeLessThanOrEqual(64);
    }
    expect(frame.some((value, index) => value > (idleWaveformFrame[index] ?? 0))).toBe(true);
  });

  it("smooths towards the target by half per frame", () => {
    const loudPrevious = Array.from({ length: 76 }, () => 64);
    const frame = buildReactiveWaveformFrame(new Uint8Array(64), 0, 0, loudPrevious);

    for (const [index, value] of frame.entries()) {
      const base = idleWaveformFrame[index] ?? 0;
      expect(value).toBe(Math.round((64 * 0.5 + base * 0.5) * 10) / 10);
    }
  });

  it("maps a frame to 55 overlay samples between 0 and 1", () => {
    const idleSamples = toOverlayWaveformSamples(idleWaveformFrame);

    expect(idleSamples).toHaveLength(55);
    expect(idleSamples.every((sample) => sample === 0)).toBe(true);

    const loudFrame = Array.from({ length: 76 }, () => 64);
    const loudSamples = toOverlayWaveformSamples(loudFrame);

    expect(loudSamples).toHaveLength(55);
    for (const sample of loudSamples) {
      expect(sample).toBeGreaterThanOrEqual(0);
      expect(sample).toBeLessThanOrEqual(1);
    }
    expect(loudSamples.some((sample) => sample > 0)).toBe(true);
  });

  it("sends samples to the overlay only while voice is present", () => {
    const send = vi.fn();
    window.asrpro = { send } as unknown as Window["asrpro"];

    sendOverlayWaveformFrame(idleWaveformFrame, false);
    expect(send).toHaveBeenLastCalledWith("recording:waveform-frame", []);

    sendOverlayWaveformFrame(idleWaveformFrame, true);
    expect(send).toHaveBeenLastCalledWith("recording:waveform-frame", idleSamplesOfFrame());

    window.asrpro = undefined;
  });
});

function idleSamplesOfFrame() {
  return toOverlayWaveformSamples(idleWaveformFrame);
}
