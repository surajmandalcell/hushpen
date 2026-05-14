import React, { useState } from 'react';
import { ContentHeader } from '../components/SpokenlyUI';

export const History: React.FC = () => {
  const historyItems = [
    {
      title: "First I'd like to work on the 3s on route. Need my great underscore scores. So there are 3 files.",
      type: 'Dictation',
      time: '1 minute ago',
      duration: '30 seconds'
    },
    {
      title: 'In kaboop.',
      type: 'Dictation',
      time: '3d ago',
      duration: '2 seconds'
    },
    {
      title: '...taps on random paying and blah blah blah.',
      type: 'Dictation',
      time: '4d ago',
      duration: '3 seconds'
    }
  ];

  const [activeFilter, setActiveFilter] = useState('All');

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <ContentHeader
          title="History"
        />
        <div className="flex items-center gap-4">
          <div className="flex items-center bg-[#232425] border border-gray-600 rounded-lg p-0.5">
            {['All', 'Dictation', 'Files', 'Journal'].map(filter => (
              <button
                key={filter}
                onClick={() => setActiveFilter(filter)}
                className={`px-3 py-1 text-sm rounded-md transition-colors ${
                  activeFilter === filter
                    ? 'bg-gray-500/50 text-white'
                    : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                {filter}
              </button>
            ))}
          </div>
          <button className="text-sm text-gray-400 hover:text-gray-200">Settings</button>
        </div>
      </div>

      <input
        type="search"
        placeholder="Search"
        className="w-full bg-[#232425] border border-gray-600 rounded-md px-3 py-1.5 text-sm text-white placeholder-gray-500 focus:ring-blue-500 focus:border-blue-500 mb-4"
      />

      <div className="space-y-2">
        {historyItems.map((item, i) => (
          <div key={i} className="bg-[#232425] border border-gray-700 hover:border-gray-600 rounded-lg p-3">
            <p className="text-white text-sm">{item.title}</p>
            <div className="flex items-center gap-3 text-xs text-gray-400 mt-2">
              <span>{item.type}</span>
              <span className="w-1 h-1 bg-gray-600 rounded-full"></span>
              <span>{item.time}</span>
              <span className="w-1 h-1 bg-gray-600 rounded-full"></span>
              <span>{item.duration}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
