export function getErrorMessage(error: unknown) {
  const rawMessage = error instanceof Error ? error.message : "Recording failed";
  const message = rawMessage
    .replace(/^Error invoking remote method '[^']+':\s*(?:Error:\s*)?/i, "")
    .trim();

  if (/Model download failed|checksum mismatch|\.download|ENOENT.*models[\\/]+whisper|rename .*ggml-/i.test(message)) {
    return "Whisper model download failed. Check your connection and try again.";
  }

  if (/No handler registered/i.test(rawMessage)) {
    return "Native Whisper engine needs restart. Restart ASR Pro, then try again.";
  }

  if (/native Whisper addon|whisper\.node|libwhisper/i.test(message)) {
    return "Native Whisper engine could not load. Reinstall dependencies, then restart ASR Pro.";
  }

  if (/failed to fetch|load failed|networkerror|network request failed/i.test(message)) {
    return "Failed to load.";
  }

  return message;
}

export function getRecordingErrorTitle(message: string) {
  if (/needs restart/i.test(message)) return "Engine needs restart";
  if (/Engine unavailable|Failed to load|download failed/i.test(message)) return "Engine unavailable";
  return "Recording failed";
}
