import React from 'react';
import { AppLogoMark } from '../components/icons';

export const About: React.FC = () => (
  <div>
    <div className="mt-8 flex flex-col items-center text-center max-w-lg mx-auto">
      <div className="size-24 rounded-2xl bg-[#f6f4ef] flex items-center justify-center text-[#26343b] mb-6">
        <AppLogoMark className="size-20" title="ASR Pro" />
      </div>
      <h3 className="text-3xl font-semibold text-white">ASR Pro</h3>
      <p className="text-gray-400 mt-1">Version 0.1.0</p>
      <p className="text-gray-300 mt-6">
        A modern dictation and transcription tool for fast, accurate, and private voice-to-text conversion.
      </p>
      <div className="mt-8 flex items-center gap-6">
        <button className="text-blue-400 hover:underline text-sm">Official Website</button>
        <button className="text-blue-400 hover:underline text-sm">View License</button>
        <button className="text-blue-400 hover:underline text-sm">Privacy Policy</button>
      </div>
      <p className="text-xs text-gray-500 mt-12">© 2026 Suraj Mandal. All rights reserved.</p>
    </div>
  </div>
);
