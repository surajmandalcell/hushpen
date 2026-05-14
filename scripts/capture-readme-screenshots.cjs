const fs = require("node:fs");
const http = require("node:http");
const net = require("node:net");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { _electron } = require("playwright-core");
const electronPath = require("electron");

const expectedSize = { width: 780, height: 520 };
const screenshotViews = [
  { name: "Home", file: "asrpro-home.png", evidenceFile: "concept1.jpg", button: "Home" },
  { name: "History", file: "asrpro-history.png", evidenceFile: "concept2.jpg", button: "History" },
  { name: "Models", file: "asrpro-models.png", evidenceFile: "concept3.jpg", button: "Models library" },
  { name: "About", file: "asrpro-about.png", evidenceFile: "concept4.jpg", button: "About" },
];

const seededHistoryRows = [
  {
    id: "readme-history-1",
    title: "Product demo follow-up",
    text: "Summarize the product demo, send the follow-up notes, and schedule the model comparison review.",
    kind: "Dictation",
    model: "Local Whisper",
    durationSeconds: 58,
    createdAt: Date.parse("2026-05-15T09:30:00+05:30"),
    status: "completed",
  },
  {
    id: "readme-history-2",
    title: "Roadmap voice note",
    text: "Keep the desktop release private first, tighten screenshot checks, and verify the packaged runtime before sharing.",
    kind: "Dictation",
    model: "Local Whisper",
    durationSeconds: 72,
    createdAt: Date.parse("2026-05-15T08:45:00+05:30"),
    status: "completed",
  },
  {
    id: "readme-history-3",
    title: "Audio file transcript",
    text: "The imported audio sample should stay in history with model details and a replayable local recording.",
    kind: "File",
    model: "Local Whisper",
    durationSeconds: 94,
    createdAt: Date.parse("2026-05-14T16:20:00+05:30"),
    status: "completed",
  },
];

function readImageSize(filePath) {
  const buffer = fs.readFileSync(filePath);
  if (buffer.subarray(0, 8).toString("hex") === "89504e470d0a1a0a") {
    return {
      width: buffer.readUInt32BE(16),
      height: buffer.readUInt32BE(20),
    };
  }

  if (buffer[0] === 0xff && buffer[1] === 0xd8) {
    return readJpegSize(filePath, buffer);
  }

  throw new Error(`${filePath} is not a supported PNG or JPEG file`);
}

function readJpegSize(filePath, buffer) {
  let offset = 2;
  while (offset < buffer.length) {
    while (buffer[offset] === 0xff) offset += 1;
    const marker = buffer[offset];
    offset += 1;

    if (marker === 0xd9 || marker === 0xda) break;
    if (marker >= 0xd0 && marker <= 0xd7) continue;

    const length = buffer.readUInt16BE(offset);
    const isStartOfFrame = marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker);
    if (isStartOfFrame) {
      return {
        height: buffer.readUInt16BE(offset + 3),
        width: buffer.readUInt16BE(offset + 5),
      };
    }
    offset += length;
  }

  throw new Error(`${filePath} does not contain a JPEG size marker`);
}

function getViteExecutable(repoRoot) {
  return path.join(repoRoot, "node_modules", ".bin", process.platform === "win32" ? "vite.cmd" : "vite");
}

function isPortAvailable(port) {
  return new Promise((resolve) => {
    const server = net.createServer()
      .once("error", () => resolve(false))
      .once("listening", () => {
        server.close(() => resolve(true));
      })
      .listen(port, "127.0.0.1");
  });
}

async function findAvailablePort(startPort) {
  for (let port = startPort; port < startPort + 20; port += 1) {
    if (await isPortAvailable(port)) return port;
  }

  throw new Error(`No available local port found from ${startPort} to ${startPort + 19}`);
}

function waitForHttp(url, processHandle, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;

  return new Promise((resolve, reject) => {
    const check = () => {
      if (processHandle.exitCode !== null) {
        reject(new Error(`Vite exited before ${url} became available`));
        return;
      }

      const request = http.get(url, (response) => {
        response.resume();
        if (response.statusCode && response.statusCode >= 200 && response.statusCode < 500) {
          resolve();
          return;
        }
        retry();
      });

      request.setTimeout(800, () => {
        request.destroy();
        retry();
      });
      request.on("error", retry);
    };

    const retry = () => {
      if (Date.now() >= deadline) {
        reject(new Error(`Timed out waiting for ${url}`));
        return;
      }
      setTimeout(check, 250);
    };

    check();
  });
}

