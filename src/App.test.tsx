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

function mockAudioCapture(devices: Partial<MediaDeviceInfo>[] = []) {
  const stopTrack = vi.fn();
  const stream = {
    getTracks: () => [{ stop: stopTrack }],
  } as unknown as MediaStream;
  const getUserMedia = vi.fn().mockResolvedValue(stream);
  const enumerateDevices = vi.fn().mockResolvedValue(devices);

  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      getUserMedia,
      enumerateDevices,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
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

  return { stopTrack, getUserMedia, enumerateDevices };
}

function renderedClassNames() {
  return Array.from(document.querySelectorAll("[class]"))
    .map((element) => element.getAttribute("class") || "")
    .join("\n");
}

describe("ASR Pro Electron shell", () => {
  it("renders a Superwhisper-style home surface without the bottom-left Pro pill", () => {
    render(<App />);

    expect(screen.getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByText("Average speed")).toBeTruthy();
    expect(screen.getByText("Words this week")).toBeTruthy();
    expect(screen.getByText("Recordings")).toBeTruthy();
    expect(screen.queryByText("Apps used")).toBeNull();
    expect(screen.getByText("Get started")).toBeTruthy();
    expect(screen.getByText("What's new?")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Ready to Dictate" })).toBeNull();
    expect(screen.getByRole("button", { name: "Start Recording" })).toBeTruthy();
    expect(screen.getAllByText("Local Whisper").length).toBeGreaterThan(0);
    expect(screen.queryByText(/Superwhisper/i)).toBeNull();
    expect(screen.queryByRole("button", { name: "Modes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Vocabulary" })).toBeNull();
    expect(screen.queryByText("Create a mode")).toBeNull();
    expect(screen.queryByText("Add vocabulary")).toBeNull();
  });

  it("keeps the shell free of titlebar slogans, shortcut badges, and redundant tabs", () => {
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn(),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      windowControl: vi.fn(),
    };

    render(<App />);

    expect(screen.getByRole("button", { name: "Home" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Modes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Vocabulary" })).toBeNull();
    expect(screen.getByRole("button", { name: "Configuration" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sound" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Models library" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "History" })).toBeTruthy();
    expect(screen.queryByText("Local first")).toBeNull();
    expect(screen.queryByText("Global overlay active")).toBeNull();
    expect(screen.queryByText(/CommandOrControl/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Dictate" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Shortcuts" })).toBeNull();
  });

  it("navigates to the transcript history", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByRole("heading", { name: "Transcript library" })).toBeTruthy();
    expect(screen.getByText("No transcription history yet")).toBeTruthy();
  });

  it("shows a visible selected state on the active sidebar item", async () => {
    const user = userEvent.setup();
    render(<App />);

    const homeButton = screen.getByRole("button", { name: "Home" });
    expect(homeButton.getAttribute("aria-current")).toBe("page");
    expect(homeButton.className).toContain("bg-[#686868]");

    const historyButton = screen.getByRole("button", { name: "History" });
    await user.click(historyButton);

    expect(historyButton.getAttribute("aria-current")).toBe("page");
    expect(historyButton.className).toContain("bg-[#686868]");
    expect(homeButton.getAttribute("aria-current")).toBeNull();
  });

  it("does not render a local waveform inside the home page", () => {
    render(<App />);

    expect(document.querySelector(".in-app-waveform")).toBeNull();
  });

  it("toggles recording state from the dashboard", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy());
    expect(screen.queryByText("Recording now")).toBeNull();
    expect(screen.getByLabelText("Recording active")).toBeTruthy();
  });

  it("uses the selected microphone device when recording starts", async () => {
    const user = userEvent.setup();
    const { getUserMedia } = mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
      { kind: "audioinput", deviceId: "usb-mic", label: "USB Microphone" },
    ]);

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Sound" }));

    const selector = await screen.findByRole("button", { name: "Microphone selector" });
    expect(document.querySelector('select[aria-label="Microphone"]')).toBeNull();
    await user.click(selector);
    await waitFor(() => expect(screen.getByRole("option", { name: "USB Microphone" })).toBeTruthy());
    await user.click(screen.getByRole("option", { name: "USB Microphone" }));

    await user.click(screen.getByRole("button", { name: "Home" }));
    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    await waitFor(() => expect(getUserMedia).toHaveBeenCalled());
    const constraints = getUserMedia.mock.calls[0][0] as MediaStreamConstraints;

    expect(constraints.audio).toMatchObject({
      deviceId: { exact: "usb-mic" },
      sampleRate: 16000,
      channelCount: 1,
    });
  });

  it("restores the saved microphone selection", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("asrpro.audioInputDevice.v1", "usb-mic");
    mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
      { kind: "audioinput", deviceId: "usb-mic", label: "USB Microphone" },
    ]);

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Sound" }));

    const selector = await screen.findByRole("button", { name: "Microphone selector" });

    await waitFor(() => expect(selector.textContent).toContain("USB Microphone"));
    expect(window.localStorage.getItem("asrpro.audioInputDevice.v1")).toBe("usb-mic");
  });

  it("resets a missing saved microphone to the system default", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("asrpro.audioInputDevice.v1", "missing-mic");
    const { getUserMedia } = mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
    ]);

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Sound" }));

    const selector = await screen.findByRole("button", { name: "Microphone selector" });

    await waitFor(() => expect(selector.textContent).toContain("System default"));
    expect(window.localStorage.getItem("asrpro.audioInputDevice.v1")).toBe("default");

    await user.click(screen.getByRole("button", { name: "Home" }));
    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    await waitFor(() => expect(getUserMedia).toHaveBeenCalled());
    const constraints = getUserMedia.mock.calls[0][0] as MediaStreamConstraints;

    expect(constraints.audio).not.toHaveProperty("deviceId");
  });

  it("opens the toolbar microphone selector and applies the selected device", async () => {
    const user = userEvent.setup();
    mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
      { kind: "audioinput", deviceId: "studio-mic", label: "Studio Microphone With A Long Name" },
    ]);

    render(<App />);

    const toolbarSelector = await screen.findByRole("button", { name: "Toolbar microphone selector" });
    await user.click(toolbarSelector);
    await user.click(screen.getByRole("option", { name: "Studio Microphone With A Long Name" }));

    await waitFor(() => expect(toolbarSelector.textContent).toContain("Studio Microphone With A Long Name"));
    expect(window.localStorage.getItem("asrpro.audioInputDevice.v1")).toBe("studio-mic");
  });

  it("shows real local history instead of static demo transcripts", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByText("No transcription history yet")).toBeTruthy();
    expect(screen.queryByText("Design review notes")).toBeNull();
    expect(screen.queryByText("Product demo call")).toBeNull();
  });

  it("records audio, transcribes it, stores the result, and stays on the current page", async () => {
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
        "http://127.0.0.1:8000/v1/audio/transcriptions",
        expect.objectContaining({
          method: "POST",
          body: expect.any(FormData),
        }),
      );
    });
    const formData = fetchMock.mock.calls[0][1].body as FormData;
    expect(formData.get("model")).toBe("whisper-base");

    await waitFor(() => {
      const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
      expect(stored).toHaveLength(1);
    });

    expect(screen.getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe("page");
    expect(screen.queryByRole("heading", { name: "Transcript library" })).toBeNull();
    expect(screen.getByText("Recordings")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "History" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Transcript library" })).toBeTruthy());
    expect(screen.getByText("Buy milk and schedule the product demo.")).toBeTruthy();
    const audio = screen.getByLabelText("Recording audio: Buy milk and schedule the product demo.");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio.getAttribute("src")).toMatch(/^data:audio\/webm/);
    const playSpy = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
    const pauseSpy = vi.spyOn(window.HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);

    await user.click(screen.getByRole("button", { name: "Play recording: Buy milk and schedule the product demo." }));

    expect(playSpy).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByRole("button", { name: "Pause recording: Buy milk and schedule the product demo." })).toBeTruthy());

    await user.click(screen.getByRole("button", { name: "Pause recording: Buy milk and schedule the product demo." }));
    expect(pauseSpy).toHaveBeenCalledTimes(1);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].text).toBe("Buy milk and schedule the product demo.");
    expect(stored[0].kind).toBe("Dictation");
    expect(stored[0].recordingUrl).toMatch(/^data:audio\/webm/);
  });

  it("keeps a playable recording in history when transcription fetch fails without changing pages", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    const fetchMock = vi.fn().mockRejectedValue(new TypeError("Failed to fetch"));
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));
    await screen.findByRole("button", { name: "Stop Recording" });
    await user.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => {
      const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
      expect(stored).toHaveLength(1);
    });

    expect(screen.getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe("page");
    expect(screen.queryByRole("heading", { name: "Transcript library" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "History" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Transcript library" })).toBeTruthy());
    expect(screen.getByText("Recording failed to transcribe")).toBeTruthy();
    expect(screen.getByText(/Failed to fetch/)).toBeTruthy();

    const audio = screen.getByLabelText("Recording audio: Recording failed to transcribe");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio.getAttribute("src")).toMatch(/^data:audio\/webm/);
    const playSpy = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);

    await user.click(screen.getByRole("button", { name: "Play recording: Recording failed to transcribe" }));
    expect(playSpy).toHaveBeenCalledTimes(1);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].status).toBe("failed");
    expect(stored[0].error).toBe("Failed to fetch");
    expect(stored[0].recordingUrl).toMatch(/^data:audio\/webm/);
  });

  it("syncs recording state from the global shortcut and tray bridge without changing pages", async () => {
    mockAudioCapture();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => ({
        text: "Shortcut dictation stayed on home.",
      }),
    });
    vi.stubGlobal("fetch", fetchMock);
    let recordingListener: ((state: { isRecording: boolean; source: string }) => void) | undefined;
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Local Whisper",
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

    await waitFor(() => expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy());
    expect(screen.queryByText("Global overlay active")).toBeNull();
    expect(screen.queryByText(/CommandOrControl/)).toBeNull();

    await act(async () => {
      recordingListener?.({ isRecording: false, source: "shortcut" });
    });

    await waitFor(() => {
      const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
      expect(stored).toHaveLength(1);
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe("page");
    expect(screen.queryByRole("heading", { name: "Transcript library" })).toBeNull();
  });

  it("uses neutral page status labels instead of colored status pills", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sound" }));
    expect(screen.getByText(/^(Default|Ready)$/)).toBeTruthy();
    expect(screen.getByText("Local")).toBeTruthy();

    const classNames = renderedClassNames();
    expect(classNames).not.toContain("bg-[#244735]");
    expect(classNames).not.toContain("bg-[#284862]");
    expect(classNames).not.toContain("bg-[#5a2c29]");
    expect(classNames).not.toContain("text-[#9ee0b6]");
    expect(classNames).not.toContain("text-[#9fd2ff]");
    expect(classNames).not.toContain("text-[#ffb3aa]");
  });

  it("updates the recording overlay placement from settings", async () => {
    const user = userEvent.setup();
    const setOverlaySettings = vi.fn().mockResolvedValue({ placement: "bottom", customBounds: null });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
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
    await user.click(screen.getByRole("button", { name: "Configuration" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Top overlay position" }).getAttribute("aria-pressed")).toBe("true"));

    await user.click(screen.getByRole("button", { name: "Bottom overlay position" }));

    expect(setOverlaySettings).toHaveBeenCalledWith({ placement: "bottom" });
    expect(screen.getByRole("button", { name: "Bottom overlay position" }).getAttribute("aria-pressed")).toBe("true");
  });
});
