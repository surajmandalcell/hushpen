const fs = require("node:fs");
const path = require("node:path");
const { execFile } = require("node:child_process");
const { promisify } = require("node:util");

const execFileAsync = promisify(execFile);
const DEFAULT_TEXT_EDITOR_ID = "system";
const TEXT_EDITOR_OPTIONS = Object.freeze([
  {
    id: "system",
    label: "System default",
    detail: "Use the operating system default editor",
  },
  {
    id: "textedit",
    label: "TextEdit",
    detail: "Open transcript text in Apple TextEdit",
    macApp: "TextEdit",
    macBundleNames: ["TextEdit.app"],
  },
  {
    id: "vscode",
    label: "Visual Studio Code",
    detail: "Open transcript text in VS Code",
    macApp: "Visual Studio Code",
    macBundleNames: ["Visual Studio Code.app"],
  },
  {
    id: "cursor",
    label: "Cursor",
    detail: "Open transcript text in Cursor",
    macApp: "Cursor",
    macBundleNames: ["Cursor.app"],
  },
]);

function getTextEditorOption(editorId) {
  return TEXT_EDITOR_OPTIONS.find((editor) => editor.id === editorId) || TEXT_EDITOR_OPTIONS[0];
}

function createTextEditors({ ctx }) {
  const iconCache = new Map();

  function ensureSystemTextIconProbe() {
    const probePath = path.join(ctx.layout.configDir, "text-editor-icon-probe.txt");
    if (!fs.existsSync(probePath)) {
      fs.writeFileSync(probePath, "", "utf8");
    }
    return probePath;
  }

  function getIconTarget(editor) {
    if (editor.id === DEFAULT_TEXT_EDITOR_ID) {
      return ensureSystemTextIconProbe();
    }

    if (ctx.platform === "darwin" && Array.isArray(editor.macBundleNames)) {
      const appDirectories = [
        "/Applications",
        "/System/Applications",
        path.join(ctx.app.getPath("home"), "Applications"),
      ];

      for (const appDirectory of appDirectories) {
        for (const bundleName of editor.macBundleNames) {
          const bundlePath = path.join(appDirectory, bundleName);
          if (fs.existsSync(bundlePath)) return bundlePath;
        }
      }
    }

    return "";
  }

  async function getIconDataUrl(editor) {
    if (iconCache.has(editor.id)) {
      return iconCache.get(editor.id);
    }

    let iconDataUrl = "";
    try {
      const iconTarget = getIconTarget(editor);
      if (iconTarget) {
        const icon = await ctx.app.getFileIcon(iconTarget, { size: "normal" });
        if (!icon.isEmpty()) {
          iconDataUrl = icon.resize({ width: 32, height: 32 }).toDataURL();
        }
      }
    } catch {
      iconDataUrl = "";
    }

    iconCache.set(editor.id, iconDataUrl);
    return iconDataUrl;
  }

  function list() {
    return Promise.all(TEXT_EDITOR_OPTIONS.map(async (editor) => ({
      id: editor.id,
      label: editor.label,
      detail: editor.detail,
      iconDataUrl: await getIconDataUrl(editor),
    })));
  }

  async function openFile(filePath, editorId = DEFAULT_TEXT_EDITOR_ID) {
    const { shell } = require("electron");
    const editor = getTextEditorOption(editorId);

    if (ctx.platform === "darwin" && editor.macApp) {
      try {
        await execFileAsync("open", ["-a", editor.macApp, filePath]);
        return;
      } catch {
        // Fall back to the system handler when a configured app is not available.
      }
    }

    const openError = await shell.openPath(filePath);
    if (openError) {
      throw new Error(openError);
    }
  }

  return { list, openFile };
}

module.exports = { DEFAULT_TEXT_EDITOR_ID, TEXT_EDITOR_OPTIONS, createTextEditors, getTextEditorOption };
