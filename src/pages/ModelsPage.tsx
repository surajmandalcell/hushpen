import { useState, useEffect, type FC } from "react";
import { Download, CheckCircle, Clock, AlertCircle, Container } from "lucide-react";
import { apiClient, Model } from "../services/api";
import { webSocketService, ContainerStatusData } from "../services/websocket";
import { PalPanelHeader, PalText, PalCard, PalButton, PalSelect } from "../components/palantirui";
import { TacticalToggle } from "../components/TacticalToggle";

const ModelsPage: FC = () => {
  const [currentModel, setCurrentModel] = useState("whisper-base");
  const [language, setLanguage] = useState("auto");
  const [processingDevice, setProcessingDevice] = useState("Docker container");
  const [realTimeProcessing, setRealTimeProcessing] = useState(true);
  const [availableModels, setAvailableModels] = useState<Model[]>([]);
  const [containerStatuses, setContainerStatuses] = useState<Record<string, ContainerStatusData>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [dockerAvailable, setDockerAvailable] = useState(true);
  const [gpuAvailable, setGpuAvailable] = useState(false);

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

    const unsubscribe = webSocketService.subscribe((message) => {
      if (message.type === "container_status") {
        const data = message.data as ContainerStatusData;
        const { model_id } = data;
        if (!model_id) {
          return;
        }

        setContainerStatuses(prev => ({
          ...prev,
          [model_id]: data
        }));
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
    <div className="space-y-6">
      <PalPanelHeader
        title="Models & Dictation Engine"
        subtitle="Configure AI models, language settings, and processing options for speech recognition. Models are running in Docker containers."
        withBorder={false}
      />

      {!dockerAvailable && (
        <PalCard variant="default" padding="md" className="border-l-4 border-l-palantir-accent-orange">
          <div className="flex items-center gap-2">
            <AlertCircle size={16} className="text-palantir-accent-orange" />
            <PalText size="sm" className="text-palantir-accent-orange">
              Docker is not available. Model functionality may be limited.
            </PalText>
          </div>
        </PalCard>
      )}

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-6">
        <PalText size="lg" weight="semibold">Model Configuration</PalText>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Current Model</PalText>
            <PalText size="sm" variant="muted">
              Select the AI model to use for speech recognition
            </PalText>
          </div>
          <PalSelect
            value={currentModel}
            onChange={(e) => setCurrentModel(e.target.value)}
          >
            {availableModels.map((model) => (
              <option key={model.id} value={model.id}>
                {model.id}
              </option>
            ))}
          </PalSelect>
        </div>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Language</PalText>
            <PalText size="sm" variant="muted">
              Primary language for speech recognition
            </PalText>
          </div>
          <PalSelect
            value={language}
            onChange={(e) => setLanguage(e.target.value)}
          >
            {languages.map((lang) => (
              <option key={lang.id} value={lang.id}>
                {lang.name}
              </option>
            ))}
          </PalSelect>
        </div>

        <div className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700">
          <div>
            <PalText weight="medium">Processing Device</PalText>
            <PalText size="sm" variant="muted">
              Hardware to use for AI model processing
            </PalText>
          </div>
          <div className="flex items-center gap-2 px-3 py-2 bg-palantir-zinc-100 dark:bg-palantir-zinc-800 rounded-pal">
            <Container size={16} className="text-palantir-zinc-500 dark:text-palantir-zinc-400" />
            <PalText size="sm">{processingDevice}</PalText>
            {gpuAvailable && (
              <span className="px-2 py-1 bg-palantir-accent-green bg-opacity-20 text-palantir-accent-green rounded text-xs">
                GPU
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center justify-between py-3">
          <div>
            <PalText weight="medium">Real-time Processing</PalText>
            <PalText size="sm" variant="muted">
              Enable real-time speech recognition for live transcription
            </PalText>
          </div>
          <TacticalToggle
            checked={realTimeProcessing}
            onChange={(checked) => setRealTimeProcessing(checked)}
            size="md"
          />
        </div>
      </PalCard>

      <PalCard variant="default" padding="lg" withGlow={true} withCornerMarkers={true} className="space-y-4">
        <PalText size="lg" weight="semibold">Available Models</PalText>

        {isLoading ? (
          <div className="py-4">
            <PalText variant="muted">Loading models...</PalText>
          </div>
        ) : (
          <div className="space-y-3">
            {availableModels.map((model) => {
              const containerStatus = containerStatuses[model.id];
              const getStatusIcon = () => {
                if (containerStatus?.status === "running") {
                  return <CheckCircle size={16} className="text-palantir-accent-green" />;
                } else if (containerStatus?.status === "starting") {
                  return <Clock size={16} className="text-palantir-accent-orange" />;
                } else if (model.ready) {
                  return <CheckCircle size={16} className="text-palantir-accent-green" />;
                } else {
                  return <AlertCircle size={16} className="text-palantir-zinc-400" />;
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
                <div key={model.id} className="flex items-center justify-between py-3 border-b border-palantir-zinc-200 dark:border-palantir-zinc-700 last:border-0">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      {getStatusIcon()}
                      <PalText weight="medium">{model.id}</PalText>
                      {containerStatus?.gpu_allocated && (
                        <span className="px-2 py-1 bg-palantir-accent-green bg-opacity-20 text-palantir-accent-green rounded text-xs">
                          GPU
                        </span>
                      )}
                    </div>
                    <PalText size="sm" variant="muted">
                      Status: {getStatusText()}
                      {containerStatus?.image && (
                        <span className="ml-2">• Image: {containerStatus.image}</span>
                      )}
                    </PalText>
                  </div>
                  <div>
                    {model.ready || containerStatus?.status === "running" ? (
                      <PalButton
                        variant={currentModel === model.id ? "secondary" : "primary"}
                        size="sm"
                        onClick={() => handleModelSelect(model.id)}
                        disabled={currentModel === model.id || isLoading}
                      >
                        {currentModel === model.id ? "Selected" : "Select"}
                      </PalButton>
                    ) : (
                      <PalButton variant="ghost" size="sm" disabled>
                        {containerStatus?.status === "starting" ? (
                          <div className="flex items-center gap-2">
                            <Clock size={14} />
                            <span>Starting...</span>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <Download size={14} />
                            <span>Not Available</span>
                          </div>
                        )}
                      </PalButton>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </PalCard>
    </div>
  );
};

export default ModelsPage;
