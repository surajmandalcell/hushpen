const historyDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});

export function formatDuration(seconds: number) {
  const rounded = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(rounded / 60);
  const remainingSeconds = rounded % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}

export function formatByteCount(bytes?: number) {
  const value = Number(bytes);
  if (!Number.isFinite(value) || value <= 0) return "0 B";

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let scaled = value;
  let unitIndex = 0;

  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  const precision = scaled >= 10 || unitIndex === 0 ? 0 : 1;
  return `${scaled.toFixed(precision)} ${units[unitIndex]}`;
}

export function formatHistoryGroupLabel(createdAt: number, now = Date.now()) {
  const elapsedDays = Math.max(0, Math.floor((startOfDay(now) - startOfDay(createdAt)) / 86_400_000));
  if (elapsedDays === 0) return "Today";
  if (elapsedDays === 1) return "Yesterday";
  if (elapsedDays < 30) return `${elapsedDays} days ago`;
  return historyDateFormatter.format(new Date(createdAt));
}

export function countWords(text: string) {
  return text.trim().split(/\s+/).filter(Boolean).length;
}

export function formatHomeRelativePath(filePath?: string) {
  if (!filePath) return undefined;
  return filePath.replace(/^\/(?:Users|home)\/[^/]+(?=\/|$)/, "~");
}

function startOfDay(timestamp: number) {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function formatEngineStatus(status?: string) {
  if (status === "ready") return "Ready";
  if (status === "starting") return "Starting";
  if (status === "downloading") return "Downloading";
  if (status === "transcribing") return "Transcribing";
  if (status === "idle") return "Idle";
  if (status === "failed") return "Failed";
  if (status === "stopped") return "Stopped";
  return "Unknown";
}
