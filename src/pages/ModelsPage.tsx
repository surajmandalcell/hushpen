import React, { useState, useEffect } from "react";
import { Download, CheckCircle, Clock, AlertCircle, Container } from "lucide-react";
import { apiClient, Model } from "../services/api";
import { webSocketService, ContainerStatusData } from "../services/websocket";

const ModelsPage: React.FC = () => {
  const [currentModel, setCurrentModel] = useState("whisper-base");
  const [language, setLanguage] = useState("auto");
  const [processingDevice, setProcessingDevice] = useState("Docker container");
  const [realTimeProcessing, setRealTimeProcessing] = useState(true);
  const [availableModels, setAvailableModels] = useState<Model[]>([]);
  const [containerStatuses, setContainerStatuses] = useState<Record<string, ContainerStatusData>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [dockerAvailable, setDockerAvailable] = useState(true);
  const [gpuAvailable, setGpuAvailable] = useState(false);

  // Fetch models from API on component mount
  useEffect(() => {
    const fetchModels = async () => {
      try {
        const response = await apiClient.listModels();
        setAvailableModels(response.data);
      } catch (error) {
        console.error("Failed to fetch models:", error);
      } finally {
        setIsLoading(false);
      }
    };

    const fetchHealth = async () => {
      try {
        const health = await apiClient.healthCheck();
        setProcessingDevice(health.device || "Docker container");
        if (health.status === "unhealthy") {
          setDockerAvailable(false);
        }
      } catch (error) {
        console.error("Failed to fetch health status:", error);
        setDockerAvailable(false);
      }
    };

    fetchModels();
    fetchHealth();

    // Subscribe to WebSocket for container status updates
    const unsubscribe = webSocketService.subscribe((message) => {
      if (message.type === "container_status") {
        const data = message.data as ContainerStatusData;
        if (data.model_id) {
          setContainerStatuses(prev => ({
            ...prev,
            [data.model_id]: data
          }));
        }
      } else if (message.type === "system_status") {
        const data = message.data;
        setGpuAvailable(data.gpu_available || false);
        setDockerAvailable(data.docker_available !== false);
        setProcessingDevice(data.gpu_available ? "Docker container (GPU)" : "Docker container (CPU)");
      }
    });

    return () => {
      unsubscribe();
    };
  }, []);

  // Handle model selection
  const handleModelSelect = async (modelId: string) => {
    try {
      setIsLoading(true);
      await apiClient.setModel(modelId);
      setCurrentModel(modelId);
    } catch (error) {
      console.error(`Failed to set model to ${modelId}:`, error);
    } finally {
      setIsLoading(false);
    }
  };

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

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Models & Dictation Engine</h1>
        <p className="page-description">
          Configure AI models, language settings, and processing options for
          speech recognition. Models are running in Docker containers.
        </p>
        {!dockerAvailable && (
          <div className="warning-banner" style={{ 
            backgroundColor: 'var(--warning-bg)', 
            padding: '12px', 
            borderRadius: 'var(--border-radius)', 
            marginTop: '16px',
            border: '1px solid var(--warning-border)'
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <AlertCircle size={16} color="var(--warning-yellow)" />
              <span style={{ color: 'var(--warning-text)' }}>
                Docker is not available. Model functionality may be limited.
              </span>
            </div>
          </div>
        )}
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
                  {model.id}
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
            <output className="device-info" style={{
              padding: '8px 12px',
              backgroundColor: 'var(--secondary-bg)',
              borderRadius: 'var(--border-radius)',
              display: 'flex',
              alignItems: 'center',
              gap: '8px'
            }}>
              <Container size={16} color="var(--secondary-text)" />
              <span>{processingDevice}</span>
              {gpuAvailable && (
                <span style={{ 
                  marginLeft: '8px', 
                  padding: '2px 6px', 
                  backgroundColor: 'var(--success-bg)', 
                  color: 'var(--success-green)', 
                  borderRadius: '4px',
                  fontSize: '12px'
                }}>
                  GPU
                </span>
              )}
            </output>
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
            <button
              type="button"
              className={`toggle-switch ${realTimeProcessing ? "active" : ""}`}
              onClick={() => setRealTimeProcessing(!realTimeProcessing)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  setRealTimeProcessing(!realTimeProcessing);
                }
              }}
              aria-pressed={realTimeProcessing}
              aria-label="Toggle real-time processing"
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="section-title">Available Models</h2>

        {isLoading ? (
          <div className="setting-row">
            <div className="setting-info">
              <h3 className="setting-label">Loading models...</h3>
            </div>
          </div>
        ) : (
          availableModels.map((model) => {
            const containerStatus = containerStatuses[model.id];
            const getStatusIcon = () => {
              if (containerStatus?.status === "running") {
                return <CheckCircle size={16} color="var(--success-green)" />;
              } else if (containerStatus?.status === "starting") {
                return <Clock size={16} color="var(--warning-yellow)" />;
              } else if (model.ready) {
                return <CheckCircle size={16} color="var(--success-green)" />;
              } else {
                return <AlertCircle size={16} color="var(--secondary-text)" />;
              }
            };

            const getStatusText = () => {
              if (containerStatus?.status === "running") {
                return `Container running${containerStatus.gpu_allocated ? " (GPU)" : " (CPU)"}`;
              } else if (containerStatus?.status === "starting") {
                return "Container starting...";
              } else if (model.ready) {
                return "Ready to use";
              } else {
                return "Not ready";
              }
            };

            return (
              <div key={model.id} className="setting-row">
                <div className="setting-info">
                  <div
                    style={{ display: "flex", alignItems: "center", gap: "8px" }}
                  >
                    {getStatusIcon()}
                    <h3 className="setting-label">{model.id}</h3>
                    {containerStatus?.gpu_allocated && (
                      <span style={{ 
                        padding: '2px 6px', 
                        backgroundColor: 'var(--success-bg)', 
                        color: 'var(--success-green)', 
                        borderRadius: '4px',
                        fontSize: '12px'
                      }}>
                        GPU
                      </span>
                    )}
                  </div>
                  <p className="setting-description">
                    Status: {getStatusText()}
                    {containerStatus?.image && (
                      <span style={{ marginLeft: '8px', fontSize: '12px', color: 'var(--secondary-text)' }}>
                        Image: {containerStatus.image}
                      </span>
                    )}
                  </p>
                </div>
                <div className="setting-control">
                  {model.ready || containerStatus?.status === "running" ? (
                    <button
                      type="button"
                      className="button"
                      onClick={() => handleModelSelect(model.id)}
                      disabled={currentModel === model.id || isLoading}
                    >
                      {currentModel === model.id ? "Selected" : "Select"}
                    </button>
                  ) : (
                    <button type="button" className="button" disabled>
                      {containerStatus?.status === "starting" ? (
                        <>
                          <Clock size={16} style={{ marginRight: "6px" }} />
                          Starting...
                        </>
                      ) : (
                        <>
                          <Download size={16} style={{ marginRight: "6px" }} />
                          Not Available
                        </>
                      )}
                    </button>
                  )}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};

export default ModelsPage;