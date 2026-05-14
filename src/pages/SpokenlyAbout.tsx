import React from 'react';
import { SpokenlyIcon } from '../components/icons';

export const About: React.FC = () => (
  <div>
    <div className="mt-8 flex flex-col items-center text-center max-w-lg mx-auto">
      <div className="w-24 h-24 rounded-2xl bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-white mb-6">
        <SpokenlyIcon />
      </div>
      <h3 className="text-3xl font-bold text-white">Spokenly</h3>
      <p className="text-gray-400 mt-1">Version 2.13.3 (259)</p>
      <p className="text-gray-300 mt-6">
        A modern dictation and transcription tool for fast, accurate, and private voice-to-text conversion.
      </p>
      <div className="mt-8 flex items-center gap-6">
        <button className="text-blue-400 hover:underline text-sm">Official Website</button>
        <button className="text-blue-400 hover:underline text-sm">View License</button>
        <button className="text-blue-400 hover:underline text-sm">Privacy Policy</button>
      </div>
      <p className="text-xs text-gray-500 mt-12">© 2025 Spokenly Inc. All rights reserved.</p>
    </div>
  </div>
);