async function waitForActiveNav(page, label) {
  const navButton = page.getByRole("button", { name: label, exact: true });
  await navButton.waitFor({ state: "visible" });
  await page.waitForFunction((buttonLabel) => {
    const buttons = Array.from(document.querySelectorAll("button[aria-label]"));
    const button = buttons.find((item) => item.getAttribute("aria-label") === buttonLabel);
    return button?.getAttribute("aria-current") === "page";
  }, label);
}

function assertImageSize(filePath, label) {
  const size = readImageSize(filePath);
  if (size.width !== expectedSize.width || size.height !== expectedSize.height) {
    throw new Error(`${label} captured at ${size.width}x${size.height}, expected ${expectedSize.width}x${expectedSize.height}`);
  }
  console.log(`${label}: ${size.width}x${size.height}`);
}

async function captureView(page, view, screenshotDir, evidenceDir) {
  await page.getByRole("button", { name: view.button, exact: true }).click();
  await waitForActiveNav(page, view.button);
  await page.waitForTimeout(180);

  const outputPath = path.join(screenshotDir, view.file);
  const evidencePath = path.join(evidenceDir, view.evidenceFile);
  await page.screenshot({ path: outputPath, fullPage: false, animations: "disabled" });
  await page.screenshot({
    path: evidencePath,
    type: "jpeg",
    quality: 92,
    fullPage: false,
    animations: "disabled",
  });

  assertImageSize(outputPath, view.file);
  assertImageSize(evidencePath, view.evidenceFile);
}

async function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const screenshotDir = path.join(repoRoot, "docs", "screenshots");
  const evidenceDir = path.join(repoRoot, "_evidence");
  fs.mkdirSync(screenshotDir, { recursive: true });
  fs.mkdirSync(evidenceDir, { recursive: true });

  const port = await findAvailablePort(4270);
  const devUrl = `http://127.0.0.1:${port}`;
  const vite = spawn(getViteExecutable(repoRoot), ["--host", "127.0.0.1", "--port", String(port), "--strictPort"], {
    cwd: repoRoot,
    env: { ...process.env, BROWSER: "none" },
    stdio: ["ignore", "pipe", "pipe"],
  });

  let viteOutput = "";
  vite.stdout.on("data", (chunk) => {
    viteOutput += chunk.toString();
  });
  vite.stderr.on("data", (chunk) => {
    viteOutput += chunk.toString();
  });

  let electronApp;
  try {
    await waitForHttp(devUrl, vite);

    electronApp = await _electron.launch({
      executablePath: electronPath,
      args: [repoRoot],
      cwd: repoRoot,
      env: {
        ...process.env,
        ASRPRO_SCREENSHOT_MODE: "1",
        VITE_DEV_SERVER_URL: devUrl,
      },
    });

    const page = await electronApp.firstWindow();
    const consoleIssues = [];
    page.on("console", (message) => {
      if (["error", "warning"].includes(message.type())) {
        consoleIssues.push(`${message.type()}: ${message.text()}`);
      }
    });
    page.on("pageerror", (error) => {
      consoleIssues.push(`pageerror: ${error.message}`);
    });

    await page.waitForLoadState("domcontentloaded");
    await page.getByRole("button", { name: "Home", exact: true }).waitFor({ state: "visible" });
    await page.evaluate((rows) => {
      window.localStorage.setItem("asrpro.transcriptHistory.v1", JSON.stringify(rows));
      window.localStorage.setItem("asrpro.audioInputDevice.v1", "default");
    }, seededHistoryRows);
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.getByRole("button", { name: "Home", exact: true }).waitFor({ state: "visible" });

    const browserWindow = await electronApp.browserWindow(page);
    const windowState = await browserWindow.evaluate((win) => ({
      size: win.getSize(),
      contentSize: win.getContentSize(),
      resizable: win.isResizable(),
      maximizable: win.isMaximizable(),
      fullscreenable: win.isFullScreenable(),
    }));

    if (
      windowState.contentSize[0] !== expectedSize.width
      || windowState.contentSize[1] !== expectedSize.height
      || windowState.resizable
      || windowState.maximizable
      || windowState.fullscreenable
    ) {
      throw new Error(`Unexpected Electron window state: ${JSON.stringify(windowState)}`);
    }

    for (const view of screenshotViews) {
      await captureView(page, view, screenshotDir, evidenceDir);
    }

    const relevantConsoleIssues = consoleIssues.filter((issue) => !issue.includes("Electron Security Warning"));
    if (relevantConsoleIssues.length) {
      throw new Error(`Console issues during screenshot capture:\n${relevantConsoleIssues.join("\n")}`);
    }
  } catch (error) {
    if (viteOutput.trim()) {
      console.error(viteOutput.trim());
    }
    throw error;
  } finally {
    if (electronApp) {
      await electronApp.evaluate(({ app }) => app.quit()).catch(() => {});
      await electronApp.close().catch(() => {});
    }
    vite.kill();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
