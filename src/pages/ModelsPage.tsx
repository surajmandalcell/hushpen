import React, { useState } from "react";
import { Download, CheckCircle, Clock, AlertCircle } from "lucide-react";

const ModelsPage: React.FC = () => {
  const [currentModel, setCurrentModel] = useState("whisper-base");
  const [language, setLanguage] = useState("auto");
  const [processingDevice, setProcessingDevice] = useState("cpu");
  const [realTimeProcessing, setRealTimeProcessing] = useState(true);

  const availableModels = [
    { id: "whisper-tiny", name: "Whisper Tiny", size: "39M", status: "ready" },
    { id: "whisper-base", name: "Whisper Base", size: "74M", status: "ready" },
    {
      id: "whisper-large",
      name: "Whisper Large",
      size: "1550M",
      status: "downloading",
    },
  ];

  const languages = [
    { id: "auto", name: "Auto-detect" },
    { id: "en", name: "English" },
    { id: "es", name: "Spanish" },
    { id: "fr", name: "French" },
    { id: "de", name: "German" },
    { id: "it", name: "Italian" },
    { id: "pt", name: "Portuguese" },
    { id: "ru", name: "Russian" },
    { id: "ja", name: "Japanese" },
    { id: "zh", name: "Chinese" },
  ];

  const devices = [
    { id: "cpu", name: "CPU" },
    { id: "cuda", name: "GPU (CUDA)" },
    { id: "metal", name: "GPU (Metal)" },
  ];

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Models & Dictation Engine</h1>
        <p className="page-description">
          Configure AI models, language settings, and processing options for
          speech recognition.
        </p>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Model Configuration</h2>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Current Model</h3>
            <p className="setting-description">
              Select the AI model to use for speech recognition
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={currentModel}
              onChange={(e) => setCurrentModel(e.target.value)}
            >
              {availableModels.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name} ({model.size})
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Language</h3>
            <p className="setting-description">
              Primary language for speech recognition
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
            >
              {languages.map((lang) => (
                <option key={lang.id} value={lang.id}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Processing Device</h3>
            <p className="setting-description">
              Hardware to use for AI model processing
            </p>
          </div>
          <div className="setting-control">
            <select
              className="dropdown"
              value={processingDevice}
              onChange={(e) => setProcessingDevice(e.target.value)}
            >
              {devices.map((device) => (
                <option key={device.id} value={device.id}>
                  {device.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <h3 className="setting-label">Real-time Processing</h3>
            <p className="setting-description">
              Enable real-time speech recognition for live transcription
            </p>
          </div>
          <div className="setting-control">
            <div
              className={`toggle-switch ${realTimeProcessing ? "active" : ""}`}
              onClick={() => setRealTimeProcessing(!realTimeProcessing)}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Available Models</h2>

        {availableModels.map((model) => {
          const getStatusIcon = () => {
            switch (model.status) {
              case "ready":
                return <CheckCircle size={16} color="var(--success-green)" />;
              case "downloading":
                return <Clock size={16} color="var(--warning-yellow)" />;
              default:
                return <AlertCircle size={16} color="var(--secondary-text)" />;
            }
          };

          return (
            <div key={model.id} className="setting-row">
              <div className="setting-info">
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  {getStatusIcon()}
                  <h3 className="setting-label">{model.name}</h3>
                </div>
                <p className="setting-description">
                  Size: {model.size} | Status: {model.status}
                </p>
              </div>
              <div className="setting-control">
                {model.status === "ready" ? (
                  <button
                    className="button"
                    onClick={() => setCurrentModel(model.id)}
                    disabled={currentModel === model.id}
                  >
                    {currentModel === model.id ? "Selected" : "Select"}
                  </button>
                ) : (
                  <button className="button" disabled>
                    {model.status === "downloading" ? (
                      <>
                        <Clock size={16} style={{ marginRight: "6px" }} />
                        Downloading...
                      </>
                    ) : (
                      <>
                        <Download size={16} style={{ marginRight: "6px" }} />
                        Download
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default ModelsPage;
