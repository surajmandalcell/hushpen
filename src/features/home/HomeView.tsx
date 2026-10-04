import type { ReactNode } from "react";
import { ArrowUpRight, History, Info, Library, Mic2 } from "lucide-react";
import { panelDividerClass, panelSurfaceClass, sharedRadiusClass } from "../../components/ui/classes";
import { ShortcutCluster } from "../../components/ui/ShortcutCluster";
import { getRecordingErrorTitle } from "../../lib/errors";
import { formatDuration } from "../../lib/format";
import { formatShortcutParts } from "../../lib/shortcut";
import type { ViewProps } from "../../types/view";
import { buildHomeStats } from "./homeStats";

export function HomeView({ recording, models, history, runtimeInfo, navigate }: ViewProps) {
  const { isRecording, status: recordingStatus, error: recordingError, durationSeconds } = recording;
  const isBusy = recordingStatus === "starting" || recordingStatus === "preparing-engine" || recordingStatus === "transcribing";
  const statusDetail = recordingStatus === "starting"
    ? "Opening microphone..."
    : recordingStatus === "preparing-engine"
      ? "Preparing Whisper engine..."
    : recordingStatus === "transcribing"
      ? "Loading Whisper model and transcribing..."
      : isRecording
        ? `Recording ${formatDuration(durationSeconds)}`
        : "Turn your voice to text with a single click.";
  const shortcutParts = formatShortcutParts(runtimeInfo?.shortcut);
  const recordingTitle = isRecording ? "Stop recording" : recordingStatus === "preparing-engine" ? "Preparing engine" : recordingStatus === "transcribing" ? "Transcribing" : "Start recording";
  const recordingActionLabel = isRecording ? "Stop Recording" : recordingStatus === "preparing-engine" ? "Preparing Engine" : recordingStatus === "transcribing" ? "Transcribing" : "Start Recording";
  const stats = buildHomeStats(history.rows);
  const openHistory = () => navigate("history");

  return (
    <section className="mx-auto flex w-full max-w-[520px] flex-col gap-5 pt-1">
      <div className={`${panelSurfaceClass} grid grid-cols-2 sm:grid-cols-4`}>
        {stats.map((stat) => (
          <div key={stat.label} className={`border-t ${panelDividerClass} p-4 first:border-t-0 sm:border-l sm:border-t-0 sm:first:border-l-0`}>
            <p className="text-[15px] font-semibold leading-none text-[#f3f3f3]">{stat.value}</p>
            <p className="mt-2 text-[11px] font-semibold leading-none text-[#a4a4a4]">{stat.label}</p>
          </div>
        ))}
      </div>

      <span
        aria-label={isRecording ? "Recording active" : "Recording inactive"}
        className="sr-only"
      />

      <section>
        <h2 className="mb-3 text-[13px] font-semibold text-[#a9a9a9]">Get started</h2>
        <div className="space-y-2">
          <HomeActionRow
            icon={<Mic2 className="size-3.5" />}
            title={recordingTitle}
            detail={statusDetail}
            disabled={isBusy}
            trailing={<ShortcutCluster parts={shortcutParts} />}
            ariaLabel={recordingActionLabel}
            onClick={() => recording.setRecording(!isRecording)}
          />
          <HomeActionRow icon={<History className="size-3.5" />} title="Review history" detail="Replay saved recordings and transcripts." onClick={openHistory} />
          <HomeActionRow icon={<Library className="size-3.5" />} title="Choose speech model" detail={models.selectedModel} onClick={() => navigate("models")} />
        </div>
      </section>

      {recordingError ? (
        <div
          role="alert"
          className="selectable-text flex items-start gap-2 rounded-[12px] border border-[#ff7a66]/25 bg-[#ff6b4a]/10 px-3 py-2 text-left"
        >
          <span className="mt-0.5 grid size-5 shrink-0 place-items-center rounded-full bg-[#ff7a66]/15 text-[#ffad9f]">
            <Info className="size-3" aria-hidden="true" />
          </span>
          <span className="min-w-0">
            <span className="block text-[12px] font-semibold leading-4 text-[#ffd2ca]">
              {getRecordingErrorTitle(recordingError)}
            </span>
            <span className="block break-words text-[12px] font-medium leading-5 text-[#ffad9f]">
              {recordingError}
            </span>
          </span>
        </div>
      ) : null}

      <section>
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[13px] font-semibold text-[#a9a9a9]">What's new?</h2>
          <button type="button" className={`inline-flex items-center gap-1.5 ${sharedRadiusClass} px-2 py-1 text-[12px] font-semibold text-[#ececec] transition hover:bg-white/[0.07] hover:text-white`} onClick={openHistory}>
            <span>View history</span>
            <ArrowUpRight className="size-3" />
          </button>
        </div>
        <div className={panelSurfaceClass}>
          <UpdateRow date="May 14" title="Recording history playback" detail="Saved dictations keep playable source audio with their transcripts." />
          <UpdateRow date="May 14" title="Microphone picker" detail="Choose the input device from the toolbar or Sound settings." />
          <UpdateRow date="May 13" title="Native Whisper engine" detail="Desktop transcription now runs through local Whisper models in Electron." />
        </div>
      </section>
    </section>
  );
}

interface HomeActionRowProps {
  icon: ReactNode;
  title: string;
  detail?: string;
  trailing?: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
  onClick?: () => void;
}

function HomeActionRow({ icon, title, detail, trailing, ariaLabel, disabled = false, onClick }: HomeActionRowProps) {
  const content = (
    <>
      <div className="grid size-7 shrink-0 place-items-center text-[#a8a8a8]">{icon}</div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[14px] font-semibold leading-5 text-[#eeeeee]">{title}</p>
        {detail ? <p className="selectable-text truncate text-[13px] font-semibold leading-5 text-[#aaa]">{detail}</p> : null}
      </div>
      {trailing ? <div className="shrink-0">{trailing}</div> : null}
    </>
  );

  if (onClick) {
    return (
      <button
        type="button"
        aria-label={ariaLabel}
        className={`flex w-full items-center gap-3 ${sharedRadiusClass} px-2.5 py-2 text-left transition hover:bg-[#424242]/80 disabled:cursor-not-allowed disabled:opacity-70`}
        disabled={disabled}
        onClick={onClick}
      >
        {content}
      </button>
    );
  }

  return <div className={`flex w-full items-center gap-3 ${sharedRadiusClass} px-2.5 py-2`}>{content}</div>;
}

interface UpdateRowProps {
  date: string;
  title: string;
  detail: string;
}

function UpdateRow({ date, title, detail }: UpdateRowProps) {
  return (
    <div className={`grid grid-cols-[56px_minmax(0,1fr)] gap-3 border-t ${panelDividerClass} px-4 py-3 first:border-t-0`}>
      <p className="text-[12px] font-semibold text-[#8d8d8d]">{date}</p>
      <div className="min-w-0">
        <p className="truncate text-[13px] font-semibold text-[#eeeeee]">{title}</p>
        <p className="selectable-text mt-1 text-[12px] font-medium leading-5 text-[#b6b6b6]">{detail}</p>
      </div>
    </div>
  );
}
