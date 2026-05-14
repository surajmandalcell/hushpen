import React from 'react';
import { ContentHeader } from '../components/SpokenlyUI';

export const AiPrompts: React.FC = () => (
  <div>
    <ContentHeader
      title="AI Prompts"
      subtitle="Easily improve your spoken text using AI-powered prompts."
    />
    <div className="mt-8 max-w-md mx-auto">
      <h3 className="text-lg font-semibold text-center text-white">How it works:</h3>
      <div className="mt-6 space-y-6">
        <div className="flex items-start gap-4">
          <div className="w-8 h-8 rounded-full bg-[#3a3b3d] flex items-center justify-center text-white font-bold flex-shrink-0">
            1
          </div>
          <div>
            <h4 className="font-semibold text-white">Step 1: Activate & Dictate</h4>
            <p className="text-sm text-gray-400">Use shortcut keys or speak in chosen apps</p>
          </div>
        </div>
        <div className="flex items-start gap-4">
          <div className="w-8 h-8 rounded-full bg-[#3a3b3d] flex items-center justify-center text-white font-bold flex-shrink-0">
            2
          </div>
          <div>
            <h4 className="font-semibold text-white">Step 2: AI Enhancement</h4>
            <p className="text-sm text-gray-400">Your speech is processed with your chosen prompt</p>
          </div>
        </div>
        <div className="flex items-start gap-4">
          <div className="w-8 h-8 rounded-full bg-[#3a3b3d] flex items-center justify-center text-white font-bold flex-shrink-0">
            3
          </div>
          <div>
            <h4 className="font-semibold text-white">Step 3: Auto-Typed Result</h4>
            <p className="text-sm text-gray-400">The AI-enhanced text is typed automatically</p>
          </div>
        </div>
      </div>
      <div className="mt-10 flex flex-col items-center gap-4">
        <button className="bg-blue-600 hover:bg-blue-700 text-white font-semibold py-2 px-6 rounded-md">
          Edit Main Prompt
        </button>
        <button className="text-blue-400 hover:underline text-sm">Create New Prompt</button>
      </div>
    </div>
  </div>
);