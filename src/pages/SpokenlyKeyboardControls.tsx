import React, { useState } from 'react';
import { ContentHeader, SettingsSection, ShortcutKey, Modal } from '../components/SpokenlyUI';
import { PlusIcon } from '../components/icons';

export const KeyboardControls: React.FC = () => {
  const [isModalOpen, setIsModalOpen] = useState(false);

  return (
    <div>
      <ContentHeader
        title="Keyboard Controls"
        subtitle="Choose your preferred keyboard keys for starting Spokenly. Press only these keys to begin recording."
      />
      <div className="divide-y divide-gray-700">
        <SettingsSection title="Recording Keys">
          <div className="flex items-center justify-between">
            <p className="text-sm text-gray-200">Activation Keys</p>
            <div className="flex items-center gap-2">
              <button className="text-sm text-gray-300 bg-[#3a3b3d] border border-gray-600 rounded-md px-3 py-1">
                Hold or Toggle
              </button>
              <ShortcutKey keys={['⌘', 'K']} />
              <button
                onClick={() => setIsModalOpen(true)}
                className="p-2 bg-[#3a3b3d] border border-gray-600 rounded-md hover:bg-gray-600"
              >
                <PlusIcon />
              </button>
            </div>
          </div>
        </SettingsSection>

        <SettingsSection title="Try Your Keys">
          <div className="p-4 bg-[#232425] border border-gray-700 rounded-lg text-center">
            <p className="text-gray-400">Click in the text box below first.</p>
          </div>
        </SettingsSection>
      </div>

      <Modal isVisible={isModalOpen} onClose={() => setIsModalOpen(false)}>
        <h3 className="text-lg font-bold">Record Keyboard Shortcut</h3>
        <p className="text-sm text-gray-400 mt-2 mb-4">
          Press the keys you want to use for your shortcut. The shortcut must include at least one modifier key (⌘, ⌃, ⌥, ⇧) and one regular key (except function keys).
        </p>
        <div className="my-4 p-4 bg-green-500/20 border border-green-500/50 rounded-lg">
          <p className="text-lg font-semibold text-green-300">Recording...</p>
        </div>
        <button
          onClick={() => setIsModalOpen(false)}
          className="w-full bg-[#3a3b3d] hover:bg-gray-600 text-white font-semibold py-2 rounded-md"
        >
          Cancel
        </button>
      </Modal>
    </div>
  );
};