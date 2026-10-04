export function writeTextToClipboard(text: string) {
  void navigator.clipboard?.writeText(text).catch(() => {});
}
