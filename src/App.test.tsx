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
    expect(screen.getByRole("heading", { name: "Ready to Dictate" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start Recording" })).toBeTruthy();
    expect(screen.getAllByText("Local Whisper").length).toBeGreaterThan(0);
  });

  it("keeps the sidebar title to one app-name row without the platform label", async () => {
    const getPlatform = vi.fn().mockResolvedValue({
      platform: "darwin",
      arch: "arm64",
      versions: {},
    });
    window.asrpro = {
      getPlatform,
      getAppInfo: vi.fn(),
      selectAudioFiles: vi.fn(),
      onAddFiles: vi.fn(),
      windowControl: vi.fn(),
    };

    render(<App />);
    await waitFor(() => expect(getPlatform).toHaveBeenCalledOnce());

    const sidebarTitle = screen.getAllByText("ASR Pro")[0].closest("div");
    await waitFor(() => expect(sidebarTitle?.textContent).toBe("ASR Pro"));
    expect(screen.queryByText("macOS")).toBeNull();
  });

  it("navigates to the file transcription workspace", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Files" }));

    expect(screen.getByRole("heading", { name: "Drop audio or video" })).toBeTruthy();
    expect(screen.getByText("Queued transcription jobs")).toBeTruthy();
  });

  it("toggles recording state from the dashboard", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Start Recording" }));

    expect(screen.getByRole("button", { name: "Stop Recording" })).toBeTruthy();
    expect(screen.getAllByText("Recording now").length).toBeGreaterThan(0);
  });
});
