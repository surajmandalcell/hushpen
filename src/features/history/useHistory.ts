import { useCallback, useState } from "react";
import { getErrorMessage, getRecordingErrorTitle } from "../../lib/errors";
import { buildHistoryTitle } from "../../lib/history";
import { dataUrlToBlob } from "../../lib/wav";
import type { TranscriptHistoryRow } from "../../types/history";
import type { Transcribe } from "../recording/useTranscriber";
import type { HistoryRepository } from "./historyRepository";

interface UseHistoryOptions {
  repository: HistoryRepository;
  selectedModel: string;
  transcribe: Transcribe;
}

export function useHistory({ repository, selectedModel, transcribe }: UseHistoryOptions) {
  const [rows, setRows] = useState<TranscriptHistoryRow[]>(() => repository.list());
  const [reprocessingRowId, setReprocessingRowId] = useState<string | null>(null);
  const [openingTranscriptRowId, setOpeningTranscriptRowId] = useState<string | null>(null);

  const addRow = useCallback((row: TranscriptHistoryRow) => {
    setRows((current) => {
      const next = [row, ...current].slice(0, 100);
      repository.save(next);
      return next;
    });
  }, [repository]);

  const updateRow = useCallback((rowId: string, updater: (row: TranscriptHistoryRow) => TranscriptHistoryRow) => {
    setRows((current) => {
      const next = current.map((row) => (row.id === rowId ? updater(row) : row));
      repository.save(next);
      return next;
    });
  }, [repository]);

  const deleteRow = useCallback((row: TranscriptHistoryRow) => {
    const deleteTranscriptText = window.asrpro?.deleteTranscriptText;
    if (deleteTranscriptText) {
      void deleteTranscriptText({
        title: row.title,
        filePath: row.transcriptFilePath,
      }).catch(() => {});
    }

    setRows((current) => {
      const next = current.filter((currentRow) => currentRow.id !== row.id);
      repository.save(next);
      return next;
    });
  }, [repository]);

  const reprocessRow = useCallback(async (row: TranscriptHistoryRow) => {
    if (!row.recordingUrl || reprocessingRowId) return;

    setReprocessingRowId(row.id);

    try {
      const result = await transcribe(dataUrlToBlob(row.recordingUrl));
      const text = typeof result === "string" ? result : result?.text;
      if (!text || !text.trim()) {
        throw new Error("No transcription text returned");
      }

      const normalizedText = text.replace(/\s+/g, " ").trim();
      updateRow(row.id, (current) => ({
        ...current,
        title: buildHistoryTitle(normalizedText),
        text: normalizedText,
        model: selectedModel,
        status: "completed",
        error: undefined,
      }));
    } catch (error) {
      const message = getErrorMessage(error);
      updateRow(row.id, (current) => ({
        ...current,
        text: current.status === "failed" || !current.text.trim() ? message : current.text,
        title: current.status === "failed" || !current.title.trim() ? getRecordingErrorTitle(message) : current.title,
        model: selectedModel,
        status: "failed",
        error: message,
      }));
    } finally {
      setReprocessingRowId(null);
    }
  }, [reprocessingRowId, selectedModel, transcribe, updateRow]);

  const openTranscriptText = useCallback(async (row: TranscriptHistoryRow) => {
    if (!row.text.trim() || openingTranscriptRowId) return;

    setOpeningTranscriptRowId(row.id);

    try {
      const request = {
        title: row.title,
        text: row.text,
      };

      if (window.asrpro?.openTranscriptText) {
        const result = await window.asrpro.openTranscriptText(request);
        if (result?.filePath) {
          updateRow(row.id, (current) => ({
            ...current,
            transcriptFilePath: result.filePath,
          }));
        }
        return;
      }

      const blobUrl = URL.createObjectURL(new Blob([`${row.text.trim()}\n`], { type: "text/plain;charset=utf-8" }));
      window.open(blobUrl, "_blank", "noopener,noreferrer");
      window.setTimeout(() => URL.revokeObjectURL(blobUrl), 60_000);
    } finally {
      setOpeningTranscriptRowId(null);
    }
  }, [openingTranscriptRowId, updateRow]);

  return {
    rows,
    reprocessingRowId,
    openingTranscriptRowId,
    addRow,
    deleteRow,
    reprocessRow,
    openTranscriptText,
  };
}

export type HistoryState = ReturnType<typeof useHistory>;
