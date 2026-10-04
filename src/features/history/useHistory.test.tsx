import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderApp } from "../../test/renderApp";
import type { TranscriptHistoryRow } from "../../types/history";
import type { HistoryRepository } from "./historyRepository";

afterEach(() => {
  window.asrpro = undefined;
  cleanup();
});

function buildRow(id: string, title: string): TranscriptHistoryRow {
  return {
    id,
    title,
    text: `Transcript for ${title}`,
    kind: "Dictation",
    model: "Whisper Base English",
    durationSeconds: 5,
    createdAt: Date.now(),
    status: "completed",
  };
}

describe("history through the repository", () => {
  it("reads the initial rows from the injected repository", async () => {
    const repository: HistoryRepository = {
      list: vi.fn(() => [buildRow("one", "First note"), buildRow("two", "Second note")]),
      save: vi.fn(),
    };
    const user = userEvent.setup();

    await renderApp({ historyRepository: repository });
    await user.click(screen.getByRole("button", { name: "History" }));

    expect(repository.list).toHaveBeenCalledTimes(1);
    expect(screen.getByText("First note")).toBeTruthy();
    expect(screen.getByText("Second note")).toBeTruthy();
  });

  it("saves the remaining rows through the repository when a row is deleted", async () => {
    const repository: HistoryRepository = {
      list: vi.fn(() => [buildRow("one", "First note"), buildRow("two", "Second note")]),
      save: vi.fn(),
    };
    const user = userEvent.setup();

    await renderApp({ historyRepository: repository });
    await user.click(screen.getByRole("button", { name: "History" }));
    await user.click(screen.getByRole("button", { name: "Delete transcript: First note" }));

    expect(repository.save).toHaveBeenCalled();
    const saved = vi.mocked(repository.save).mock.calls.at(-1)?.[0] ?? [];
    expect(saved.map((row) => row.id)).toEqual(["two"]);
    expect(screen.queryByText("First note")).toBeNull();
  });
});
