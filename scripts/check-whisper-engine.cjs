#!/usr/bin/env node

try {
  require("./prepare-whisper-addon.cjs");
  const { loadAddon } = require("../electron/whisper-engine.cjs");
  const whisper = loadAddon();
  if (typeof whisper.transcribe !== "function") {
    throw new Error("Expected @kutalia/whisper-node-addon to export transcribe().");
  }
  console.log("Native Whisper engine addon is available.");
} catch (error) {
  console.error("Native Whisper engine addon is not available.");
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
