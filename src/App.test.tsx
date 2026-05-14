import { act } from "react";
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { audioRecordingService } from "./services/audioRecording";
import packageMetadata from "../package.json";

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

function appStylesheet() {
  return readFileSync("src/index.css", "utf8");
}

describe("ASR Pro Electron shell", () => {
  it("scopes the app shell to non-selectable chrome while keeping text exceptions selectable", () => {
    render(<App />);

    const shell = document.body.firstElementChild?.firstElementChild;
    const homeButton = screen.getByRole("button", { name: "Home" });
    const statusDescription = screen.getByText("Turn your voice to text with a single click.");
    const updateDescription = screen.getByText("Saved dictations keep playable source audio with their transcripts.");

    expect(shell?.className).toContain("app-chrome");
    expect(homeButton.className).not.toContain("selectable-text");
    expect(statusDescription.className).toContain("selectable-text");
    expect(updateDescription.className).toContain("selectable-text");
  });

  it("keeps transcript content and history search selectable inside the non-selectable shell", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "history-selection-row",
        title: "Meeting follow-up",
        text: "Send the launch notes and review the transcript.",
        kind: "Dictation",
        model: "Local Whisper",
        durationSeconds: 12,
        createdAt: Date.now(),
        status: "completed",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    const search = screen.getByRole("searchbox", { name: "Search history" }) as HTMLInputElement;
    const title = screen.getByText("Meeting follow-up");
    const transcript = screen.getByText("Send the launch notes and review the transcript.");

    expect(search.className).not.toContain("selectable-text");
    await user.type(search, "launch");
    expect(search.value).toBe("launch");
    expect(title.className).toContain("selectable-text");
    expect(transcript.className).toContain("selectable-text");
  });

  it("defines scoped CSS that prevents drag selection except for readable text and fields", () => {
    const css = appStylesheet();

    expect(css).toContain(".app-chrome");
    expect(css).toContain("user-select: none");
    expect(css).toContain("-webkit-user-select: none");
    expect(css).toContain("-webkit-user-drag: none");
    expect(css).toContain(".app-chrome .selectable-text");
    expect(css).toContain("user-select: text");
    expect(css).toContain(".app-chrome :is(input, textarea, select, [contenteditable=\"true\"])");
  });

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
    expect(screen.getByText("Review history")).toBeTruthy();
    expect(screen.getByText("Choose speech model")).toBeTruthy();
    expect(screen.queryByText("Customize shortcuts")).toBeNull();
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

  it("renders only fixed close and minimize traffic lights with hover glyphs", () => {
    render(<App />);

    const closeButton = screen.getByRole("button", { name: "Close window" });
    const minimizeButton = screen.getByRole("button", { name: "Minimize window" });

    expect(screen.queryByRole("button", { name: "Maximize window" })).toBeNull();
    expect(closeButton.classList).toContain("size-[13px]");
    expect(closeButton.classList).toContain("border-0");
    expect(closeButton.classList).toContain("outline-none");
    expect(closeButton.classList).not.toContain("border");
    expect(minimizeButton.classList).toContain("size-[13px]");
    expect(minimizeButton.classList).toContain("border-0");
    expect(minimizeButton.classList).toContain("outline-none");
    expect(minimizeButton.classList).not.toContain("border");
    expect(closeButton.querySelector('[data-window-dot-icon="close"]')).toBeTruthy();
    expect(minimizeButton.querySelector('[data-window-dot-icon="minimize"]')).toBeTruthy();
  });

  it("renders About metadata as a flat definition list instead of nested fact cards", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "About" }));

    const facts = document.querySelector('dl[aria-label="Product facts"]');

    expect(facts).toBeTruthy();
    expect(facts?.querySelectorAll("dt")).toHaveLength(3);
    expect(facts?.querySelectorAll("dd")).toHaveLength(3);
    expect(screen.getByText("Version").tagName).toBe("DT");
    expect(screen.getByText(packageMetadata.version).tagName).toBe("DD");
  });

  it("renders About content on grouped app surfaces without standalone white rules", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "About" }));

    const summaryPanel = document.querySelector('section[aria-label="About product summary"]');
    const facts = document.querySelector('dl[aria-label="Product facts"]');

    expect(summaryPanel).toBeTruthy();
    expect(summaryPanel?.className).toContain("rounded-[22px]");
    expect(summaryPanel?.className).toContain("bg-white/[0.055]");
    expect(facts?.className).not.toContain("border-y");
    expect(facts?.className).not.toContain("divide-y");
    expect(screen.queryByRole("list", { name: "Highlights" })).toBeNull();
  });

  it("shows real About metadata without implementation stack or highlights", async () => {
    const user = userEvent.setup();
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn().mockResolvedValue({ name: "ASR Pro", version: "2.4.6" }),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Local Whisper",
        dataDir: "/Users/surajmandal/Library/Application Support/ASR Pro/data",
        shortcut: "CommandOrControl+`",
      }),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      windowControl: vi.fn(),
    };

    render(<App />);
    await user.click(screen.getByRole("button", { name: "About" }));

    await waitFor(() => expect(screen.getAllByText("2.4.6").length).toBeGreaterThan(0));
    expect(screen.queryByText("Electron + React")).toBeNull();
    expect(screen.queryByText("Runtime")).toBeNull();
    expect(screen.queryByText("Highlights")).toBeNull();
    expect(screen.queryByRole("list", { name: "Highlights" })).toBeNull();
    expect(screen.getByText("/Users/surajmandal/Library/Application Support/ASR Pro/data")).toBeTruthy();
  });

  it("renders GitHub project and issue links on About", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "About" }));

    const githubLink = screen.getByRole("link", { name: /GitHub/i });
    const issueLink = screen.getByRole("link", { name: /Report issue/i });

    expect(githubLink.getAttribute("href")).toBe("https://github.com/surajmandalcell/asrpro");
    expect(githubLink.getAttribute("target")).toBe("_blank");
    expect(issueLink.getAttribute("href")).toBe("https://github.com/surajmandalcell/asrpro/issues/new");
    expect(issueLink.getAttribute("target")).toBe("_blank");
  });

  it("navigates to the transcript history", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByRole("heading", { name: "History" })).toBeTruthy();
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

  it("uses microphone device icons instead of dropdown arrows", async () => {
    const user = userEvent.setup();
    mockAudioCapture([
      { kind: "audioinput", deviceId: "macbook-mic", label: "MacBook Pro Microphone" },
      { kind: "audioinput", deviceId: "iphone-mic", label: "Suraj's iPhone Microphone" },
      { kind: "audioinput", deviceId: "webcam-mic", label: "Logitech Webcam Microphone" },
    ]);

    render(<App />);

    const toolbarSelector = await screen.findByRole("button", { name: "Toolbar microphone selector" });
    expect(toolbarSelector.querySelector(".lucide-chevron-down")).toBeNull();
    expect(toolbarSelector.querySelector('[data-device-icon="mic"]')).toBeTruthy();

    await user.click(toolbarSelector);

    expect(screen.getByRole("option", { name: "MacBook Pro Microphone" }).querySelector("[data-device-icon]")?.getAttribute("data-device-icon")).toBe("laptop");
    expect(screen.getByRole("option", { name: "Suraj's iPhone Microphone" }).querySelector('[data-device-icon="phone"]')).toBeTruthy();
    expect(screen.getByRole("option", { name: "Logitech Webcam Microphone" }).querySelector('[data-device-icon="webcam"]')).toBeTruthy();
  });

  it("uses shared rounded styling for the Sound microphone controls", async () => {
    const user = userEvent.setup();
    mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
      { kind: "audioinput", deviceId: "usb-mic", label: "USB Microphone" },
    ]);

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Sound" }));

    const selector = await screen.findByRole("button", { name: "Microphone selector" });
    const refreshButton = screen.getByRole("button", { name: "Refresh microphones" });

    expect(selector.className).toContain("rounded-[12px]");
    expect(selector.className).not.toContain("rounded-md");
    expect(selector.className).not.toContain("rounded-[7px]");
    expect(refreshButton.className).toContain("rounded-[12px]");
    expect(refreshButton.className).not.toContain("rounded-md");
    expect(refreshButton.className).not.toContain("rounded-[7px]");

    await user.click(selector);

    const listbox = screen.getByRole("listbox", { name: "Microphone options" });
    const option = screen.getByRole("option", { name: "USB Microphone" });

    expect(listbox.className).toContain("rounded-[12px]");
    expect(listbox.className).not.toContain("rounded-[9px]");
    expect(option.className).toContain("rounded-[10px]");
    expect(option.className).not.toContain("rounded-[7px]");
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
    expect(screen.queryByRole("heading", { name: "History" })).toBeNull();
    expect(screen.getByText("Get started")).toBeTruthy();
    expect(screen.getByText("Recordings")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "History" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "History" })).toBeTruthy());
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
    expect(screen.queryByRole("heading", { name: "History" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "History" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "History" })).toBeTruthy());
    expect(screen.getByText("Recording failed to transcribe")).toBeTruthy();
    expect(screen.getByText("Failed to load.")).toBeTruthy();

    const audio = screen.getByLabelText("Recording audio: Recording failed to transcribe");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio.getAttribute("src")).toMatch(/^data:audio\/webm/);
    const playSpy = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);

    await user.click(screen.getByRole("button", { name: "Play recording: Recording failed to transcribe" }));
    expect(playSpy).toHaveBeenCalledTimes(1);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].status).toBe("failed");
    expect(stored[0].error).toBe("Failed to load.");
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
    expect(screen.queryByRole("heading", { name: "History" })).toBeNull();
  });

  it("uses neutral page status labels instead of colored status pills", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sound" }));
    expect(screen.getByText(/^(Default|Ready)$/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Change" })).toBeTruthy();

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
    expect(screen.getByText("Recording overlay")).toBeTruthy();
    expect(screen.queryByText("Recording window")).toBeNull();
    expect(screen.queryByText("Classic")).toBeNull();
    expect(screen.queryByText("Hidden")).toBeNull();
    await waitFor(() => expect(screen.getByRole("button", { name: "Top overlay position" }).getAttribute("aria-pressed")).toBe("true"));

    await user.click(screen.getByRole("button", { name: "Bottom overlay position" }));

    expect(setOverlaySettings).toHaveBeenCalledWith({ placement: "bottom" });
    expect(screen.getByRole("button", { name: "Bottom overlay position" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("uses shared rounded styling inside the configuration position control", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Configuration" }));

    const topButton = screen.getByRole("button", { name: "Top overlay position" });
    const bottomButton = screen.getByRole("button", { name: "Bottom overlay position" });
    const segmentedControl = topButton.parentElement;

    expect(segmentedControl?.className).toContain("rounded-[12px]");
    expect(topButton.className).toContain("rounded-[10px]");
    expect(bottomButton.className).toContain("rounded-[10px]");
    expect(topButton.className).not.toContain("rounded-[7px]");
    expect(bottomButton.className).not.toContain("rounded-[7px]");
  });

  it("routes configuration references to their real settings pages", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Configuration" }));
    await user.click(screen.getAllByRole("button", { name: "Change" })[0]);

    expect(screen.getByRole("button", { name: "Models library" }).getAttribute("aria-current")).toBe("page");

    await user.click(screen.getByRole("button", { name: "Configuration" }));
    await user.click(screen.getAllByRole("button", { name: "Change" })[1]);

    expect(screen.getByRole("button", { name: "Sound" }).getAttribute("aria-current")).toBe("page");
  });
});
