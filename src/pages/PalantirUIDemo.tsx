import React, { useState } from 'react';
import {
  PalButton,
  PalInput,
  PalCard,
  PalIcon,
  PalText,
  PalSidebar,
  PalSidebarItem,
  PalSidebarSection,
  PalHeader,
  PalHeaderTitle,
  PalHeaderActions,
  PalPanel,
  PalPanelHeader,
  PalPanelContent,
  PalPanelFooter,
  PalModal,
  PalModalHeader,
  PalModalContent,
  PalModalFooter,
} from '../components/palantirui';

const PalantirUIDemo: React.FC = () => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const [darkMode, setDarkMode] = useState(false);

  const toggleDarkMode = () => {
    setDarkMode(!darkMode);
    document.documentElement.classList.toggle('dark');
  };

  return (
    <div className={`min-h-screen bg-palantir-zinc-50 dark:bg-palantir-zinc-950 transition-colors duration-300`}>
      <div className="flex h-screen">
        {/* Sidebar */}
        <PalSidebar
          collapsed={sidebarCollapsed}
          width="md"
          className="border-r border-palantir-zinc-200 dark:border-palantir-zinc-700"
        >
          <PalSidebarSection title="Navigation" collapsed={sidebarCollapsed}>
            <PalSidebarItem
              icon={<PalIcon size="sm">🏠</PalIcon>}
              label="Home"
              active
              collapsed={sidebarCollapsed}
            />
            <PalSidebarItem
              icon={<PalIcon size="sm">📊</PalIcon>}
              label="Dashboard"
              collapsed={sidebarCollapsed}
            />
            <PalSidebarItem
              icon={<PalIcon size="sm">⚙️</PalIcon>}
              label="Settings"
              collapsed={sidebarCollapsed}
            />
          </PalSidebarSection>
          
          <PalSidebarSection title="Tools" collapsed={sidebarCollapsed}>
            <PalSidebarItem
              icon={<PalIcon size="sm">🎙️</PalIcon>}
              label="Recording"
              collapsed={sidebarCollapsed}
            />
            <PalSidebarItem
              icon={<PalIcon size="sm">📝</PalIcon>}
              label="Transcription"
              collapsed={sidebarCollapsed}
            />
          </PalSidebarSection>
        </PalSidebar>

        {/* Main Content */}
        <div className="flex-1 flex flex-col">
          {/* Header */}
          <PalHeader
            height="md"
            withWindowControls={true}
            className="border-b border-palantir-zinc-200 dark:border-palantir-zinc-700"
          >
            <PalHeaderTitle
              title="ASR Pro"
              subtitle="PalantirUI Demo"
            />
            <PalHeaderActions>
              <PalButton
                variant="ghost"
                size="sm"
                onClick={toggleDarkMode}
              >
                {darkMode ? '☀️' : '🌙'}
              </PalButton>
              <PalButton
                variant="secondary"
                size="sm"
                onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              >
                {sidebarCollapsed ? '→' : '←'}
              </PalButton>
            </PalHeaderActions>
          </PalHeader>

          {/* Content Area */}
          <div className="flex-1 p-6 overflow-auto pal-scrollbar">
            <div className="max-w-6xl mx-auto space-y-8">
              {/* Title Section */}
              <div>
                <PalText size="2xl" weight="bold" className="mb-2">
                  PalantirUI Component Library
                </PalText>
                <PalText variant="muted">
                  A sophisticated component library with zinc/gray color palette, transparent layers, and geometric precision.
                </PalText>
              </div>

              {/* Button Examples */}
              <PalPanel>
                <PalPanelHeader title="Buttons" />
                <PalPanelContent>
                  <div className="flex flex-wrap gap-4">
                    <PalButton variant="primary">Primary</PalButton>
                    <PalButton variant="secondary">Secondary</PalButton>
                    <PalButton variant="ghost">Ghost</PalButton>
                    <PalButton withGlow>With Glow</PalButton>
                    <PalButton withCornerMarkers>With Corners</PalButton>
                    <PalButton disabled>Disabled</PalButton>
                  </div>
                </PalPanelContent>
              </PalPanel>

              {/* Input Examples */}
              <PalPanel>
                <PalPanelHeader title="Inputs" />
                <PalPanelContent>
                  <div className="space-y-4 max-w-md">
                    <PalInput
                      placeholder="Default input"
                      value={inputValue}
                      onChange={(e) => setInputValue(e.target.value)}
                    />
                    <PalInput
                      placeholder="With glow"
                      withGlow
                    />
                    <PalInput
                      placeholder="With corner markers"
                      withCornerMarkers
                    />
                    <PalInput
                      placeholder="Error state"
                      error
                    />
                  </div>
                </PalPanelContent>
              </PalPanel>

              {/* Card Examples */}
              <PalPanel>
                <PalPanelHeader title="Cards" />
                <PalPanelContent>
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <PalCard>
                      <div className="p-4">
                        <PalText weight="semibold" className="mb-2">Default Card</PalText>
                        <PalText size="sm" variant="muted">
                          Basic card with default styling.
                        </PalText>
                      </div>
                    </PalCard>
                    
                    <PalCard variant="hover">
                      <div className="p-4">
                        <PalText weight="semibold" className="mb-2">Hover Card</PalText>
                        <PalText size="sm" variant="muted">
                          Card with hover effects.
                        </PalText>
                      </div>
                    </PalCard>
                    
                    <PalCard variant="glow">
                      <div className="p-4">
                        <PalText weight="semibold" className="mb-2">Glow Card</PalText>
                        <PalText size="sm" variant="muted">
                          Card with glow effects.
                        </PalText>
                      </div>
                    </PalCard>
                  </div>
                </PalPanelContent>
              </PalPanel>

              {/* Icon Examples */}
              <PalPanel>
                <PalPanelHeader title="Icons" />
                <PalPanelContent>
                  <div className="flex flex-wrap gap-4">
                    <PalIcon size="xs">🏠</PalIcon>
                    <PalIcon size="sm">📊</PalIcon>
                    <PalIcon size="md">⚙️</PalIcon>
                    <PalIcon size="lg">🎙️</PalIcon>
                    <PalIcon size="xl">📝</PalIcon>
                    <PalIcon variant="muted" size="md">🔇</PalIcon>
                    <PalIcon variant="accent" size="md">🔊</PalIcon>
                    <PalIcon withGlow size="md">✨</PalIcon>
                  </div>
                </PalPanelContent>
              </PalPanel>

              {/* Modal Examples */}
              <PalPanel>
                <PalPanelHeader title="Modal" />
                <PalPanelContent>
                  <PalButton onClick={() => setModalOpen(true)}>
                    Open Modal
                  </PalButton>
                </PalPanelContent>
              </PalPanel>

              {/* Text Examples */}
              <PalPanel>
                <PalPanelHeader title="Text" />
                <PalPanelContent>
                  <div className="space-y-2">
                    <PalText size="xs">Extra Small Text</PalText>
                    <PalText size="sm">Small Text</PalText>
                    <PalText size="base">Base Text</PalText>
                    <PalText size="lg">Large Text</PalText>
                    <PalText size="xl">Extra Large Text</PalText>
                    <PalText size="2xl">2X Large Text</PalText>
                    <PalText weight="normal">Normal Weight</PalText>
                    <PalText weight="medium">Medium Weight</PalText>
                    <PalText weight="semibold">Semibold Weight</PalText>
                    <PalText weight="bold">Bold Weight</PalText>
                    <PalText variant="muted">Muted Text</PalText>
                    <PalText variant="accent">Accent Text</PalText>
                    <PalText variant="gradient">Gradient Text</PalText>
                  </div>
                </PalPanelContent>
              </PalPanel>
            </div>
          </div>
        </div>
      </div>

      {/* Modal */}
      <PalModal
        isOpen={modalOpen}
        onClose={() => setModalOpen(false)}
        size="md"
        withGlow
      >
        <PalModalHeader
          title="Demo Modal"
          subtitle="This is a PalantirUI modal"
          onClose={() => setModalOpen(false)}
        />
        <PalModalContent>
          <PalText>
            This is a demonstration of the PalantirUI modal component. It features a backdrop blur,
            smooth animations, and proper accessibility features.
          </PalText>
        </PalModalContent>
        <PalModalFooter>
          <PalButton variant="ghost" onClick={() => setModalOpen(false)}>
            Cancel
          </PalButton>
          <PalButton onClick={() => setModalOpen(false)}>
            Confirm
          </PalButton>
        </PalModalFooter>
      </PalModal>
    </div>
  );
};

export default PalantirUIDemo;