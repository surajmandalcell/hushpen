import React, { useState } from "react";
import { Upload, FileAudio, FolderOpen } from "lucide-react";

const TranscribePage: React.FC = () => {
  const [isDragging, setIsDragging] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");

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

  const processFiles = (files: File[]) => {
    setIsProcessing(true);
    setCurrentFile(files[0].name);

    // Simulate file processing
    let currentProgress = 0;
    const interval = setInterval(() => {
      currentProgress += 10;
      setProgress(currentProgress);

      if (currentProgress >= 100) {
        clearInterval(interval);
        setTimeout(() => {
          setIsProcessing(false);
          setProgress(0);
          setCurrentFile("");
        }, 1000);
      }
    }, 200);
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

        <div
          className={`drop-zone ${isDragging ? "dragging" : ""} ${
            isProcessing ? "processing" : ""
          }`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
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
                id="file-input"
              />
              <label
                htmlFor="file-input"
                className="button"
                style={{ marginTop: "16px" }}
              >
                <FolderOpen size={16} style={{ marginRight: "8px" }} />
                Browse Files
              </label>
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
        </div>
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
            <button className="button">
              <FolderOpen size={16} style={{ marginRight: "6px" }} />
              Choose Folder
            </button>
          </div>
        </div>
      </div>

      <style dangerouslySetInnerHTML={{
        __html: `
        .drop-zone {
          border: 2px dashed var(--border-color);
          border-radius: var(--border-radius);
          padding: 48px 24px;
          text-align: center;
          background: linear-gradient(
            135deg,
            var(--card-bg) 0%,
            var(--secondary-bg) 100%
          );
          transition: all 0.3s ease;
          cursor: pointer;
        }

        .drop-zone:hover,
        .drop-zone.dragging {
          border-color: var(--accent-blue);
          background: linear-gradient(
            135deg,
            rgba(0, 122, 204, 0.2) 0%,
            var(--card-bg) 100%
          );
        }

        .drop-zone.processing {
          border-color: var(--success-green);
          cursor: default;
        }

        .drop-content,
        .processing-content {
          color: var(--primary-text);
        }

        .drop-icon,
        .processing-icon {
          margin-bottom: 16px;
          color: var(--secondary-text);
        }

        .processing-icon {
          animation: pulse 2s infinite;
          color: var(--accent-blue);
        }

        @keyframes pulse {
          0%,
          100% {
            opacity: 1;
          }
          50% {
            opacity: 0.5;
          }
        }

        .drop-content h3,
        .processing-content h3 {
          font-size: 18px;
          font-weight: 600;
          margin: 0 0 8px 0;
        }

        .drop-content p,
        .processing-content p {
          color: var(--secondary-text);
          margin: 0;
        }

        .progress-bar {
          width: 100%;
          height: 8px;
          background-color: var(--border-color);
          border-radius: 4px;
          margin: 16px 0 8px 0;
          overflow: hidden;
        }

        .progress-fill {
          height: 100%;
          background: linear-gradient(
            90deg,
            var(--accent-blue),
            var(--success-green)
          );
          transition: width 0.3s ease;
        }
        `
      }} />
    </div>
  );
};

export default TranscribePage;
