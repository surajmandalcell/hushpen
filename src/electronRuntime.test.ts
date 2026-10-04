import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import https from "node:https";
import { tmpdir } from "node:os";
import path from "node:path";
import { Readable } from "node:stream";
import { afterEach, describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const runtime = require("../electron/runtime.cjs");
const whisperEngine = require("../electron/whisper-engine.cjs");
const constants = require("../electron/core/constants.cjs");

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Electron runtime helpers", () => {
  it("uses CommandOrControl+` as the global recording shortcut", () => {
    expect(runtime.RECORDING_SHORTCUT).toBe("CommandOrControl+`");
  });

  it("keeps the main window fixed-size and removes maximize entry points", () => {
    const windowSource = readFileSync("electron/windows/mainWindow.cjs", "utf8");
    const constantsSource = readFileSync("electron/core/constants.cjs", "utf8");
    const preloadSource = readFileSync("electron/preload.cjs", "utf8");
    const windowIpcSource = readFileSync("electron/ipc/app.cjs", "utf8");
    const menuSource = readFileSync("electron/shell/menu.cjs", "utf8");

    expect(constants.MAIN_WINDOW_SIZE).toEqual({ width: 780, height: 520 });
    expect(constantsSource).toContain('const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";');
    expect(windowSource).toMatch(
      /new BrowserWindow\(\{[\s\S]*?width: MAIN_WINDOW_SIZE\.width,[\s\S]*?height: MAIN_WINDOW_SIZE\.height,[\s\S]*?minWidth: MAIN_WINDOW_SIZE\.width,[\s\S]*?minHeight: MAIN_WINDOW_SIZE\.height,[\s\S]*?maxWidth: MAIN_WINDOW_SIZE\.width,[\s\S]*?maxHeight: MAIN_WINDOW_SIZE\.height,[\s\S]*?resizable: false,[\s\S]*?maximizable: false,[\s\S]*?fullscreenable: false,/
    );
    for (const source of [windowSource, windowIpcSource, menuSource]) {
      expect(source).not.toContain('role: "zoom"');
      expect(source).not.toContain('action === "maximize"');
      expect(source).not.toContain(".maximize()");
    }
    expect(windowSource).toContain("lockMainWindowSize(win)");
    expect(windowSource).toContain("setMinimumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height)");
    expect(windowSource).toContain("setMaximumSize(MAIN_WINDOW_SIZE.width, MAIN_WINDOW_SIZE.height)");
    expect(windowSource).toContain('win.on("will-resize"');
    expect(windowSource).toContain("event.preventDefault()");
    expect(windowSource).toContain('win.on("maximize"');
    expect(windowSource).toContain("win.unmaximize()");
    expect(windowSource).toContain('win.on("enter-full-screen"');
    expect(windowSource).toContain("win.setFullScreen(false)");
    expect(readFileSync("electron/main.cjs", "utf8")).toContain('mode: "screenshot"');
    expect(readFileSync("electron/main.cjs", "utf8")).toContain("if (!SCREENSHOT_MODE)");
    expect(preloadSource).not.toContain('"maximize"');
  });
  it("keeps the startup paint dark with a temporary loader until React mounts", () => {
    const indexSource = readFileSync("index.html", "utf8");
    const rendererEntrySource = readFileSync("src/main.tsx", "utf8");
    const windowSource = readFileSync("electron/windows/mainWindow.cjs", "utf8");

    expect(indexSource).toContain('<html lang="en" class="dark">');
    expect(indexSource).toContain('id="app-loading-state"');
    expect(indexSource).toContain("Loading ASR Pro");
    expect(indexSource).toMatch(/html,\s*body,\s*#root[\s\S]*background:\s*#2f2f2f/);
    expect(rendererEntrySource).toContain('document.getElementById("app-loading-state")');
    expect(rendererEntrySource).toContain("requestAnimationFrame");
    expect(constants.MAIN_WINDOW_BACKGROUND).toBe("#2f2f2f");
    expect(windowSource).toContain("backgroundColor: MAIN_WINDOW_BACKGROUND");
  });
  it("uses Whisper Base English as the default native model", () => {
    expect(runtime.DEFAULT_MODEL.id).toBe("whisper-base-en");
    expect(runtime.DEFAULT_MODEL.displayName).toBe("Whisper Base English");
    expect(runtime.DEFAULT_MODEL.fileName).toBe("ggml-base.en.bin");
    expect(runtime.AVAILABLE_MODELS.map((model: { id: string }) => model.id)).toEqual([
      "whisper-tiny-en",
      "whisper-base-en",
      "whisper-base",
      "whisper-small-en",
      "whisper-large-v3-turbo",
    ]);
    expect(runtime.AVAILABLE_MODELS.find((model: { id: string }) => model.id === "whisper-large-v3-turbo")).toMatchObject({
      displayName: "Whisper Large v3 Turbo",
      fileName: "ggml-large-v3-turbo.bin",
      sizeLabel: "1.5 GiB",
      sha1: "4af2b29d7ec73d781377bfd1758ca957a807e941",
    });
  });

  it("reports model storage and runtime memory grouped for the settings surface", () => {
    const dataDir = mkdtempSync(path.join(tmpdir(), "asrpro-model-stats-"));

    try {
      mkdirSync(path.join(dataDir, "models", "whisper"), { recursive: true });
      mkdirSync(path.join(dataDir, "transcripts"), { recursive: true });
      mkdirSync(path.join(dataDir, "config"), { recursive: true });
      writeFileSync(path.join(dataDir, "models", "whisper", "ggml-base.en.bin"), Buffer.alloc(12), { flag: "w" });
      writeFileSync(path.join(dataDir, "transcripts", "note.txt"), Buffer.alloc(5), { flag: "w" });
      writeFileSync(path.join(dataDir, "config", "settings.json"), Buffer.alloc(3), { flag: "w" });

      const stats = runtime.collectRuntimeStorageStats(dataDir, {
        rss: 100,
        heapUsed: 40,
        external: 20,
        arrayBuffers: 10,
      }, "whisper-base-en");

      expect(stats.groups.map((group: { id: string }) => group.id)).toEqual(["memory", "disk"]);
      expect(stats.groups[0].items.map((item: { id: string; bytes: number }) => [item.id, item.bytes])).toEqual([
        ["resident", 100],
        ["heap", 40],
        ["native", 20],
        ["model-memory", 12],
      ]);
      expect(stats.groups[0].items.map((item: { label: string; detail?: string }) => [item.label, item.detail])).toContainEqual([
        "AI model memory",
        "Whisper Base English active model estimate",
      ]);
      expect(stats.groups[1].items.map((item: { id: string; bytes: number }) => [item.id, item.bytes])).toEqual([
        ["whisper-models", 12],
        ["transcripts", 5],
        ["configuration", 3],
        ["other", 0],
      ]);
    } finally {
      rmSync(dataDir, { recursive: true, force: true });
    }
  });

  it("normalizes native Whisper segment arrays without timestamp metadata", () => {
    expect(whisperEngine.normalizeTranscriptionResult({
      transcription: [
        ["00:-16:-47,-260", "00:00:30,000", "Hello, I am not talking about the T3."],
        ["00:00:30,000", "00:00:40,000", "This is the actual next sentence."],
      ],
    })).toBe("Hello, I am not talking about the T3. This is the actual next sentence.");
  });

  it("uses Electron IPC for native Node Whisper transcription", () => {
    const mainSource = readFileSync("electron/main.cjs", "utf8");
    const engineIpcSource = readFileSync("electron/ipc/engine.cjs", "utf8");
    const channels = JSON.parse(readFileSync("shared/ipc-channels.json", "utf8")).channels;

    expect(engineIpcSource).toContain('router.handle("engine:transcribe-audio"');
    expect(channels["engine:transcribe-audio"].kind).toBe("invoke");
    expect(mainSource.indexOf("mainWindow.create();")).toBeLessThan(mainSource.indexOf("registerGlobalShortcut({ actions })"));
  });
  it("shares one in-flight file download when the same model is requested twice", async () => {
    const dataDir = mkdtempSync(path.join(tmpdir(), "asrpro-duplicate-model-download-"));
    const model = whisperEngine.AVAILABLE_MODELS.find((candidate: { id: string }) => candidate.id === "whisper-tiny-en");
    const originalSha1 = model.sha1;
    const payload = Buffer.from("tiny model fixture");
    model.sha1 = createHash("sha1").update(payload).digest("hex");
    let requestCount = 0;

    vi.spyOn(https, "get").mockImplementation(((_url: string | URL, callback: (response: Readable) => void) => {
      requestCount += 1;
      const request = new EventEmitter();
      const response = new Readable({
        read() {},
      }) as Readable & {
        statusCode?: number;
        headers: Record<string, string>;
      };
      response.statusCode = 200;
      response.headers = { "content-length": String(payload.length) };

      setTimeout(() => {
        callback(response);
        setTimeout(() => {
          response.push(payload);
          response.push(null);
        }, 5);
      }, 0);

      return request;
    }) as typeof https.get);

    try {
      const [first, second] = await Promise.all([
        whisperEngine.downloadModelFile({ modelId: model.id, dataDir }),
        whisperEngine.downloadModelFile({ modelId: model.id, dataDir }),
      ]);

      expect(first.path).toBe(second.path);
      expect(requestCount).toBe(1);
      expect(readFileSync(whisperEngine.getModelPath(dataDir, model.id))).toEqual(payload);
    } finally {
      model.sha1 = originalSha1;
      rmSync(dataDir, { recursive: true, force: true });
    }
  });

  it("opens history transcript text through Electron IPC", () => {
    const transcriptSource = readFileSync("electron/ipc/transcripts.cjs", "utf8");
    const editorSource = readFileSync("electron/shell/textEditors.cjs", "utf8");
    const channels = JSON.parse(readFileSync("shared/ipc-channels.json", "utf8")).channels;

    expect(transcriptSource).toContain('router.handle("transcript:open-text"');
    expect(transcriptSource).toContain('textEditors.openFile(filePath, ctx.settings.get("editor.defaultTextEditor"))');
    expect(transcriptSource).toContain('router.handle("transcript:delete-text"');
    expect(editorSource).toContain("app.getFileIcon");
    expect(editorSource).toContain("iconDataUrl");
    expect(editorSource).toContain("macBundleNames");
    expect(channels["transcript:open-text"].kind).toBe("invoke");
    expect(channels["transcript:delete-text"].kind).toBe("invoke");
  });
  it("exposes a persisted app setting for automatic transcript clipboard copying", () => {
    const defaults = JSON.parse(readFileSync("shared/settings-defaults.json", "utf8"));
    const runtimeStateSource = readFileSync("electron/ipc/runtimeState.cjs", "utf8");

    expect(defaults["output.autoCopy"]).toBe(true);
    expect(runtimeStateSource).toContain('autoCopyTranscripts: values["output.autoCopy"]');
  });
  it("exposes startup launch settings through Electron IPC", () => {
    const defaults = JSON.parse(readFileSync("shared/settings-defaults.json", "utf8"));
    const startupSource = readFileSync("electron/shell/startup.cjs", "utf8");
    const settingsIpcSource = readFileSync("electron/ipc/settings.cjs", "utf8");

    expect(defaults["startup.launchAtLogin"]).toBe(false);
    expect(settingsIpcSource).toContain('"startup.launchAtLogin"');
    expect(startupSource).toContain("app.setLoginItemSettings");
    expect(startupSource).toContain("buildLinuxAutostartDesktopEntry");
    expect(startupSource).toContain("PORTABLE_EXECUTABLE_FILE");
  });
  it("links the portable data and setup docs from the README", () => {
    const readme = readFileSync("README.md", "utf8");
    const portableDocs = readFileSync("docs/portable-data.md", "utf8");
    const gettingStartedDocs = readFileSync("docs/getting-started.md", "utf8");
    const startupDocs = readFileSync("docs/startup.md", "utf8");

    expect(readme).toContain("docs/portable-data.md");
    expect(readme).toContain("docs/getting-started.md");
    expect(readme).toContain("docs/startup.md");
    expect(portableDocs).toContain("asrpro-data/");
    expect(portableDocs).toContain("history");
    expect(portableDocs).toContain("models");
    expect(portableDocs).toContain("config");
    expect(gettingStartedDocs).toContain("pick a microphone");
    expect(gettingStartedDocs).toContain("Launch at startup");
    expect(startupDocs).toContain("Launch at startup");
    expect(startupDocs).toContain("replaces the saved sign-in target");
  });

  it("renders the recording overlay as a text-free waveform pill", () => {
    const html = runtime.createRecordingOverlayHtml();

    expect(runtime.OVERLAY_WINDOW_SIZE).toEqual({ width: 156, height: 40 });
    expect(html).toContain('class="surface"');
    expect(html).toContain('class="waveform"');
    expect(html).toContain("width: 148px");
    expect(html).toContain("width: 132px");
    expect(html.match(/data-base=/g)?.length).toBe(44);
    expect(html).toContain("now - lastInputAt > 260");
    expect(html).toContain("desired > current[index] ? 0.34 : 0.2");
    expect(html).toContain("Math.sin(now * 0.018");
    expect(html).toContain("rgba(18, 18, 20, 0.94)");
    expect(html).toContain("rgba(230, 230, 234, var(--bar-opacity))");
    expect(html).toContain("asrproSetWaveformFrame");
    expect(html).toContain("asrproOverlay?.onWaveformFrame");
    expect(html).toContain("requestAnimationFrame(renderWaveform)");
    expect(html).not.toContain("rgba(245, 245, 247");
    expect(html).not.toContain("rgba(255, 255, 255");
    expect(html).not.toContain("executeJavaScript");
    expect(html).not.toContain("transition: height");
    expect(html).not.toContain("animation:");
    expect(html).not.toContain("Recording now");
    expect(html).not.toContain("CommandOrControl");
  });

  it("shows the global recording overlay for every recording start source", () => {
    expect(runtime.shouldShowRecordingOverlay("renderer")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("shortcut")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("tray")).toBe(true);
    expect(runtime.shouldShowRecordingOverlay("menu")).toBe(true);
  });

  it("uses standard app data for packaged macOS apps in Applications folders", () => {
    const dataDir = runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "darwin",
      resourcesPath: "/Applications/ASR Pro.app/Contents/Resources",
      exePath: "/Applications/ASR Pro.app/Contents/MacOS/ASR Pro",
      appPath: "/project",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
    });

    expect(dataDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data");

    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "darwin",
      resourcesPath: "/Volumes/External1TB/Applications/ASR Pro.app/Contents/Resources",
      exePath: "/Volumes/External1TB/Applications/ASR Pro.app/Contents/MacOS/ASR Pro",
      appPath: "/project",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
    })).toBe("/Users/suraj/Library/Application Support/ASR Pro/data");
  });

  it("keeps packaged Windows and Linux data beside the executable", () => {
    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "win32",
      resourcesPath: "D:\\Tools\\ASR Pro\\resources",
      exePath: "D:\\Tools\\ASR Pro\\ASR Pro.exe",
      appPath: "C:\\repo",
      userDataPath: "C:\\Users\\suraj\\AppData\\Roaming\\ASR Pro",
    })).toBe("D:\\Tools\\ASR Pro\\asrpro-data");

    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "linux",
      resourcesPath: "/mnt/tools/asrpro/resources",
      exePath: "/mnt/tools/asrpro/asrpro",
      appPath: "/repo",
      userDataPath: "/home/suraj/.config/ASR Pro",
    })).toBe("/mnt/tools/asrpro/asrpro-data");
  });

  it("uses the electron-builder portable executable folder on Windows when present", () => {
    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "win32",
      resourcesPath: "C:\\Users\\suraj\\AppData\\Local\\Temp\\asrpro-portable\\resources",
      exePath: "C:\\Users\\suraj\\AppData\\Local\\Temp\\asrpro-portable\\ASR Pro.exe",
      appPath: "C:\\repo",
      userDataPath: "C:\\Users\\suraj\\AppData\\Roaming\\ASR Pro",
      portableExecutableDir: "E:\\Tools\\ASR Pro",
    })).toBe("E:\\Tools\\ASR Pro\\asrpro-data");
  });

  it("keeps packaged macOS data beside the app when outside Applications folders", () => {
    expect(runtime.resolveContainedDataDir({
      isPackaged: true,
      platform: "darwin",
      resourcesPath: "/Users/suraj/Downloads/ASR Pro.app/Contents/Resources",
      exePath: "/Users/suraj/Downloads/ASR Pro.app/Contents/MacOS/ASR Pro",
      appPath: "/project",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
    })).toBe("/Users/suraj/Downloads/asrpro-data");
  });

  it("uses an explicit data directory override for isolated automation runs", () => {
    const dataDir = runtime.resolveContainedDataDir({
      isPackaged: false,
      platform: "darwin",
      resourcesPath: "/repo/resources",
      exePath: "/repo/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron",
      appPath: "/repo",
      userDataPath: "/Users/suraj/Library/Application Support/ASR Pro",
      dataDirOverride: "/private/tmp/asrpro-readme-capture-1234",
    });

    expect(dataDir).toBe("/private/tmp/asrpro-readme-capture-1234");
  });

  it("resolves Whisper model cache paths under app data", () => {
    const paths = runtime.buildModelPaths("/Users/suraj/Library/Application Support/ASR Pro/data");

    expect(paths.modelsDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models");
    expect(paths.whisperModelsDir).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models/whisper");
    expect(paths.defaultModelPath).toBe("/Users/suraj/Library/Application Support/ASR Pro/data/models/whisper/ggml-base.en.bin");
  });

  it("builds a Linux autostart entry for the current executable", () => {
    expect(runtime.buildLinuxAutostartDesktopEntry({
      appName: "ASR Pro",
      executablePath: "/mnt/tools/asrpro/asrpro",
    })).toContain('Exec="/mnt/tools/asrpro/asrpro"');
  });

  it("resolves platform app and tray icon assets", () => {
    const assetRoot = "/repo/src/assets";

    expect(runtime.resolveTrayIconPath("darwin", assetRoot)).toBe(path.join(assetRoot, "asrpro-tray-dark.png"));
    expect(runtime.resolveTrayIconPath("win32", assetRoot, false)).toBe(path.join(assetRoot, "asrpro-tray-dark.ico"));
    expect(runtime.resolveTrayIconPath("win32", assetRoot, true)).toBe(path.join(assetRoot, "asrpro-tray-light.ico"));
    expect(runtime.resolveTrayIconPath("linux", assetRoot, false)).toBe(path.join(assetRoot, "asrpro-tray-dark.png"));
    expect(runtime.resolveTrayIconPath("linux", assetRoot, true)).toBe(path.join(assetRoot, "asrpro-tray-light.png"));
    expect(runtime.resolveAppIconPath("darwin", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.png"));
    expect(runtime.resolveAppIconPath("win32", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.ico"));
    expect(runtime.resolveAppIconPath("linux", assetRoot)).toBe(path.join(assetRoot, "asrpro-app-icon.png"));
  });

  it("sets the macOS development Dock icon from runtime assets", () => {
    const mainSource = readFileSync("electron/main.cjs", "utf8");

    expect(mainSource).toContain("setMacDockIcon();");
    expect(mainSource).toContain("function setMacDockIcon()");
    expect(mainSource).toContain('process.platform !== "darwin"');
    expect(mainSource).toContain("app.dock.setIcon(");
  });
  it("resolves runtime asset roots for development and packaged apps", () => {
    expect(runtime.resolveRuntimeAssetRoot({
      isPackaged: false,
      resourcesPath: "/repo/resources",
      appPath: "/repo",
    })).toBe(path.join("/repo", "src", "assets"));

    expect(runtime.resolveRuntimeAssetRoot({
      isPackaged: true,
      resourcesPath: "/Applications/ASR Pro.app/Contents/Resources",
      appPath: "/repo",
    })).toBe(path.join("/Applications/ASR Pro.app/Contents/Resources", "assets"));
  });

  it("places the recording overlay at the bottom of the selected work area", () => {
    const bounds = runtime.resolveOverlayBounds({
      settings: { placement: "bottom" },
      primaryDisplay: {
        id: 1,
        workArea: { x: 10, y: 30, width: 1400, height: 820 },
      },
      displays: [],
      width: 420,
      height: 84,
    });

    expect(bounds).toEqual({
      x: 500,
      y: 750,
    });
  });

  it("remembers a dragged overlay position on the matching monitor", () => {
    const bounds = runtime.resolveOverlayBounds({
      settings: {
        placement: "top",
        customBounds: {
          displayId: 7,
          x: 2030,
          y: 620,
        },
      },
      primaryDisplay: {
        id: 1,
        workArea: { x: 0, y: 25, width: 1440, height: 850 },
      },
      displays: [
        { id: 1, workArea: { x: 0, y: 25, width: 1440, height: 850 } },
        { id: 7, workArea: { x: 1440, y: 0, width: 1728, height: 1117 } },
      ],
      width: 420,
      height: 84,
    });

    expect(bounds).toEqual({
      x: 2030,
      y: 620,
    });
  });
});
