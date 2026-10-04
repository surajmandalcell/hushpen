const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:4270";
const MAIN_WINDOW_SIZE = Object.freeze({ width: 780, height: 520 });
const MAIN_WINDOW_BACKGROUND = "#2f2f2f";
const SCREENSHOT_MODE = process.env.ASRPRO_SCREENSHOT_MODE === "1";

module.exports = { DEV_SERVER_URL, MAIN_WINDOW_BACKGROUND, MAIN_WINDOW_SIZE, SCREENSHOT_MODE };
