import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const { lockMainWindowSize } = require("../../electron/windows/mainWindow.cjs") as {
  lockMainWindowSize: (win: unknown, platform?: string) => void;
};

function fakeWindow(size: [number, number] = [780, 520]) {
  const listeners = new Map<string, (event: { preventDefault: () => void }) => void>();
  const state = { resizable: true, maximizable: true, fullScreenable: true, size, min: [0, 0], max: [0, 0] };
  const win = {
    setMinimumSize: vi.fn((width: number, height: number) => { state.min = [width, height]; }),
    setMaximumSize: vi.fn((width: number, height: number) => { state.max = [width, height]; }),
    setResizable: vi.fn((value: boolean) => { state.resizable = value; }),
    setMaximizable: vi.fn((value: boolean) => { state.maximizable = value; }),
    setFullScreenable: vi.fn((value: boolean) => { state.fullScreenable = value; }),
    setFullScreen: vi.fn(),
    setSize: vi.fn((width: number, height: number) => { state.size = [width, height]; }),
    getSize: () => state.size,
    unmaximize: vi.fn(),
    on: (name: string, listener: (event: { preventDefault: () => void }) => void) => listeners.set(name, listener),
  };
  return { win, state, emit: (name: string) => {
    const event = { preventDefault: vi.fn() };
    listeners.get(name)?.(event);
    return event;
  } };
}

describe("main window flags (D-15)", () => {
  it("is not resizable, maximizable, or fullscreenable, and fixed at 780 x 520", () => {
    const { win, state } = fakeWindow();
    lockMainWindowSize(win);

    expect(state.resizable).toBe(false);
    expect(state.maximizable).toBe(false);
    expect(state.fullScreenable).toBe(false);
    expect(state.min).toEqual([780, 520]);
    expect(state.max).toEqual([780, 520]);
  });

  it("reports not maximizable on Linux even though Electron always answers true there", () => {
    const { win } = fakeWindow();
    (win as Record<string, unknown>).isMaximizable = () => true;
    lockMainWindowSize(win, "linux");

    expect((win as unknown as { isMaximizable: () => boolean }).isMaximizable()).toBe(false);
  });

  it("restores the fixed size when a platform resizes, maximizes, or fullscreens anyway", () => {
    const { win, state, emit } = fakeWindow();
    lockMainWindowSize(win);

    expect(emit("will-resize").preventDefault).toHaveBeenCalled();

    state.size = [1200, 900];
    emit("resize");
    expect(state.size).toEqual([780, 520]);

    emit("maximize");
    expect(win.unmaximize).toHaveBeenCalled();

    emit("enter-full-screen");
    expect(win.setFullScreen).toHaveBeenCalledWith(false);
  });

  it("creates the BrowserWindow with the three flags set to false", () => {
    const source = readFileSync("electron/windows/mainWindow.cjs", "utf8");
    const options = source.slice(source.indexOf("new BrowserWindow({"), source.indexOf("webPreferences"));

    expect(options).toMatch(/resizable: false/);
    expect(options).toMatch(/maximizable: false/);
    expect(options).toMatch(/fullscreenable: false/);
  });
});
