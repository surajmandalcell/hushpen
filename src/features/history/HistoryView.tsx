import { useEffect, useState } from "react";
import { Copy, FileText, RefreshCw, Search, Trash2 } from "lucide-react";
import { historyActionButtonClass, panelSurfaceClass, sharedRadiusClass } from "../../components/ui/classes";
import { ShortcutCluster } from "../../components/ui/ShortcutCluster";
import { writeTextToClipboard } from "../../lib/clipboard";
import { formatHistoryGroupLabel } from "../../lib/format";
import type { TranscriptHistoryRow } from "../../types/history";
import type { ViewProps } from "../../types/view";
import { HistoryRecordingPlayer, MissingHistoryAudioNotice } from "./HistoryPlayer";

export function HistoryView({ history }: ViewProps) {
  const { rows, reprocessingRowId, openingTranscriptRowId } = history;
  const [query, setQuery] = useState("");
  const [expandedRowId, setExpandedRowId] = useState<string | null>(rows[0]?.id ?? null);
  const normalizedQuery = query.trim().toLowerCase();
  const filteredRows = rows.filter((row) => (
    !normalizedQuery
    || row.title.toLowerCase().includes(normalizedQuery)
    || row.text.toLowerCase().includes(normalizedQuery)
    || row.model.toLowerCase().includes(normalizedQuery)
  ));
  const groupedRows = filteredRows.reduce<Array<{ label: string; rows: TranscriptHistoryRow[] }>>((groups, row) => {
    const label = formatHistoryGroupLabel(row.createdAt);
    const group = groups.find((item) => item.label === label);
    if (group) {
      group.rows.push(row);
    } else {
      groups.push({ label, rows: [row] });
    }
    return groups;
  }, []);

  useEffect(() => {
    setExpandedRowId((current) => {
      if (current && rows.some((row) => row.id === current)) return current;
      return rows[0]?.id ?? null;
    });
  }, [rows]);

  return (
    <section className="mx-auto w-full max-w-[520px] space-y-4">
      <h2 className="sr-only">History</h2>
      <div className="flex h-10 items-center gap-2 rounded-full border border-white/[0.09] bg-white/[0.035] px-3 text-[#8e8e8e] backdrop-blur-2xl">
        <Search className="size-3.5 shrink-0" />
        <input
          type="search"
          aria-label="Search history"
          className="liquid-search-input min-w-0 flex-1 appearance-none border-0 bg-transparent p-0 text-[13px] font-semibold text-[#e8e8e8] outline-none placeholder:text-[#777]"
          placeholder="Find..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <ShortcutCluster parts={["⌘", "F"]} muted />
      </div>

      {groupedRows.length ? (
        <div className="space-y-5">
          {groupedRows.map((group) => (
            <section key={group.label} className="space-y-2">
              <h3 className="px-2 text-[12px] font-semibold text-[#858585]">{group.label}</h3>
              <div className="space-y-2">
                {group.rows.map((row) => (
                  <HistoryCard
                    key={row.id}
                    row={row}
                    expanded={expandedRowId === row.id}
                    onCopy={() => writeTextToClipboard(row.text)}
                    onReprocess={() => history.reprocessRow(row)}
                    onOpenTranscript={() => history.openTranscriptText(row)}
                    onDelete={() => history.deleteRow(row)}
                    onToggle={() => setExpandedRowId((current) => (current === row.id ? null : row.id))}
                    reprocessing={reprocessingRowId === row.id}
                    openingTranscript={openingTranscriptRowId === row.id}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className={`${panelSurfaceClass} p-6 text-center`}>
          <p className="text-[14px] font-semibold text-[#eeeeee]">No transcription history yet</p>
          <p className="selectable-text mt-1 text-[12px] leading-5 text-[#b4b4b4]">Stop a recording after dictation and the transcript will appear here.</p>
        </div>
      )}
    </section>
  );
}

interface HistoryCardProps {
  row: TranscriptHistoryRow;
  expanded: boolean;
  onCopy: () => void;
  onReprocess: () => void;
  onOpenTranscript: () => void;
  onDelete: () => void;
  onToggle: () => void;
  reprocessing: boolean;
  openingTranscript: boolean;
}

function HistoryCard({ row, expanded, onCopy, onReprocess, onOpenTranscript, onDelete, onToggle, reprocessing, openingTranscript }: HistoryCardProps) {
  const canReprocess = Boolean(row.recordingUrl);
  const canOpenTranscript = Boolean(row.text.trim());

  return (
    <article className={`${panelSurfaceClass} p-4 transition-colors duration-200 ${expanded ? "bg-white/[0.07]" : ""}`}>
      <button
        type="button"
        className="block w-full text-left"
        onClick={onToggle}
      >
        <p className={`selectable-text ${expanded ? "line-clamp-3" : "truncate"} text-[13px] font-semibold leading-5 text-[#f1f1f1]`}>
          {row.title}
        </p>
        {expanded && row.status !== "failed" && row.text && row.text !== row.title ? (
          <p className="selectable-text mt-2 text-[12px] font-medium leading-5 text-[#c7c7c7]">{row.text}</p>
        ) : null}
      </button>

      {expanded ? (
          <div className="history-card-details mt-3 space-y-3">
          {row.recordingUrl ? (
            <HistoryRecordingPlayer title={row.title} src={row.recordingUrl} />
          ) : (
            <MissingHistoryAudioNotice />
          )}
          {row.status === "failed" ? (
            <p className={`selectable-text ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.05] px-3 py-2 text-[12px] font-medium text-[#ffb3aa]`}>
              {row.error ?? "Transcription failed"}
            </p>
          ) : null}
          <div className="flex items-center justify-between">
            <div className={`inline-flex ${sharedRadiusClass} bg-white/[0.07] p-0.5`}>
              <span className="rounded-[7px] bg-[#777] px-2 py-1 text-[12px] font-semibold text-white">Original</span>
              <span className="px-2 py-1 text-[12px] font-semibold text-[#9d9d9d]">Segmented</span>
            </div>
            <div className="flex items-center gap-1 text-[#bdbdbd]">
              {canReprocess ? (
                <button
                  type="button"
                  aria-busy={reprocessing || undefined}
                  aria-label={`Reprocess clip: ${row.title}`}
                  className={historyActionButtonClass}
                  disabled={reprocessing}
                  onClick={onReprocess}
                >
                  <RefreshCw className={`size-3 ${reprocessing ? "animate-spin" : ""}`} />
                </button>
              ) : null}
              {canOpenTranscript ? (
                <button
                  type="button"
                  aria-busy={openingTranscript || undefined}
                  aria-label={`Open transcript text: ${row.title}`}
                  className={historyActionButtonClass}
                  disabled={openingTranscript}
                  onClick={onOpenTranscript}
                >
                  <FileText className="size-3" />
                </button>
              ) : null}
              <button type="button" aria-label={`Copy transcript: ${row.title}`} className={historyActionButtonClass} onClick={onCopy}>
                <Copy className="size-3" />
              </button>
              <button type="button" aria-label={`Delete transcript: ${row.title}`} className={historyActionButtonClass} onClick={onDelete}>
                <Trash2 className="size-3" />
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </article>
  );
}
