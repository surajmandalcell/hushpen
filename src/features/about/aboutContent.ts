import { Bug, Github, type LucideIcon } from "lucide-react";
import { formatHomeRelativePath } from "../../lib/format";

const githubRepositoryUrl = "https://github.com/surajmandalcell/asrpro";
const githubIssueUrl = `${githubRepositoryUrl}/issues/new`;

export const aboutActionLinks: Array<{ icon: LucideIcon; label: string; detail: string; href: string }> = [
  {
    icon: Github,
    label: "GitHub",
    detail: "View the project",
    href: githubRepositoryUrl,
  },
  {
    icon: Bug,
    label: "Report issue",
    detail: "Open a new issue",
    href: githubIssueUrl,
  },
];

export function buildAboutFactRows(appVersion: string, storagePath?: string): Array<{ label: string; value: string }> {
  return [
    { label: "Version", value: appVersion },
    { label: "Recognition", value: "Private dictation and file transcription" },
    { label: "Data folder", value: formatHomeRelativePath(storagePath) || "Waiting for local data folder" },
  ];
}
