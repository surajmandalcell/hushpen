import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";

afterEach(() => {
  vi.restoreAllMocks();
  window.asrpro = undefined;
  cleanup();
});

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

  it("toggles recording state from the dashboard", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy();
    expect(screen.queryByText("Recording now")).toBeNull();
    expect(screen.getByLabelText("Recording active")).toBeTruthy();
  });

  it("syncs recording state from the global shortcut and tray bridge", async () => {
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
