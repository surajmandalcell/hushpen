import React, { useState, useId } from "react";
import { Upload, FileAudio, FolderOpen, Container, Clock } from "lucide-react";
import { fileSystemService } from "../services/fileSystem";
import { apiClient, TranscriptionResponse } from "../services/api";
import { webSocketService } from "../services/websocket";

const TranscribePage: React.FC = () => {
  const fileInputId = useId();
  const [isDragging, setIsDragging] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");
  const [transcriptionResults, setTranscriptionResults] = useState<TranscriptionResponse[]>([]);
  const [currentModel, setCurrentModel] = useState("");

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
        // Convert file paths to File objects
        const fileObjects = await Promise.all(
          files.map(async (path) => {
            const binaryData = await fileSystemService.readBinaryFile(path);
            const fileName = path.split("/").pop() || "unknown";
            // Convert Uint8Array to ArrayBuffer for File constructor compatibility
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
      // Process each file
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        setCurrentFile(file.name);
        setProgress(0);

        try {
          // Use the API client to transcribe the file
          const result = await apiClient.transcribeFile(file);
          console.log(`Transcription completed for ${file.name}:`, result);

          // Update the current model if available
          if (result.model_id) {
            setCurrentModel(result.model_id);
          }

          // Store the transcription result
          setTranscriptionResults(prev => [...prev, result]);

          // Save the transcription result
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
    <div>
      <div className="page-header">
        <h1 className="page-title">Transcribe Files</h1>
        <p className="page-description">
          Upload audio files to transcribe them using AI speech recognition.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">File Upload</h2>

        <button
          type="button"
          className={`drop-zone ${isDragging ? "dragging" : ""} ${
            isProcessing ? "processing" : ""
          }`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={() => document.getElementById(fileInputId)?.click()}
          disabled={isProcessing}
          aria-label="Drop zone for audio files"
          style={{
            border: '2px dashed var(--border-color)',
            borderRadius: 'var(--border-radius)',
            padding: '48px 24px',
            textAlign: 'center',
            background: isProcessing 
              ? 'linear-gradient(135deg, var(--card-bg) 0%, var(--secondary-bg) 100%)'
              : isDragging 
                ? 'linear-gradient(135deg, rgba(0, 122, 204, 0.2) 0%, var(--card-bg) 100%)'
                : 'linear-gradient(135deg, var(--card-bg) 0%, var(--secondary-bg) 100%)',
            transition: 'all 0.3s ease',
            cursor: isProcessing ? 'default' : 'pointer',
            width: '100%',
            display: 'block'
          }}
        >
          {isProcessing ? (
            <div className="processing-content">
              <FileAudio size={48} className="processing-icon" />
              <h3>Processing {currentFile}</h3>
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <p>{progress}% complete</p>
            </div>
          ) : (
            <div className="drop-content">
              <Upload size={48} className="drop-icon" />
              <h3>Drop audio files here</h3>
              <p>or click to browse</p>
              <input
                type="file"
                multiple
                accept="audio/*,.wav,.mp3,.m4a,.flac"
                onChange={handleFileSelect}
                style={{ display: "none" }}
                id={fileInputId}
              />
              <label
                htmlFor={fileInputId}
                className="button"
                style={{ marginTop: "16px" }}
              >
                <Upload size={16} style={{ marginRight: "8px" }} />
                Browse Files
              </label>
              <button
                type="button"
                className="button"
                style={{ marginTop: "16px" }}
                onClick={handleNativeFileDialog}
              >
                <FolderOpen size={16} style={{ marginRight: "8px" }} />
                Open Native Dialog
              </button>
              <p
                style={{
                  fontSize: "12px",
                  color: "var(--secondary-text)",
                  marginTop: "12px",
                }}
              >
                Supported formats: WAV, MP3, M4A, FLAC
              </p>
            </div>
          )}
        </button>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Transcription Results</h2>
        
        {transcriptionResults.length > 0 && (
          <div style={{ marginTop: '16px' }}>
            {transcriptionResults.map((result, index) => (
              <div key={`transcription-${index}-${result.text?.substring(0, 10)}`} className="setting-row" style={{
                padding: '12px', 
                backgroundColor: 'var(--secondary-bg)', 
                borderRadius: 'var(--border-radius)',
                marginBottom: '12px'
              }}>
                <div className="setting-info">
                  <h3 className="setting-label" style={{ marginBottom: '8px' }}>
                    Transcription Result
                  </h3>
                  <p className="setting-description" style={{ 
                    fontSize: '14px', 
                    lineHeight: '1.5',
                    marginBottom: '8px'
                  }}>
                    {result.text}
                  </p>
                  
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '16px', marginTop: '12px' }}>
                    {result.model_id && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Container size={14} color="var(--secondary-text)" />
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Model: {result.model_id}
                        </span>
                      </div>
                    )}
                    
                    {result.backend && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Container size={14} color="var(--secondary-text)" />
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Backend: {result.backend}
                        </span>
                      </div>
                    )}
                    
                    {result.processing_time && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Clock size={14} color="var(--secondary-text)" />
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Processing time: {result.processing_time.toFixed(2)}s
                        </span>
                      </div>
                    )}
                    
                    {result.container_info?.status && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Container size={14} color="var(--secondary-text)" />
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Container status: {result.container_info.status}
                          {result.container_info.gpu_allocated && " (GPU)"}
                        </span>
                      </div>
                    )}
                    
                    {result.language && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Language: {result.language}
                        </span>
                      </div>
                    )}
                    
                    {result.duration && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{ fontSize: '12px', color: 'var(--secondary-text)' }}>
                          Duration: {result.duration.toFixed(2)}s
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <h2 className="section-title">Output Settings</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Output Format</h3>
            <p className="setting-description">
              Choose the format for transcription output
            </p>
          </div>
          <div className="setting-control">
            <select className="dropdown">
              <option value="txt">Text (.txt)</option>
              <option value="srt">Subtitles (.srt)</option>
              <option value="json">JSON (.json)</option>
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Output Directory</h3>
            <p className="setting-description">
              Where to save transcribed files
            </p>
          </div>
          <div className="setting-control">
            <button type="button" className="button">
              <FolderOpen size={16} style={{ marginRight: "6px" }} />
              Choose Folder
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default TranscribePage;