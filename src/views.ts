import type { ComponentType } from "react";
import {
  History,
  Home,
  Info,
  Library,
  Settings as SettingsIcon,
  Volume2,
  type LucideIcon,
} from "lucide-react";
import { AboutView } from "./features/about/AboutView";
import { HistoryView } from "./features/history/HistoryView";
import { HomeView } from "./features/home/HomeView";
import { ModelsView } from "./features/models/ModelsView";
import { SettingsView } from "./features/settings/SettingsView";
import { SoundView } from "./features/sound/SoundView";
import type { ViewId } from "./types/app";
import type { ViewProps } from "./types/view";

export interface ViewDefinition {
  id: ViewId;
  label: string;
  icon: LucideIcon;
  tone: string;
  component: ComponentType<ViewProps>;
}

export const views: readonly ViewDefinition[] = [
  { id: "home", label: "Home", icon: Home, tone: "bg-[#ff7a32] text-white", component: HomeView },
  { id: "configuration", label: "Configuration", icon: SettingsIcon, tone: "bg-[#727272] text-white", component: SettingsView },
  { id: "sound", label: "Sound", icon: Volume2, tone: "bg-[#737373] text-white", component: SoundView },
  { id: "models", label: "Models library", icon: Library, tone: "bg-[#8f8f8f] text-white", component: ModelsView },
  { id: "history", label: "History", icon: History, tone: "bg-[#7167ff] text-white", component: HistoryView },
  { id: "about", label: "About", icon: Info, tone: "bg-[#727272] text-white", component: AboutView },
];

export function findView(id: ViewId): ViewDefinition {
  return views.find((view) => view.id === id) ?? views[0];
}
