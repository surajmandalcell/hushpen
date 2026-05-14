import React from 'react';
import { ContentHeader } from '../components/SpokenlyUI';

interface StatMeterProps {
  label: string;
  level: number;
}

const StatMeter: React.FC<StatMeterProps> = ({ label, level }) => (
  <div className="flex items-center gap-1">
    <span className="text-xs text-gray-400">{label}</span>
    <div className="flex gap-0.5">
      {[...Array(5)].map((_, i) => (
        <div key={i} className={`w-2 h-2 rounded-sm ${i < level ? 'bg-green-500' : 'bg-gray-600'}`}></div>
      ))}
    </div>
  </div>
);

interface ModelTagProps {
  text: string;
}

const ModelTag: React.FC<ModelTagProps> = ({ text }) => (
  <span className="text-xs bg-[#232425] text-gray-300 px-2 py-0.5 rounded-md">{text}</span>
);

export const DictationModels: React.FC = () => {
  const models = [
    {
      name: "NVIDIA Parakeet TDT 0.6B V3",
      accuracy: 5,
      speed: 4,
      size: "496 MB",
      tags: ["Multilingual", "Local"],
      bestFor: "Perfect for private transcription or offline use."
    },
    {
      name: "Whisper Large V3 Turbo (Quantized)",
      accuracy: 4,
      speed: 4,
      size: "1.5 GB",
      tags: ["Multilingual", "Local"],
      bestFor: "Compressed model delivers excellent accuracy while using significantly less memory than full-size models."
    },
    {
      name: "Distil-Whisper Large V3.5 (English Only)",
      accuracy: 4,
      speed: 5,
      size: "1.5 GB",
      tags: ["English", "Local"],
      bestFor: "The newest Distil-Whisper model trained on 4x more diverse data for enhanced robustness.",
      isOptimized: true
    },
    {
      name: "Distil-Whisper Medium (English Only)",
      accuracy: 4,
      speed: 4,
      size: "794 MB",
      tags: ["English", "Local"],
      bestFor: "Works well for English dictation with almost immediate output."
    },
    {
      name: "Whisper Tiny (English)",
      accuracy: 2,
      speed: 5,
      size: "75 MB",
      tags: ["English", "Local"],
      bestFor: "For instant voice notes when speed matters. Smallest model gives quick results but limited accuracy.",
      inUse: true
    }
  ];

  return (
    <div>
      <ContentHeader
        title="Dictation Models"
        subtitle="Choose from various dictation models - from cloud-based options to local models that work offline."
      />

      <div className="my-4 flex items-center gap-2 flex-wrap">
        <button className="text-sm text-gray-200 bg-[#232425] border border-gray-600 rounded-full px-4 py-1.5">
          All
        </button>
        <button className="text-sm text-gray-400 hover:text-gray-200 hover:bg-[#232425]/50 rounded-full px-4 py-1.5">
          Online
        </button>
        <button className="text-sm text-gray-400 hover:text-gray-200 hover:bg-[#232425]/50 rounded-full px-4 py-1.5">
          Local
        </button>
      </div>

      <div className="space-y-3">
        {models.map(model => (
          <div
            key={model.name}
            className={`p-4 rounded-lg border transition-colors ${
              model.inUse
                ? 'bg-blue-600/10 border-blue-500'
                : 'bg-[#232425] border-gray-700 hover:border-gray-600'
            }`}
          >
            <div className="flex justify-between items-start">
              <div>
                <h3 className="font-semibold text-white">{model.name}</h3>
                <p className="text-xs text-gray-400 mt-1 max-w-md">{model.bestFor}</p>
              </div>
              {model.isOptimized ? (
                <button className="text-sm bg-blue-600 text-white font-semibold px-4 py-2 rounded-md hover:bg-blue-700 transition-colors">
                  Download
                </button>
              ) : model.inUse ? (
                <div className="text-sm bg-green-500/20 border border-green-500/50 text-green-300 font-semibold px-4 py-2 rounded-md">
                  Now Using
                </div>
              ) : (
                <button className="text-sm bg-[#3a3b3d] text-white font-semibold px-4 py-2 rounded-md hover:bg-gray-600 transition-colors">
                  Download
                </button>
              )}
            </div>
            <div className="mt-3 flex items-center gap-4 flex-wrap">
              <StatMeter label="Accuracy" level={model.accuracy} />
              <StatMeter label="Speed" level={model.speed} />
              <span className="text-xs text-gray-400">{model.size}</span>
              {model.tags.map(tag => (
                <ModelTag key={tag} text={tag} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};