import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HistoryRecordingPlayer } from "./HistoryPlayer";

const dataUrl = `data:audio/wav;base64,${window.btoa("RIFF")}`;

describe("HistoryRecordingPlayer", () => {
  const createObjectURL = vi.fn((_blob: Blob) => "blob:app/clip-1");
  const revokeObjectURL = vi.fn();

  beforeEach(() => {
    URL.createObjectURL = createObjectURL;
    URL.revokeObjectURL = revokeObjectURL;
  });

  afterEach(() => {
    createObjectURL.mockClear();
    revokeObjectURL.mockClear();
  });

  it("plays a saved data URL through a blob URL, which the page CSP allows", () => {
    const { container } = render(<HistoryRecordingPlayer title="Clip" src={dataUrl} />);

    const audio = container.querySelector("audio") as HTMLAudioElement;
    expect(audio.getAttribute("src")).toBe("blob:app/clip-1");
    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(createObjectURL.mock.calls[0][0].type).toBe("audio/wav");
  });

  it("releases the blob URL when the row goes away", () => {
    const { unmount } = render(<HistoryRecordingPlayer title="Clip" src={dataUrl} />);

    unmount();

    expect(revokeObjectURL).toHaveBeenCalledWith("blob:app/clip-1");
  });

  it("renders without a source when the saved audio cannot be decoded", () => {
    const { container } = render(<HistoryRecordingPlayer title="Clip" src="data:audio/wav;base64" />);

    expect(container.querySelector("audio")?.hasAttribute("src")).toBe(false);
    expect(createObjectURL).not.toHaveBeenCalled();
  });

  it("passes any other source through unchanged", () => {
    const { container } = render(<HistoryRecordingPlayer title="Clip" src="blob:app/other" />);

    expect(container.querySelector("audio")?.getAttribute("src")).toBe("blob:app/other");
    expect(createObjectURL).not.toHaveBeenCalled();
  });
});
