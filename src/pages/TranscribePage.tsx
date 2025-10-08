import { useState, useId, type FC } from "react";
import { Upload, FileAudio, FolderOpen, Container, Clock } from "lucide-react";
import { fileSystemService } from "../services/fileSystem";
import { apiClient, TranscriptionResponse } from "../services/api";
import { PalPanelHeader, PalText, PalCard, PalButton, PalSelect } from "../components/palantirui";

const TranscribePage: FC = () => {
  const fileInputId = useId();
  const [isDragging, setIsDragging] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");
  const [transcriptionResults, setTranscriptionResults] = useState<TranscriptionResponse[]>([]);
  const [outputFormat, setOutputFormat] = useState("txt");

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);

    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      processFiles(files);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    if (files.length > 0) {
      processFiles(files);
    }
  };

  const handleNativeFileDialog = async () => {
    try {
      const files = await fileSystemService.openFileDialog({
        multiple: true,
        filters: [
          {
            name: "Audio Files",
            extensions: ["mp3", "wav", "m4a", "flac", "ogg"],
          },
        ],
      });

      if (files && files.length > 0) {
        const fileObjects = await Promise.all(
          files.map(async (path) => {
            const binaryData = await fileSystemService.readBinaryFile(path);
            const fileName = path.split("/").pop() || "unknown";
            const arrayBuffer = new ArrayBuffer(binaryData.length);
            new Uint8Array(arrayBuffer).set(binaryData);
            return new File([arrayBuffer], fileName, { type: "audio/wav" });
          })
        );

        processFiles(fileObjects);
      }
    } catch (error) {
      console.error("Failed to open native file dialog:", error);
    }
  };

  const processFiles = async (files: File[]) => {
    setIsProcessing(true);
    setCurrentFile(files[0].name);

    try {
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        setCurrentFile(file.name);
        setProgress(0);

        try {
          const result = await apiClient.transcribeFile(file);
          console.log(`Transcription completed for ${file.name}:`, result);

          setTranscriptionResults(prev => [...prev, result]);

          if (result.text) {
            await fileSystemService.saveFileDialog(
              result.text,
              undefined,
              `${file.name.replace(/\.[^/.]+$/, "")}-transcription.txt`
            );
          }
        } catch (error) {
          console.error(`Failed to transcribe ${file.name}:`, error);
        }

        setProgress(100);
      }

      setIsProcessing(false);
      setProgress(0);
      setCurrentFile("");
    } catch (error) {
      console.error("Error processing files:", error);
      setIsProcessing(false);
      setProgress(0);
      setCurrentFile("");
    }
  };

  return (
    <div className="space-y-6">
      <PalPanelHeader
        title="Transcribe Files"
        subtitle="Upload audio files to transcribe them using AI speech recognition."
        withBorder={false}
      />

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
        <PalText size="lg" weight="semibold">File Upload</PalText>

        <div
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={() => !isProcessing && document.getElementById(fileInputId)?.click()}
          className={`
            border-2 border-dashed rounded-pal-lg p-12 text-center transition-all duration-300 cursor-pointer
            ${isDragging ? 'border-palantir-accent-blue bg-palantir-accent-blue bg-opacity-10' : 'border-palantir-zinc-300 dark:border-palantir-zinc-600'}
            ${isProcessing ? 'cursor-default opacity-75' : 'hover:border-palantir-accent-blue hover:bg-palantir-zinc-50 dark:hover:bg-palantir-zinc-800'}
          `}
        >
          {isProcessing ? (
            <div className="space-y-4">
              <FileAudio size={48} className="mx-auto text-palantir-accent-blue animate-pulse" />
              <div>
                <PalText size="lg" weight="semibold">Processing {currentFile}</PalText>
                <div className="mt-3 w-full bg-palantir-zinc-200 dark:bg-palantir-zinc-700 rounded-full h-2">
                  <div
                    className="bg-palantir-accent-blue h-2 rounded-full transition-all duration-300"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <PalText size="sm" variant="muted" className="mt-2">{progress}% complete</PalText>
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              <Upload size={48} className="mx-auto text-palantir-zinc-400" />
              <div>
                <PalText size="lg" weight="semibold">Drop audio files here</PalText>
                <PalText size="sm" variant="muted">or click to browse</PalText>
              </div>
              <input
                type="file"
                multiple
                accept="audio/*,.wav,.mp3,.m4a,.flac"
                onChange={handleFileSelect}
                style={{ display: "none" }}
                id={fileInputId}
              />
              <div className="flex gap-3 justify-center">
                <PalButton variant="primary" size="md">
                  <Upload size={16} className="mr-2" />
                  Browse Files
                </PalButton>
                <PalButton variant="secondary" size="md" onClick={(e) => { e.stopPropagation(); handleNativeFileDialog(); }}>
                  <FolderOpen size={16} className="mr-2" />
                  Open Native Dialog
                </PalButton>
              </div>
              <PalText size="xs" variant="muted">
                Supported formats: WAV, MP3, M4A, FLAC
              </PalText>
            </div>
          )}
        </div>
      </PalCard>

      {transcriptionResults.length > 0 && (
        <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
          <PalText size="lg" weight="semibold">Transcription Results</PalText>

          <div className="space-y-3">
            {transcriptionResults.map((result, index) => (
              <PalCard
                key={`transcription-${index}-${result.text?.substring(0, 10)}`}
                variant="default"
                padding="md"
                className="bg-palantir-zinc-50 dark:bg-palantir-zinc-800"
              >
                <div className="space-y-3">
                  <PalText weight="semibold">Transcription Result</PalText>

                  <PalText size="sm" className="leading-relaxed">
                    {result.text}
                  </PalText>

                  <div className="flex flex-wrap gap-4 text-palantir-zinc-500 dark:text-palantir-zinc-400">
                    {result.model_id && (
                      <div className="flex items-center gap-2">
                        <Container size={14} />
                        <PalText size="xs">Model: {result.model_id}</PalText>
                      </div>
                    )}

                    {result.backend && (
                      <div className="flex items-center gap-2">
                        <Container size={14} />
                        <PalText size="xs">Backend: {result.backend}</PalText>
                      </div>
                    )}

                    {result.processing_time && (
                      <div className="flex items-center gap-2">
                        <Clock size={14} />
                        <PalText size="xs">Processing time: {result.processing_time.toFixed(2)}s</PalText>
                      </div>
                    )}

                    {result.container_info?.status && (
                      <div className="flex items-center gap-2">
                        <Container size={14} />
                        <PalText size="xs">
                          Container: {result.container_info.status}
                          {result.container_info.gpu_allocated && " (GPU)"}
                        </PalText>
                      </div>
                    )}

                    {result.language && (
                      <PalText size="xs">Language: {result.language}</PalText>
                    )}

                    {result.duration && (
                      <PalText size="xs">Duration: {result.duration.toFixed(2)}s</PalText>
                    )}
                  </div>
                </div>
              </PalCard>
            ))}
          </div>
        </PalCard>
      )}

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
        <PalText size="lg" weight="semibold">Output Settings</PalText>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Output Format</PalText>
            <PalText size="sm" variant="muted">
              Choose the format for transcription output
            </PalText>
          </div>
          <PalSelect value={outputFormat} onChange={(e) => setOutputFormat(e.target.value)}>
            <option value="txt">Text (.txt)</option>
            <option value="srt">Subtitles (.srt)</option>
            <option value="json">JSON (.json)</option>
          </PalSelect>
        </div>

        <div className="flex items-center justify-between py-3">
          <div>
            <PalText weight="medium">Output Directory</PalText>
            <PalText size="sm" variant="muted">
              Where to save transcribed files
            </PalText>
          </div>
          <PalButton variant="primary" size="sm">
            <FolderOpen size={16} className="mr-2" />
            Choose Folder
          </PalButton>
        </div>
      </PalCard>
    </div>
  );
};

export default TranscribePage;
