export interface TranscriptHistoryRow {
  id: string;
  title: string;
  text: string;
  kind: "Dictation" | "File";
  model: string;
  durationSeconds: number;
  createdAt: number;
  status: "completed" | "failed";
  recordingUrl?: string;
  transcriptFilePath?: string;
  error?: string;
}
