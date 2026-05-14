import React from 'react';
import { ContentHeader } from '../components/SpokenlyUI';
import { UploadIcon } from '../components/icons';

export const TranscribeFile: React.FC = () => (
  <div>
    <ContentHeader
      title="Transcribe File"
    />
    <div className="h-full flex flex-col">
      <div className="flex-grow flex items-center justify-center p-4">
        <div className="w-full max-w-xl h-64 border-2 border-dashed border-gray-600 rounded-xl flex flex-col items-center justify-center text-center">
          <UploadIcon />
          <h3 className="text-lg font-semibold text-white mt-4">Drop your files here</h3>
          <p className="text-sm text-gray-400">MP3   WAV   M4A   FLAC   MP4   MOV</p>
        </div>
      </div>
    </div>
  </div>
);
