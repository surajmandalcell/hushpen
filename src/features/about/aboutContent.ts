import { Bug, FolderOpen, Github, ScrollText, type LucideIcon } from "lucide-react";
import { formatHomeRelativePath } from "../../lib/format";
import type { OpenTarget } from "../../types/app";

const githubRepositoryUrl = "https://github.com/surajmandalcell/asrpro";
const githubIssueUrl = `${githubRepositoryUrl}/issues/new`;

export interface AboutAction {
  icon: LucideIcon;
  label: string;
  detail: string;
  target: OpenTarget;
  /** Shown as the link address; clicks always go through the named target. */
  href?: string;
}

export const aboutActionLinks: AboutAction[] = [
  {
    icon: Github,
    label: "GitHub",
    detail: "View the project",
    target: "repo",
    href: githubRepositoryUrl,
  },
  {
    icon: Bug,
    label: "Report issue",
    detail: "Open a new issue",
    target: "issues",
    href: githubIssueUrl,
  },
];

export const aboutFolderActions: AboutAction[] = [
  {
    icon: FolderOpen,
    label: "Open data folder",
    detail: "Show your local data",
    target: "data-folder",
  },
  {
    icon: ScrollText,
    label: "Open log folder",
    detail: "Show the log files",
    target: "log-folder",
  },
];

export function buildAboutFactRows(appVersion: string, storagePath?: string): Array<{ label: string; value: string }> {
  return [
    { label: "Version", value: appVersion },
    { label: "Recognition", value: "Private dictation and file transcription" },
    { label: "Data folder", value: formatHomeRelativePath(storagePath) || "Waiting for local data folder" },
  ];
}
