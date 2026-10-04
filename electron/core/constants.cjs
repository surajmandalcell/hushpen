const path = require("node:path");

const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";
const MAIN_WINDOW_SIZE = Object.freeze({ width: 780, height: 520 });
const DIST_INDEX_PATH = path.join(__dirname, "..", "..", "dist", "index.html");
const MAIN_WINDOW_BACKGROUND = "#2f2f2f";
const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";

module.exports = { DEV_SERVER_URL, DIST_INDEX_PATH, MAIN_WINDOW_BACKGROUND, MAIN_WINDOW_SIZE, SCREENSHOT_MODE };
