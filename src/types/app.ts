import type { LucideIcon } from "lucide-react";

export type ViewId = "home" | "configuration" | "sound" | "models" | "history" | "about";
export type WindowAction = "minimize" | "close";
export type RecordingStatus = "idle" | "starting" | "recording" | "preparing-engine" | "transcribing" | "error";

export interface NavItem {
  id: ViewId;
  label: string;
  icon: LucideIcon;
}

export interface AppInfo {
  name: string;
  version: string;
}
