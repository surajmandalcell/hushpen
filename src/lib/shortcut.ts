export function formatShortcutParts(shortcut?: string) {
  const normalized = (shortcut || "CommandOrControl+`").split("+").flatMap((part) => {
    const trimmed = part.trim();
    return trimmed ? [trimmed] : [];
  });

  return normalized.map((part) => {
    if (part === "CommandOrControl" || part === "Command" || part === "Meta") return "⌘";
    if (part === "Control" || part === "Ctrl") return "⌃";
    if (part === "Alt" || part === "Option") return "⌥";
    if (part === "Shift") return "⇧";
    if (part === "Escape") return "esc";
    return part.replace("Backquote", "`");
  });
}
