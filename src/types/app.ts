export type ViewId = "home" | "configuration" | "sound" | "models" | "history" | "about";
export type WindowAction = "minimize" | "close";
export type RecordingStatus = "idle" | "starting" | "recording" | "preparing-engine" | "transcribing" | "error";

export interface AppInfo {
  name: string;
  version: string;
}
