import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { audioRecordingService } from "./services/audioRecording";

afterEach(async () => {
  if (audioRecordingService.isRecording()) {
    await audioRecordingService.stopRecording().catch(() => null);
  }
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  window.asrpro = undefined;
  window.localStorage.clear();
  cleanup();
});

function mockAudioCapture() {
  const stopTrack = vi.fn();
  const stream = {
    getTracks: () => [{ stop: stopTrack }],
  } as unknown as MediaStream;

  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      getUserMedia: vi.fn().mockResolvedValue(stream),
    },
  });

  vi.stubGlobal("AudioContext", class {
    createMediaStreamSource() {
      return { connect: vi.fn() };
    }

    createAnalyser() {
      return {
        fftSize: 256,
        frequencyBinCount: 8,
        getByteFrequencyData: (samples: Uint8Array) => samples.fill(72),
      };
    }

    close = vi.fn();
  });

  vi.stubGlobal("MediaRecorder", class {
    static isTypeSupported() {
      return true;
    }

    ondataavailable?: (event: { data: Blob }) => void;
    onstop?: () => void;
    state = "inactive";
    mimeType: string;

    constructor(_stream: MediaStream, options?: MediaRecorderOptions) {
      this.mimeType = options?.mimeType || "audio/webm";
    }

    start() {
      this.state = "recording";
    }

    stop() {
      this.state = "inactive";
      this.ondataavailable?.({ data: new Blob(["recorded audio"], { type: this.mimeType }) });
      this.onstop?.();
    }
  });

  return { stopTrack };
}

describe("ASR Pro Electron shell", () => {
  it("renders the ASR Pro dashboard as the primary desktop surface", () => {
    render(<App />);

    expect(screen.getAllByText("ASR Pro").length).toBeGreaterThan(0);
    expect(screen.getByText("Dictation")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Ready to Dictate" })).toBeNull();
    expect(screen.getByRole("button", { name: "Start Recording" })).toBeTruthy();
    expect(screen.getAllByText(/Parakeet-TDT-0\.6B-v3/).length).toBeGreaterThan(0);
  });

  it("keeps the shell free of titlebar slogans, shortcut badges, and redundant tabs", () => {
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      selectAudioFiles: vi.fn(),
      onAddFiles: vi.fn(),
      getRuntimeState: vi.fn(),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      windowControl: vi.fn(),
    };

    render(<App />);

    const sidebarTitle = screen.getAllByText("ASR Pro")[0].closest("div");
    expect(sidebarTitle?.textContent).toBe("ASR Pro");
    expect(screen.queryByText("Local first")).toBeNull();
    expect(screen.queryByText("Global overlay active")).toBeNull();
    expect(screen.queryByText(/CommandOrControl/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Dictate" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Shortcuts" })).toBeNull();
  });

  it("navigates to the file transcription workspace", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Files" }));

    expect(screen.getByRole("heading", { name: "Drop audio or video" })).toBeTruthy();
    expect(screen.getByText("Queued transcription jobs")).toBeTruthy();
  });

  it("shows a visible selected state on the active sidebar item", async () => {
    const user = userEvent.setup();
    render(<App />);

    const dashboardButton = screen.getByRole("button", { name: "Dashboard" });
    expect(dashboardButton.getAttribute("aria-current")).toBe("page");
    expect(dashboardButton.className).toContain("bg-[#d9d9e1]");

    const filesButton = screen.getByRole("button", { name: "Files" });
    await user.click(filesButton);

    expect(filesButton.getAttribute("aria-current")).toBe("page");
    expect(filesButton.className).toContain("bg-[#d9d9e1]");
    expect(dashboardButton.getAttribute("aria-current")).toBeNull();
  });

  it("renders a non-uniform full-width waveform baseline", () => {
    render(<App />);

    const bars = Array.from(document.querySelectorAll<HTMLElement>(".in-app-waveform__bars span"));
    const heights = bars.map((bar) => bar.style.getPropertyValue("--wave-height"));

    expect(bars).toHaveLength(76);
    expect(new Set(heights).size).toBeGreaterThan(24);
    expect(heights.slice(0, 12)).not.toEqual(heights.slice(12, 24));
  });

  it("toggles recording state from the dashboard", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy();
    expect(screen.queryByText("Recording now")).toBeNull();
    expect(screen.getByLabelText("Recording active")).toBeTruthy();
  });

  it("shows real local history instead of static demo transcripts", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByText("No transcription history yet")).toBeTruthy();
    expect(screen.queryByText("Design review notes")).toBeNull();
    expect(screen.queryByText("Product demo call")).toBeNull();
  });

  it("records audio, transcribes it, and stores the result in history", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => ({
        text: "Buy milk and schedule the product demo.",
        duration: 1.7,
      }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));
    await screen.findByRole("button", { name: "Stop Recording" });
    await user.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "http://localhost:8000/v1/audio/transcriptions",
        expect.objectContaining({
          method: "POST",
          body: expect.any(FormData),
        }),
      );
    });

    await waitFor(() => expect(screen.getByRole("heading", { name: "Transcript library" })).toBeTruthy());
    expect(screen.getByText("Buy milk and schedule the product demo.")).toBeTruthy();
    const audio = screen.getByLabelText("Play recording: Buy milk and schedule the product demo.");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio.getAttribute("src")).toMatch(/^data:audio\/webm/);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].text).toBe("Buy milk and schedule the product demo.");
    expect(stored[0].kind).toBe("Dictation");
    expect(stored[0].recordingUrl).toMatch(/^data:audio\/webm/);
  });

  it("syncs recording state from the global shortcut and tray bridge", async () => {
    mockAudioCapture();
    let recordingListener: ((state: { isRecording: boolean; source: string }) => void) | undefined;
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      selectAudioFiles: vi.fn(),
      onAddFiles: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Parakeet-TDT-0.6B-v3",
        shortcut: "CommandOrControl+`",
      }),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn((callback) => {
        recordingListener = callback;
        return vi.fn();
      }),
      windowControl: vi.fn(),
    };

    render(<App />);
    await waitFor(() => expect(recordingListener).toBeTruthy());

    await act(async () => {
      recordingListener?.({ isRecording: true, source: "shortcut" });
    });

    expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy();
    expect(screen.queryByText("Global overlay active")).toBeNull();
    expect(screen.queryByText(/CommandOrControl/)).toBeNull();
  });

  it("updates the recording overlay placement from settings", async () => {
    const user = userEvent.setup();
    const setOverlaySettings = vi.fn().mockResolvedValue({ placement: "bottom", customBounds: null });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      selectAudioFiles: vi.fn(),
      onAddFiles: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Parakeet-TDT-0.6B-v3",
        shortcut: "CommandOrControl+`",
        overlaySettings: { placement: "top", customBounds: null },
      }),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      setOverlaySettings,
      windowControl: vi.fn(),
    };

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Top overlay position" }).getAttribute("aria-pressed")).toBe("true"));

    await user.click(screen.getByRole("button", { name: "Bottom overlay position" }));

    expect(setOverlaySettings).toHaveBeenCalledWith({ placement: "bottom" });
    expect(screen.getByRole("button", { name: "Bottom overlay position" }).getAttribute("aria-pressed")).toBe("true");
  });
});
