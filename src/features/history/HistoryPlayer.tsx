import { useRef, useState } from "react";
import { Pause, Play, VolumeX } from "lucide-react";
import { sharedRadiusClass } from "../../components/ui/classes";
import { historyWaveformBars } from "../../lib/waveform";

export function MissingHistoryAudioNotice() {
  return (
    <div className={`flex items-center gap-2 ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.05] px-3 py-2 text-[#bdbdbd]`}>
      <VolumeX className="size-3 shrink-0" aria-hidden="true" />
      <span className="text-[12px] font-semibold">No source audio saved</span>
    </div>
  );
}

interface HistoryRecordingPlayerProps {
  title: string;
  src: string;
}

export function HistoryRecordingPlayer({ title, src }: HistoryRecordingPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);

  const togglePlayback = async () => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlaying) {
      audio.pause();
      setIsPlaying(false);
      return;
    }

    try {
      await audio.play();
      setIsPlaying(true);
    } catch {
      setIsPlaying(false);
    }
  };

  return (
    <div className={`flex items-center gap-3 ${sharedRadiusClass} border border-white/[0.08] bg-white/[0.085] px-3 py-2`}>
      <button
        type="button"
        aria-label={`${isPlaying ? "Pause" : "Play"} recording: ${title}`}
        className={`grid size-7 shrink-0 place-items-center ${sharedRadiusClass} text-white transition active:scale-[0.97] ${isPlaying ? "bg-[#0a84ff]" : "bg-white/[0.12] hover:bg-white/[0.18]"}`}
        onClick={togglePlayback}
      >
        {isPlaying ? <Pause className="size-3" /> : <Play className="size-3" />}
      </button>
      <div className="flex min-w-0 flex-1 items-center gap-px overflow-hidden" aria-hidden="true">
        {historyWaveformBars.map((bar) => (
          <span
            key={bar.id}
            className="w-px shrink-0 rounded-full bg-[#a9a9a9]"
            style={{ height: `${bar.height}px`, opacity: bar.index % 4 === 0 ? 0.35 : 0.62 }}
          />
        ))}
      </div>
      <audio
        ref={audioRef}
        aria-label={`Recording audio: ${title}`}
        preload="metadata"
        src={src}
        onEnded={() => setIsPlaying(false)}
        onPause={() => setIsPlaying(false)}
        onPlay={() => setIsPlaying(true)}
      />
    </div>
  );
}
