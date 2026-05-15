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
        model: "Whisper Base English",
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

  it("defines subtle inset scrollbar styling with class-specific autohide", () => {
    const css = appStylesheet();

    expect(css).toContain("*::-webkit-scrollbar");
    expect(css).toContain(".scrollbar-macos");
    expect(css).toContain(".scrollbar-autohide");
    expect(css).toContain("width: 8px;");
    expect(css).toContain("height: 8px;");
    expect(css).toContain("border: 3px solid transparent;");
    expect(css).toContain("background-clip: content-box;");
    expect(css).toContain("rgba(214, 214, 214, 0.12)");
    expect(css).toContain(".scrollbar-autohide:not(.is-scrollbar-visible)");
    expect(css).not.toContain("width: 10px;");
  });

  it("keeps scrollbar autohide behavior off dropdowns and sidebar scroll areas", async () => {
    const user = userEvent.setup();
    mockAudioCapture([
      { kind: "audioinput", deviceId: "built-in-mic", label: "Built-in Microphone" },
      { kind: "audioinput", deviceId: "usb-mic", label: "USB Microphone" },
    ]);

    render(<App />);
    const mainPane = document.querySelector("main");
    const sidebarNav = screen.getByRole("navigation", { name: "Primary" });

    expect(mainPane?.className).toContain("scrollbar-macos");
    expect(mainPane?.className).toContain("scrollbar-autohide");
    expect(sidebarNav.className).toContain("scrollbar-macos");
    expect(sidebarNav.className).not.toContain("scrollbar-autohide");
    expect(sidebarNav.className).not.toContain("is-scrollbar-visible");

    await user.click(screen.getByRole("button", { name: "Sound" }));
    const selector = await screen.findByRole("button", { name: "Microphone selector" });
    await user.click(selector);

    const listbox = screen.getByRole("listbox", { name: "Microphone options" });
    expect(listbox.className).toContain("scrollbar-macos");
    expect(listbox.className).toContain("dropdown-options-scrollbar");
    expect(listbox.className).toContain("overflow-y-auto");
    expect(listbox.className).toContain("overflow-x-hidden");
    expect(listbox.className).toContain("pr-0.5");
    expect(listbox.className).not.toContain("scrollbar-autohide");
    expect(listbox.className).not.toContain("is-scrollbar-visible");
    expect(appStylesheet()).toContain(".dropdown-options-scrollbar::-webkit-scrollbar-track");
    expect(appStylesheet()).toContain("margin-block: 6px;");
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

  it("uses the Ink Slate brand tile on About", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "About" }));

    const brandTile = document.querySelector('[data-brand-icon-surface="ink-slate"]');

    expect(brandTile).toBeTruthy();
    expect(brandTile?.getAttribute("style")).toContain("rgb(32, 39, 45)");
    expect(brandTile?.getAttribute("style")).toContain("rgb(16, 23, 29)");
    expect(brandTile?.getAttribute("style")).toContain("rgb(4, 7, 10)");
  });

  it("shows real About metadata without implementation stack or highlights", async () => {
    const user = userEvent.setup();
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn().mockResolvedValue({ name: "ASR Pro", version: "2.4.6" }),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
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
    expect(screen.getByText("~/Library/Application Support/ASR Pro/data")).toBeTruthy();
    expect(screen.queryByText("/Users/surajmandal/Library/Application Support/ASR Pro/data")).toBeNull();
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

  it("keeps the About sidebar icon neutral", () => {
    render(<App />);

    const aboutButton = screen.getByRole("button", { name: "About" });
    const aboutIconTile = aboutButton.querySelector("span");

    expect(aboutIconTile?.className).toContain("bg-[#727272]");
    expect(aboutIconTile?.className).toContain("text-white");
    expect(aboutIconTile?.className).not.toContain("#92c2c6");
    expect(aboutIconTile?.className).not.toContain("#b9dfe2");
    expect(aboutIconTile?.className).not.toContain("border");
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

  it("purges seeded screenshot fixture rows from normal local history", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "readme-history-1",
        title: "Product demo follow-up",
        text: "Summarize the product demo, send the follow-up notes, and schedule the model comparison review.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 58,
        createdAt: Date.now(),
        status: "completed",
      },
      {
        id: "real-history-row",
        title: "Original planning note",
        text: "Keep the real user transcript and saved source audio.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 12,
        createdAt: Date.now(),
        status: "completed",
        recordingUrl: "data:audio/webm;base64,cmVhbA==",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.queryByText("Product demo follow-up")).toBeNull();
    expect(screen.getByText("Original planning note")).toBeTruthy();
    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0]).toMatchObject({
      id: "real-history-row",
      recordingUrl: "data:audio/webm;base64,cmVhbA==",
    });
  });

  it("keeps seeded screenshot fixture rows only in screenshot mode", async () => {
    const user = userEvent.setup();
    window.asrpro = { isScreenshotMode: true } as any;
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "readme-history-1",
        title: "Product demo follow-up",
        text: "Summarize the product demo, send the follow-up notes, and schedule the model comparison review.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 58,
        createdAt: Date.now(),
        status: "completed",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByText("Product demo follow-up")).toBeTruthy();
    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].id).toBe("readme-history-1");
  });

  it("shows when a history transcript has no saved source audio", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "history-missing-audio",
        title: "Team retro notes",
        text: "Summarize the retro themes and send the follow-up notes.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 58,
        createdAt: Date.now(),
        status: "completed",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.getByText("Team retro notes")).toBeTruthy();
    expect(screen.getByText("No source audio saved")).toBeTruthy();
    expect(screen.queryByLabelText("Recording audio: Team retro notes")).toBeNull();
    expect(screen.queryByRole("button", { name: "Reprocess clip: Team retro notes" })).toBeNull();
  });

  it("reprocesses a saved history clip from the row actions", async () => {
    const user = userEvent.setup();
    const transcribeAudio = vi.fn().mockResolvedValue({
      text: "Updated transcript from the saved clip.",
      model: "whisper-base-en",
      modelName: "Whisper Base English",
    });
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    window.asrpro = {
      transcribeAudio,
      windowControl: vi.fn(),
    } as any;
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "history-reprocess-row",
        title: "Original clip",
        text: "Original transcript text.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 18,
        createdAt: Date.now(),
        status: "completed",
        recordingUrl: "data:audio/webm;base64,c2F2ZWQgYXVkaW8=",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    await user.click(screen.getByRole("button", { name: "Reprocess clip: Original clip" }));

    await waitFor(() => {
      expect(transcribeAudio).toHaveBeenCalledWith(expect.objectContaining({
        audioData: expect.any(ArrayBuffer),
        mimeType: "audio/webm",
        modelId: "whisper-base-en",
      }));
    });
    expect((transcribeAudio.mock.calls[0][0].audioData as ArrayBuffer).byteLength).toBeGreaterThan(0);
    expect(fetchMock).not.toHaveBeenCalled();

    await waitFor(() => expect(screen.getByText("Updated transcript from the saved clip.")).toBeTruthy());
    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0]).toMatchObject({
      id: "history-reprocess-row",
      title: "Updated transcript from the saved clip.",
      text: "Updated transcript from the saved clip.",
      model: "Whisper Base English",
      status: "completed",
      recordingUrl: "data:audio/webm;base64,c2F2ZWQgYXVkaW8=",
    });
  });

  it("opens a saved history transcript in the text editor action and keeps action icons visually consistent", async () => {
    const user = userEvent.setup();
    const openTranscriptText = vi.fn().mockResolvedValue({
      filePath: "/Users/surajmandal/Library/Application Support/ASR Pro/data/transcripts/original-clip.txt",
    });
    window.asrpro = {
      openTranscriptText,
      windowControl: vi.fn(),
    } as any;
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "history-open-text-row",
        title: "Original clip",
        text: "Original transcript text.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 18,
        createdAt: Date.now(),
        status: "completed",
        recordingUrl: "data:audio/webm;base64,c2F2ZWQgYXVkaW8=",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    expect(screen.queryByText("0:18")).toBeNull();

    const reprocessButton = screen.getByRole("button", { name: "Reprocess clip: Original clip" });
    const openTextButton = screen.getByRole("button", { name: "Open transcript text: Original clip" });
    const copyButton = screen.getByRole("button", { name: "Copy transcript: Original clip" });
    const deleteButton = screen.getByRole("button", { name: "Delete transcript: Original clip" });

    expect([
      reprocessButton,
      openTextButton,
      copyButton,
      deleteButton,
    ].map((button) => button.className)).toEqual([
      reprocessButton.className,
      reprocessButton.className,
      reprocessButton.className,
      reprocessButton.className,
    ]);

    await user.click(openTextButton);

    expect(openTranscriptText).toHaveBeenCalledWith({
      title: "Original clip",
      text: "Original transcript text.",
    });
  });

  it("deletes an opened transcript text file when deleting its history row", async () => {
    const user = userEvent.setup();
    const transcriptFilePath = "/Users/surajmandal/Library/Application Support/ASR Pro/data/transcripts/original-clip.txt";
    const openTranscriptText = vi.fn().mockResolvedValue({ filePath: transcriptFilePath });
    const deleteTranscriptText = vi.fn().mockResolvedValue({ deleted: true });
    window.asrpro = {
      openTranscriptText,
      deleteTranscriptText,
      windowControl: vi.fn(),
    } as any;
    window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify([
      {
        id: "history-delete-text-row",
        title: "Original clip",
        text: "Original transcript text.",
        kind: "Dictation",
        model: "Whisper Base English",
        durationSeconds: 18,
        createdAt: Date.now(),
        status: "completed",
        recordingUrl: "data:audio/webm;base64,c2F2ZWQgYXVkaW8=",
      },
    ]));

    render(<App />);
    await user.click(screen.getByRole("button", { name: "History" }));

    await user.click(screen.getByRole("button", { name: "Open transcript text: Original clip" }));
    await waitFor(() => expect(openTranscriptText).toHaveBeenCalledWith({
      title: "Original clip",
      text: "Original transcript text.",
    }));

    await user.click(screen.getByRole("button", { name: "Delete transcript: Original clip" }));

    await waitFor(() => expect(deleteTranscriptText).toHaveBeenCalledWith({
      title: "Original clip",
      filePath: transcriptFilePath,
    }));
    expect(screen.queryByText("Original clip")).toBeNull();
    expect(JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]")).toEqual([]);
  });

  it("records audio, transcribes it, stores the result, and stays on the current page", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    const transcribeAudio = vi.fn().mockResolvedValue({
      text: "Buy milk and schedule the product demo.",
      model: "whisper-base-en",
      modelName: "Whisper Base English",
    });
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    window.asrpro = {
      transcribeAudio,
      windowControl: vi.fn(),
    } as any;

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));
    await screen.findByRole("button", { name: "Stop Recording" });
    await user.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => {
      expect(transcribeAudio).toHaveBeenCalledWith(expect.objectContaining({
        audioData: expect.any(ArrayBuffer),
        mimeType: expect.any(String),
        modelId: "whisper-base-en",
      }));
    });
    expect(fetchMock).not.toHaveBeenCalled();

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

  it("keeps a playable recording in history when native transcription fails without changing pages", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    const transcribeAudio = vi.fn().mockRejectedValue(new Error("Whisper model download failed."));
    window.asrpro = {
      transcribeAudio,
      windowControl: vi.fn(),
    } as any;

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
    expect(screen.getByText("Whisper model download failed. Check your connection and try again.")).toBeTruthy();

    const audio = screen.getByLabelText("Recording audio: Recording failed to transcribe");
    expect(audio.tagName).toBe("AUDIO");
    expect(audio.getAttribute("src")).toMatch(/^data:audio\/webm/);
    const playSpy = vi.spyOn(window.HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);

    await user.click(screen.getByRole("button", { name: "Play recording: Recording failed to transcribe" }));
    expect(playSpy).toHaveBeenCalledTimes(1);

    const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].status).toBe("failed");
    expect(stored[0].error).toBe("Whisper model download failed. Check your connection and try again.");
    expect(stored[0].recordingUrl).toMatch(/^data:audio\/webm/);
  });

  it("queues transcription behind the native Whisper engine and shows preparation status", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    let resolveTranscription: ((result: { text: string; model: string }) => void) | undefined;
    const transcribeAudio = vi.fn(() => new Promise<{ text: string; model: string }>((resolve) => {
      resolveTranscription = resolve;
    }));
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
        shortcut: "CommandOrControl+`",
        engine: { status: "idle", mode: "native-node" },
        capabilities: { nativeWhisper: true },
      }),
      transcribeAudio,
      setRecording: vi.fn().mockResolvedValue({ isRecording: false }),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      onEngineState: vi.fn(),
      windowControl: vi.fn(),
    } as any;

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));
    await screen.findByRole("button", { name: "Stop Recording" });
    await user.click(screen.getByRole("button", { name: "Stop Recording" }));

    await screen.findByText("Loading Whisper model and transcribing...");
    expect(screen.getByText("Transcribing")).toBeTruthy();
    expect(transcribeAudio).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveTranscription?.({ text: "Queued transcription completed after native Whisper became ready.", model: "whisper-base-en" });
    });

    await waitFor(() => {
      const stored = JSON.parse(window.localStorage.getItem("asrpro.transcriptHistory.v1") || "[]");
      expect(stored[0]?.text).toBe("Queued transcription completed after native Whisper became ready.");
    });
  });

  it("does not call the renderer network path for transcription", async () => {
    const user = userEvent.setup();
    mockAudioCapture();
    const transcribeAudio = vi.fn().mockResolvedValue({ text: "Native Whisper path only.", model: "whisper-base-en" });
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
        shortcut: "CommandOrControl+`",
        engine: { status: "idle", mode: "native-node" },
      }),
      transcribeAudio,
      setRecording: vi.fn().mockResolvedValue({ isRecording: false }),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      onEngineState: vi.fn(),
      windowControl: vi.fn(),
    } as any;

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));
    await screen.findByRole("button", { name: "Stop Recording" });
    await user.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => expect(transcribeAudio).toHaveBeenCalledTimes(1));
    expect(screen.queryByText(/No handler registered/i)).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("syncs recording state from the global shortcut and tray bridge without changing pages", async () => {
    mockAudioCapture();
    const transcribeAudio = vi.fn().mockResolvedValue({
      text: "Shortcut dictation stayed on home.",
      model: "whisper-base-en",
    });
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    let recordingListener: ((state: { isRecording: boolean; source: string }) => void) | undefined;
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
        shortcut: "CommandOrControl+`",
      }),
      transcribeAudio,
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
    expect(transcribeAudio).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
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

  it("shows selectable native Whisper model options", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Models library" }));

    const baseButton = screen.getByRole("button", { name: "Select Whisper Base English" });
    const tinyButton = screen.getByRole("button", { name: "Select Whisper Tiny English" }) as HTMLButtonElement;
    const smallButton = screen.getByRole("button", { name: "Select Whisper Small English" }) as HTMLButtonElement;
    const multilingualButton = screen.getByRole("button", { name: "Select Whisper Base Multilingual" }) as HTMLButtonElement;

    expect(baseButton.getAttribute("aria-pressed")).toBe("true");
    expect(tinyButton.disabled).toBe(false);
    expect(smallButton.disabled).toBe(false);
    expect(multilingualButton.disabled).toBe(false);
    expect(screen.queryByText("Future placeholder")).toBeNull();
  });

  it("manages individual Whisper model setup and grouped resource stats", async () => {
    const user = userEvent.setup();
    const initialModels = [
      {
        id: "whisper-base-en",
        displayName: "Whisper Base English",
        detail: "Default local model for English dictation",
        sizeLabel: "142 MiB",
        installed: true,
        diskBytes: 148_897_792,
      },
      {
        id: "whisper-large-v3-turbo",
        displayName: "Whisper Large v3 Turbo",
        detail: "High accuracy multilingual model with faster large-model decoding",
        sizeLabel: "1.5 GiB",
        installed: false,
        diskBytes: 0,
      },
    ];
    const storageStats = {
      groups: [
        {
          id: "memory",
          label: "Runtime memory",
          totalBytes: 125_829_120,
          items: [
            { id: "resident", label: "Resident set", bytes: 125_829_120 },
            { id: "heap", label: "JavaScript heap", bytes: 41_943_040 },
            { id: "model-footprint", label: "AI model footprint", bytes: 148_897_792 },
          ],
        },
        {
          id: "disk",
          label: "App data on disk",
          totalBytes: 1_759_871_488,
          items: [
            { id: "whisper-models", label: "Whisper models", bytes: 1_610_612_736 },
            { id: "transcripts", label: "Transcripts", bytes: 149_258_752 },
          ],
        },
      ],
    };
    const downloadModel = vi.fn().mockResolvedValue({
      isRecording: false,
      defaultModel: "Whisper Base English",
      models: initialModels.map((model) => (
        model.id === "whisper-large-v3-turbo" ? { ...model, installed: true, diskBytes: 1_610_612_736 } : model
      )),
      storageStats,
    });
    const deleteModel = vi.fn().mockResolvedValue({
      isRecording: false,
      defaultModel: "Whisper Base English",
      models: initialModels.map((model) => (
        model.id === "whisper-base-en" ? { ...model, installed: false, diskBytes: 0 } : model
      )),
      storageStats,
    });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
        models: initialModels,
        storageStats,
        shortcut: "CommandOrControl+`",
      }),
      downloadModel,
      deleteModel,
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      windowControl: vi.fn(),
    } as any;

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Models library" }));

    expect(await screen.findByRole("button", { name: "Select Whisper Large v3 Turbo" })).toBeTruthy();
    expect(screen.getAllByText("1.5 GiB").length).toBeGreaterThan(0);
    expect(screen.getByText("Runtime memory")).toBeTruthy();
    expect(screen.getByText("Resident set")).toBeTruthy();
    expect(screen.getByText("AI model footprint")).toBeTruthy();
    expect(screen.getAllByText("120 MiB").length).toBeGreaterThan(0);
    expect(screen.getByText("Whisper models")).toBeTruthy();
    const turboMetaRow = screen.getByText("Whisper Large v3 Turbo").parentElement;
    expect(turboMetaRow?.textContent).toContain("1.5 GiB");
    expect(turboMetaRow?.textContent).not.toContain("Needs setup");
    expect(screen.queryByText("Needs setup")).toBeNull();
    const sizeBadge = turboMetaRow?.querySelector("span:nth-child(2)");
    expect(sizeBadge?.className).toContain("text-[10px]");
    expect(screen.queryByText("Selected")).toBeNull();
    expect(screen.queryByText(/^Ready$/)).toBeNull();

    const baseSelectButton = screen.getByRole("button", { name: "Select Whisper Base English" });
    expect(baseSelectButton.textContent).not.toContain("Selected");
    expect(baseSelectButton.parentElement?.textContent).toContain("Current model");
    expect(baseSelectButton.className).toContain("text-[#9bcfff]");
    expect(baseSelectButton.querySelector(".lucide-check")).toBeTruthy();
    expect(baseSelectButton.querySelector(".lucide-circle-check")).toBeNull();
    const downloadedStatus = screen.getByLabelText("Whisper Base English downloaded");
    expect(downloadedStatus.className).toContain("text-[#a9d9b8]");
    expect(downloadedStatus.querySelector(".lucide-circle-check")).toBeTruthy();
    expect(downloadedStatus.parentElement?.textContent).toContain("Downloaded model");
    const pendingStatus = screen.getByLabelText("Whisper Large v3 Turbo not downloaded");
    expect(pendingStatus.className).toContain("text-[#cfcfcf]");
    expect(pendingStatus.className).toContain("opacity-75");
    expect(pendingStatus.querySelector(".lucide-circle-check")).toBeTruthy();
    const controlColumns = Array.from(document.querySelectorAll("[data-model-controls]"));
    expect(controlColumns).toHaveLength(initialModels.length);
    for (const controls of controlColumns) {
      expect(controls.className).toContain("grid-cols-[32px_32px_32px]");
      expect(controls.textContent).not.toContain("Needs setup");
    }

    const downloadButton = screen.getByRole("button", { name: "Download Whisper Large v3 Turbo" });
    expect(downloadButton.textContent).not.toContain("Download");
    expect(downloadButton.className).toContain("border-0");

    await user.click(downloadButton);
    expect(downloadModel).toHaveBeenCalledWith("whisper-large-v3-turbo");

    const deleteButton = screen.getByRole("button", { name: "Delete Whisper Base English" });
    expect(deleteButton.textContent).not.toContain("Delete");
    expect(deleteButton.className).toContain("border-0");

    await user.click(deleteButton);
    expect(deleteModel).toHaveBeenCalledWith("whisper-base-en");
  });

  it("selects models only through the check button", async () => {
    const user = userEvent.setup();
    const models = [
      {
        id: "whisper-base-en",
        displayName: "Whisper Base English",
        detail: "Default local model for English dictation",
        sizeLabel: "142 MiB",
        installed: true,
        diskBytes: 148_897_792,
      },
      {
        id: "whisper-small-en",
        displayName: "Whisper Small English",
        detail: "Better accuracy, larger local model",
        sizeLabel: "466 MiB",
        installed: false,
        diskBytes: 0,
      },
    ];
    const getRuntimeState = vi.fn().mockResolvedValue({
      isRecording: false,
      defaultModel: "Whisper Base English",
      models,
      shortcut: "CommandOrControl+`",
    });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState,
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      windowControl: vi.fn(),
    } as any;

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Models library" }));

    const smallButton = await screen.findByRole("button", { name: "Select Whisper Small English" });
    await waitFor(() => expect(getRuntimeState).toHaveBeenCalledTimes(1));
    const smallName = screen.getByText("Whisper Small English");

    expect(smallName.closest("button")).toBeNull();
    await user.click(smallName);
    expect(smallButton.getAttribute("aria-pressed")).toBe("false");

    await user.click(smallButton);

    await waitFor(() => expect(smallButton.getAttribute("aria-pressed")).toBe("true"));
    expect(getRuntimeState).toHaveBeenCalledTimes(1);
    expect(smallButton.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Select Whisper Base English" }).getAttribute("aria-pressed")).toBe("false");
  });

  it("updates the recording overlay placement from settings", async () => {
    const user = userEvent.setup();
    const setOverlaySettings = vi.fn().mockResolvedValue({ placement: "bottom", customBounds: null });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
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
    expect(screen.getByText("Engine")).toBeTruthy();
    expect(screen.queryByText("Recording window")).toBeNull();
    expect(screen.queryByText("Classic")).toBeNull();
    expect(screen.queryByText("Hidden")).toBeNull();
    await waitFor(() => expect(screen.getByRole("button", { name: "Top overlay position" }).getAttribute("aria-pressed")).toBe("true"));

    await user.click(screen.getByRole("button", { name: "Bottom overlay position" }));

    expect(setOverlaySettings).toHaveBeenCalledWith({ placement: "bottom" });
    expect(screen.getByRole("button", { name: "Bottom overlay position" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("selects the default text editor from configuration", async () => {
    const user = userEvent.setup();
    const iconDataUrl = "data:image/gif;base64,R0lGODlhAQABAAAAACw=";
    const setDefaultTextEditor = vi.fn().mockResolvedValue({ defaultTextEditor: "textedit" });
    window.asrpro = {
      getPlatform: vi.fn(),
      getAppInfo: vi.fn(),
      getRuntimeState: vi.fn().mockResolvedValue({
        isRecording: false,
        defaultModel: "Whisper Base English",
        defaultTextEditor: "system",
        textEditors: [
          { id: "system", label: "System default", detail: "Use the operating system default editor", iconDataUrl },
          { id: "textedit", label: "TextEdit", detail: "Open transcript text in Apple TextEdit", iconDataUrl },
          { id: "vscode", label: "Visual Studio Code", detail: "Open transcript text in VS Code", iconDataUrl },
          { id: "cursor", label: "Cursor", detail: "Open transcript text in Cursor", iconDataUrl },
        ],
        shortcut: "CommandOrControl+`",
      }),
      setRecording: vi.fn(),
      toggleRecording: vi.fn(),
      onRecordingState: vi.fn(),
      setDefaultTextEditor,
      windowControl: vi.fn(),
    } as any;

    render(<App />);
    await user.click(screen.getByRole("button", { name: "Configuration" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Text editor selector" }).textContent).toContain("System default"));
    await user.click(screen.getByRole("button", { name: "Text editor selector" }));
    const systemOption = screen.getByRole("option", { name: "System default" });
    const textEditOption = screen.getByRole("option", { name: "TextEdit" });
    const vsCodeOption = screen.getByRole("option", { name: "Visual Studio Code" });

    const systemIcon = systemOption.querySelector('[data-editor-icon="system"]');
    const textEditIcon = textEditOption.querySelector('[data-editor-icon="textedit"]');
    const vsCodeIcon = vsCodeOption.querySelector('[data-editor-icon="vscode"]');

    expect(systemIcon?.tagName).toBe("IMG");
    expect(textEditIcon?.tagName).toBe("IMG");
    expect(vsCodeIcon?.tagName).toBe("IMG");
    expect(screen.queryByText("Open transcript text in Apple TextEdit")).toBeNull();

    await user.click(textEditOption);

    expect(setDefaultTextEditor).toHaveBeenCalledWith("textedit");
    expect(screen.getByRole("button", { name: "Text editor selector" }).textContent).toContain("TextEdit");
    expect(screen.getByRole("button", { name: "Text editor selector" }).querySelector('[data-editor-icon="textedit"]')?.tagName).toBe("IMG");
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
