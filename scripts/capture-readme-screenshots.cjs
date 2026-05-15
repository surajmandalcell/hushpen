const fs = require("node:fs");
const http = require("node:http");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { _electron } = require("playwright-core");
const electronPath = require("electron");

const expectedSize = { width: 780, height: 520 };
const screenshotViews = [
  { name: "Home", file: "asrpro-home.png", evidenceFile: "asrpro_home.jpg", button: "Home" },
  { name: "Configuration", file: "asrpro-configuration.png", evidenceFile: "asrpro_configuration.jpg", button: "Configuration" },
  { name: "Sound", file: "asrpro-sound.png", evidenceFile: "asrpro_sound.jpg", button: "Sound" },
  { name: "Models library", file: "asrpro-models.png", evidenceFile: "asrpro_models_library.jpg", button: "Models library" },
  { name: "History", file: "asrpro-history.png", evidenceFile: "asrpro_history.jpg", button: "History" },
  { name: "About", file: "asrpro-about.png", evidenceFile: "asrpro_about.jpg", button: "About" },
];
const screenshotWindowRadius = 12;
const screenshotMatteColor = "#1f1f1f";

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
  await captureRoundedScreenshot(page, {
    path: outputPath,
    type: "png",
    matte: "transparent",
    omitBackground: true,
  });
  await captureRoundedScreenshot(page, {
    path: evidencePath,
    type: "jpeg",
    quality: 92,
    matte: screenshotMatteColor,
  });

  assertImageSize(outputPath, view.file);
  assertImageSize(evidencePath, view.evidenceFile);
}

async function captureRoundedScreenshot(page, options) {
  await prepareRoundedScreenshotClip(page, options.matte);
  await page.screenshot({
    path: options.path,
    type: options.type,
    quality: options.quality,
    omitBackground: options.omitBackground,
    fullPage: false,
    animations: "disabled",
  });
}

async function prepareRoundedScreenshotClip(page, matte) {
  await page.evaluate(({ matteColor, radius }) => {
    let style = document.getElementById("asrpro-rounded-screenshot-style");
    if (!style) {
      style = document.createElement("style");
      style.id = "asrpro-rounded-screenshot-style";
      style.textContent = `
        html,
        body {
          margin: 0 !important;
          background: var(--asrpro-screenshot-matte, transparent) !important;
        }

        body {
          overflow: hidden !important;
        }

        #root {
          width: 100vw !important;
          height: 100vh !important;
          overflow: hidden !important;
          border-radius: var(--asrpro-screenshot-radius, 12px) !important;
          background: transparent !important;
        }

        #root > .app-chrome {
          overflow: hidden !important;
          border-radius: inherit !important;
        }
      `;
      document.head.appendChild(style);
    }

    document.documentElement.style.setProperty("--asrpro-screenshot-matte", matteColor);
    document.documentElement.style.setProperty("--asrpro-screenshot-radius", `${radius}px`);
  }, { matteColor: matte, radius: screenshotWindowRadius });
}

async function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const screenshotDir = path.join(repoRoot, "docs", "screenshots");
  const evidenceDir = path.join(repoRoot, "_evidence");
  const captureDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "asrpro-readme-capture-"));
  fs.mkdirSync(screenshotDir, { recursive: true });
  fs.rmSync(evidenceDir, { recursive: true, force: true });
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
        ASRPRO_DATA_DIR: captureDataDir,
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
    await page.evaluate(() => {
      window.localStorage.setItem("asrpro.audioInputDevice.v1", "default");
    });
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
    fs.rmSync(captureDataDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
