function swallow(event: DragEvent) {
  if (!event.dataTransfer?.types.includes("Files")) return;
  event.preventDefault();
  if (event.type === "dragover") event.dataTransfer.dropEffect = "none";
}

/**
 * Stops the browser from opening a dropped file as a page. A handler that
 * accepts drops calls preventDefault itself, so this only swallows the rest.
 */
export function swallowStrayFileDrops(target: Document = document): () => void {
  target.addEventListener("dragover", swallow);
  target.addEventListener("drop", swallow);
  return () => {
    target.removeEventListener("dragover", swallow);
    target.removeEventListener("drop", swallow);
  };
}
