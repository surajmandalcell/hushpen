# ASR Pro PalantirUI Implementation Guide

## Getting Started

This guide provides detailed code examples and implementation steps for transitioning ASR Pro from macOS-style to PalantirUI design.

## Phase 1: Foundation Implementation

### 1.1 Update Tailwind Configuration

First, update your `tailwind.config.ts` with the PalantirUI theme:

```typescript
// tailwind.config.ts
import type { Config } from "tailwindcss";
import forms from "@tailwindcss/forms";
import typography from "@tailwindcss/typography";

const config: Config = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx,css,html,vue,svelte}"],
  theme: {
    extend: {
      colors: {
        // Remove existing macos colors and replace with palantir
        palantir: {
          // Zinc/gray base palette
          zinc: {
            50: '#fafafa',
            100: '#f4f4f5',
            200: '#e4e4e7',
            300: '#d4d4d8',
            400: '#a1a1aa',
            500: '#71717a',
            600: '#52525b',
            700: '#3f3f46',
            800: '#27272a',
            900: '#18181b',
            950: '#09090b',
          },
          // Transparent layers
          layer: {
            1: 'rgba(250, 250, 250, 0.05)',
            2: 'rgba(250, 250, 250, 0.1)',
            3: 'rgba(250, 250, 250, 0.15)',
            4: 'rgba(250, 250, 250, 0.2)',
            5: 'rgba(250, 250, 250, 0.25)',
          },
          // Accent colors adapted for ASR
          accent: {
            blue: '#3b82f6',
            cyan: '#06b6d4',
            green: '#10b981',
            orange: '#f97316',
            red: '#ef4444',
          },
          // Glow effects
          glow: {
            subtle: '0 0 20px rgba(59, 130, 246, 0.15)',
            medium: '0 0 40px rgba(59, 130, 246, 0.25)',
            strong: '0 0 60px rgba(59, 130, 246, 0.35)',
          }
        }
      },
      fontFamily: {
        inter: ['Inter', 'system-ui', 'sans-serif'],
        ibm: ['IBM Plex Mono', 'Consolas', 'monospace'],
        kaisei: ['Kaisei Tokumin', 'serif'],
      },
      borderRadius: {
        'pal-sm': '2px',
        'pal': '4px',
        'pal-lg': '6px',
        'pal-xl': '8px',
      },
      boxShadow: {
        'pal-glow-subtle': '0 0 20px rgba(59, 130, 246, 0.15)',
        'pal-glow-medium': '0 0 40px rgba(59, 130, 246, 0.25)',
        'pal-glow-strong': '0 0 60px rgba(59, 130, 246, 0.35)',
        'pal-inset': 'inset 0 2px 4px 0 rgba(0, 0, 0, 0.06)',
      },
      backdropBlur: {
        'pal': '8px',
        'pal-lg': '16px',
        'pal-xl': '24px',
      },
      animation: {
        'pal-fade-in': 'palFadeIn 0.2s ease-out',
        'pal-slide-up': 'palSlideUp 0.3s ease-out',
        'pal-slide-down': 'palSlideDown 0.3s ease-out',
        'pal-glow-pulse': 'palGlowPulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'pal-border-draw': 'palBorderDraw 0.6s ease-out',
      },
      keyframes: {
        palFadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        palSlideUp: {
          '0%': { transform: 'translateY(10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        palSlideDown: {
          '0%': { transform: 'translateY(-10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        palGlowPulse: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
        palBorderDraw: {
          '0%': { strokeDashoffset: '1000' },
          '100%': { strokeDashoffset: '0' },
        },
      },
    },
  },
  plugins: [forms, typography],
  darkMode: "class",
};

export default config;
```

### 1.2 Create PalantirUI Base CSS

Create a new CSS file `src/styles/palantirui.css`:

```css
/* src/styles/palantirui.css */
@tailwind base;
@tailwind components;
@tailwind utilities;

/* PalantirUI Base Styles */
@layer base {
  * {
    @apply box-border;
  }

  html, body {
    @apply font-inter text-palantir-zinc-900 bg-palantir-zinc-50;
    @apply dark:text-palantir-zinc-100 dark:bg-palantir-zinc-950;
    @apply transition-colors duration-200;
  }

  body {
    @apply overflow-hidden;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
  }

  input, button, select, textarea {
    @apply font-inter;
  }
}

/* PalantirUI Component Styles */
@layer components {
  /* Base layer styles */
  .pal-layer-1 { 
    background: var(--tw-gradient-from);
    backdrop-filter: blur(8px);
    @apply bg-palantir-layer-1;
  }
  
  .pal-layer-2 { 
    background: var(--tw-gradient-from);
    backdrop-filter: blur(12px);
    @apply bg-palantir-layer-2;
  }
  
  .pal-layer-3 { 
    background: var(--tw-gradient-from);
    backdrop-filter: blur(16px);
    @apply bg-palantir-layer-3;
  }

  /* Corner markers */
  .pal-corner-marker {
    @apply absolute w-3 h-3 border-2 border-palantir-accent-blue opacity-70;
  }

  .pal-corner-marker-tl {
    @apply top-0 left-0 border-r-0 border-b-0;
  }

  .pal-corner-marker-tr {
    @apply top-0 right-0 border-l-0 border-b-0;
  }

  .pal-corner-marker-bl {
    @apply bottom-0 left-0 border-r-0 border-t-0;
  }

  .pal-corner-marker-br {
    @apply bottom-0 right-0 border-l-0 border-t-0;
  }

  /* Glow effects */
  .pal-glow-subtle { @apply shadow-pal-glow-subtle; }
  .pal-glow-medium { @apply shadow-pal-glow-medium; }
  .pal-glow-strong { @apply shadow-pal-glow-strong; }

  /* Geometric precision */
  .pal-geometric-border {
    @apply border border-palantir-zinc-700 relative;
  }

  .pal-geometric-border::before {
    content: '';
    @apply absolute inset-0 border border-palantir-zinc-600 opacity-50 pointer-events-none;
  }

  /* Depth system */
  .pal-depth-1 { 
    @apply z-10 bg-palantir-layer-1;
    backdrop-filter: blur(4px);
  }

  .pal-depth-2 { 
    @apply z-20 bg-palantir-layer-2;
    backdrop-filter: blur(8px);
  }

  .pal-depth-3 { 
    @apply z-30 bg-palantir-layer-3;
    backdrop-filter: blur(12px);
  }

  .pal-depth-4 { 
    @apply z-40 bg-palantir-layer-4;
    backdrop-filter: blur(16px);
  }

  .pal-depth-5 { 
    @apply z-50 bg-palantir-layer-5;
    backdrop-filter: blur(20px);
  }
}

/* PalantirUI Utility Classes */
@layer utilities {
  .pal-text-gradient {
    background: linear-gradient(135deg, #3b82f6 0%, #06b6d4 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .pal-border-gradient {
    background: linear-gradient(135deg, #3b82f6 0%, #06b6d4 100%);
    padding: 1px;
  }
}
```

### 1.3 Create Base Components

Create a new directory `src/components/palantirui/` and start with base components:

```typescript
// src/components/palantirui/PalButton.tsx
import React from "react";
import { cn } from "../../utils/cn";

export interface PalButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  variant?: "primary" | "secondary" | "destructive" | "ghost";
  size?: "small" | "medium" | "large";
  disabled?: boolean;
  className?: string;
  type?: "button" | "submit" | "reset";
  fullWidth?: boolean;
}

const PalButton: React.FC<PalButtonProps> = ({
  children,
  onClick,
  variant = "primary",
  size = "medium",
  disabled = false,
  className = "",
  type = "button",
  fullWidth = false,
}) => {
  const baseClasses = "font-inter font-medium transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-palantir-accent-blue focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed relative overflow-hidden group";

  const variantClasses = {
    primary: "bg-palantir-accent-blue text-white hover:bg-blue-600 pal-glow-subtle hover:pal-glow-medium",
    secondary: "bg-palantir-zinc-200 text-palantir-zinc-900 hover:bg-palantir-zinc-300 dark:bg-palantir-zinc-700 dark:text-palantir-zinc-100 dark:hover:bg-palantir-zinc-600",
    destructive: "bg-palantir-accent-red text-white hover:bg-red-600",
    ghost: "bg-transparent text-palantir-zinc-700 hover:bg-palantir-zinc-100 dark:text-palantir-zinc-300 dark:hover:bg-palantir-zinc-800",
  };

  const sizeClasses = {
    small: "px-3 py-1.5 text-sm rounded-pal-sm",
    medium: "px-4 py-2 text-sm rounded-pal",
    large: "px-6 py-3 text-base rounded-pal-lg",
  };

  const widthClasses = fullWidth ? "w-full" : "";

  const buttonClasses = cn(
    baseClasses,
    variantClasses[variant],
    sizeClasses[size],
    widthClasses,
    className
  );

  return (
    <button
      type={type}
      className={buttonClasses}
      onClick={onClick}
      disabled={disabled}
    >
      <span className="relative z-10 flex items-center justify-center">
        {children}
      </span>
      {/* Corner markers for primary variant */}
      {variant === "primary" && (
        <>
          <div className="pal-corner-marker pal-corner-marker-tl" />
          <div className="pal-corner-marker pal-corner-marker-tr" />
          <div className="pal-corner-marker pal-corner-marker-bl" />
          <div className="pal-corner-marker pal-corner-marker-br" />
        </>
      )}
    </button>
  );
};

export default PalButton;
```

```typescript
// src/components/palantirui/PalInput.tsx
import React, { forwardRef } from "react";
import { cn } from "../../utils/cn";

export interface PalInputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: string;
  error?: string;
  size?: "small" | "medium" | "large";
  variant?: "default" | "search" | "rounded";
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
  fullWidth?: boolean;
}

const PalInput = forwardRef<HTMLInputElement, PalInputProps>(
  (
    {
      label,
      error,
      size = "medium",
      variant = "default",
      leftIcon,
      rightIcon,
      fullWidth = false,
      className = "",
      disabled = false,
      ...props
    },
    ref
  ) => {
    const containerClasses = cn(
      "space-y-2",
      fullWidth ? "w-full" : "",
      disabled ? "opacity-50" : ""
    );

    const sizeClasses = {
      small: "px-2 py-1 text-sm",
      medium: "px-3 py-2 text-sm",
      large: "px-4 py-3 text-base"
    };

    const variantClasses = {
      default: "rounded-pal",
      search: "rounded-full",
      rounded: "rounded-pal-lg"
    };

    const inputWrapperClasses = cn(
      "relative flex items-center pal-geometric-border pal-layer-2",
      sizeClasses[size],
      variantClasses[variant],
      "bg-palantir-zinc-50 dark:bg-palantir-zinc-900 border-palantir-zinc-700 dark:border-palantir-zinc-600",
      "focus-within:ring-2 focus-within:ring-palantir-accent-blue focus-within:border-transparent",
      "transition-all duration-200",
      error ? "border-palantir-accent-red focus-within:ring-palantir-accent-red" : "",
      className
    );

    return (
      <div className={containerClasses}>
        {label && (
          <label className="block text-sm font-medium text-palantir-zinc-700 dark:text-palantir-zinc-300">
            {label}
          </label>
        )}
        <div className={inputWrapperClasses}>
          {leftIcon && (
            <div className="absolute left-3 text-palantir-zinc-500">
              {leftIcon}
            </div>
          )}
          <input
            ref={ref}
            className={cn(
              "flex-1 bg-transparent outline-none text-palantir-zinc-900 dark:text-palantir-zinc-100 placeholder-palantir-zinc-500",
              leftIcon ? "pl-8" : "",
              rightIcon ? "pr-8" : ""
            )}
            disabled={disabled}
            {...props}
          />
          {rightIcon && (
            <div className="absolute right-3 text-palantir-zinc-500">
              {rightIcon}
            </div>
          )}
        </div>
        {error && (
          <div className="text-sm text-palantir-accent-red">{error}</div>
        )}
      </div>
    );
  }
);

PalInput.displayName = "PalInput";

export default PalInput;
```

### 1.4 Create Utility Function

Create a utility function for className merging:

```typescript
// src/utils/cn.ts
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

## Phase 2: Layout System Implementation

### 2.1 Create Layout Components

```typescript
// src/components/palantirui/PalSidebar.tsx
import React from "react";
import { cn } from "../../utils/cn";

export interface PalSidebarProps {
  children: React.ReactNode;
  collapsed?: boolean;
  className?: string;
}

const PalSidebar: React.FC<PalSidebarProps> = ({
  children,
  collapsed = false,
  className = "",
}) => {
  const sidebarClasses = cn(
    "h-full pal-layer-2 border-r border-palantir-zinc-700 dark:border-palantir-zinc-600 transition-all duration-300",
    "flex flex-col",
    collapsed ? "w-16" : "w-64",
    className
  );

  return (
    <div className={sidebarClasses}>
      <div className="relative">
        {/* Corner markers */}
        <div className="pal-corner-marker pal-corner-marker-tl" />
        <div className="pal-corner-marker pal-corner-marker-bl" />
      </div>
      {children}
    </div>
  );
};

export default PalSidebar;
```

```typescript
// src/components/palantirui/PalPanel.tsx
import React from "react";
import { cn } from "../../utils/cn";

export interface PalPanelProps {
  children: React.ReactNode;
  depth?: 1 | 2 | 3 | 4 | 5;
  className?: string;
  glow?: boolean;
}

const PalPanel: React.FC<PalPanelProps> = ({
  children,
  depth = 1,
  className = "",
  glow = false,
}) => {
  const panelClasses = cn(
    `pal-depth-${depth}`,
    "pal-geometric-border rounded-pal-lg p-6 transition-all duration-300",
    glow && "pal-glow-subtle hover:pal-glow-medium",
    className
  );

  return (
    <div className={panelClasses}>
      {/* Corner markers */}
      <div className="pal-corner-marker pal-corner-marker-tl" />
      <div className="pal-corner-marker pal-corner-marker-tr" />
      <div className="pal-corner-marker pal-corner-marker-bl" />
      <div className="pal-corner-marker pal-corner-marker-br" />
      {children}
    </div>
  );
};

export default PalPanel;
```

### 2.2 Update App Layout

Update your `App.tsx` to use the new layout system:

```typescript
// src/App.tsx
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { usePlatform } from "./services/platform";
import { useAccessibility } from "./services/accessibility";

// Import new PalantirUI components
import PalSidebar from "./components/palantirui/PalSidebar";
import PalPanel from "./components/palantirui/PalPanel";
import PalHeader from "./components/palantirui/PalHeader";

// Import existing components that will be refactored
import SystemTray from "./components/SystemTray";
import ToastContainer from "./components/ToastContainer";
import RecordingOverlay from "./components/RecordingOverlay";
import TrayNotificationList from "./components/TrayNotificationList";

// Hooks
import { useWindowState } from "./hooks/useWindowState";
import { useRecording } from "./services/recordingManager";
import { useTrayNotifications } from "./services/trayNotifications";
import { useWebSocket } from "./hooks/useWebSocket";
import { globalShortcutService } from "./services/globalShortcut";

function App() {
  const [activeSection, setActiveSection] = useState("general");
  const [isRecordingOverlayActive, setIsRecordingOverlayActive] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  
  const platformInfo = usePlatform();
  useAccessibility();
  useWindowState();

  const { state: recordingState } = useRecording();
  const { notifications } = useTrayNotifications();
  useWebSocket();

  const startRecording = () => {
    setIsRecordingOverlayActive(true);
  };

  // Initialize global shortcuts (existing code)
  useEffect(() => {
    const initializeShortcuts = async () => {
      try {
        await globalShortcutService.initialize();
        console.log("Global shortcuts initialized");
      } catch (error) {
        console.error("Failed to initialize global shortcuts:", error);
      }
    };

    initializeShortcuts();

    return () => {
      globalShortcutService.cleanup();
    };
  }, []);

  // Listen for global shortcut events (existing code)
  useEffect(() => {
    const unlistenStart = listen("recording-start", () => {
      setIsRecordingOverlayActive(true);
    });

    const unlistenStop = listen("recording-stop", () => {
      setIsRecordingOverlayActive(false);
    });

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, []);

  return (
    <div className={`h-screen w-screen overflow-hidden bg-palantir-zinc-50 dark:bg-palantir-zinc-950 transition-colors duration-200 ${platformInfo.getPlatformCSSClass()}`}>
      {/* Skip to content link for screen readers */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 px-4 py-2 bg-palantir-accent-blue text-white rounded-pal"
      >
        Skip to main content
      </a>

      <SystemTray />
      <ToastContainer />
      <RecordingOverlay
        isActive={isRecordingOverlayActive}
        isTranscribing={recordingState.isTranscribing}
        statusText={recordingState.statusText}
      />
      {notifications.length > 0 && <TrayNotificationList />}
      
      {/* New PalantirUI Layout Structure */}
      <div className="h-full flex flex-col">
        <PalHeader 
          onSidebarToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
          sidebarCollapsed={sidebarCollapsed}
        />
        
        <div className="flex flex-1 overflow-hidden" id="main-content">
          <PalSidebar collapsed={sidebarCollapsed}>
            {/* Sidebar content will be updated in next phase */}
            <div className="flex-1 px-3 py-4 space-y-1 scrollbar-pal overflow-y-auto">
              {/* Navigation items will be migrated here */}
            </div>
          </PalSidebar>
          
          <PalPanel depth={1} className="flex-1 m-4">
            {/* Content area will be updated in next phase */}
            <div className="h-full">
              {/* Existing content will be migrated here */}
            </div>
          </PalPanel>
        </div>
      </div>
    </div>
  );
}

export default App;
```

## Phase 3: Animation System Implementation

### 3.1 Create Animation Manager

```typescript
// src/utils/animations.ts
export interface AnimationOptions {
  keyframes: Keyframe[];
  duration?: number;
  easing?: string;
  fill?: FillMode;
}

export class PalantirAnimationManager {
  private animations: Map<string, Animation> = new Map();
  private rafId: number | null = null;

  createAnimation(element: Element, options: AnimationOptions, id?: string): Animation {
    const optimizedOptions: KeyframeAnimationOptions = {
      duration: options.duration || 300,
      easing: options.easing || 'cubic-bezier(0.4, 0.0, 0.2, 1)',
      fill: options.fill || 'forwards',
    };

    const animation = element.animate(options.keyframes, optimizedOptions);
    
    if (id) {
      this.animations.set(id, animation);
    }

    return animation;
  }

  createGlowAnimation(element: HTMLElement, intensity: 'subtle' | 'medium' | 'strong' = 'subtle'): Animation {
    const glowKeyframes = [
      { boxShadow: '0 0 20px rgba(59, 130, 246, 0.15)' },
      { boxShadow: '0 0 40px rgba(59, 130, 246, 0.25)' },
      { boxShadow: '0 0 20px rgba(59, 130, 246, 0.15)' },
    ];

    return this.createAnimation(element, {
      keyframes: glowKeyframes,
      duration: 2000,
      easing: 'ease-in-out',
    });
  }

  createBorderDrawAnimation(element: HTMLElement): Animation {
    const borderKeyframes = [
      { strokeDashoffset: '1000' },
      { strokeDashoffset: '0' },
    ];

    return this.createAnimation(element, {
      keyframes: borderKeyframes,
      duration: 600,
      easing: 'ease-out',
    });
  }

  cleanupAnimations() {
    this.animations.forEach(animation => animation.cancel());
    this.animations.clear();
    
    if (this.rafId) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
  }
}

export const animationManager = new PalantirAnimationManager();
```

### 3.2 Create Mouse Tracking Hook

```typescript
// src/hooks/useMouseTracking.ts
import { useState, useEffect, useCallback } from 'react';

export interface MousePosition {
  x: number;
  y: number;
}

export const useMouseTracking = () => {
  const [mousePosition, setMousePosition] = useState<MousePosition>({ x: 0, y: 0 });
  const [isMouseMoving, setIsMouseMoving] = useState(false);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    setMousePosition({ x: e.clientX, y: e.clientY });
    setIsMouseMoving(true);
  }, []);

  useEffect(() => {
    let timeoutId: NodeJS.Timeout;

    const handleMouseMoveThrottled = (e: MouseEvent) => {
      handleMouseMove(e);
      
      // Clear existing timeout
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      
      // Set new timeout to reset mouse moving state
      timeoutId = setTimeout(() => {
        setIsMouseMoving(false);
      }, 100);
    };

    document.addEventListener('mousemove', handleMouseMoveThrottled);

    return () => {
      document.removeEventListener('mousemove', handleMouseMoveThrottled);
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [handleMouseMove]);

  return { mousePosition, isMouseMoving };
};
```

## Phase 4: Component Migration Examples

### 4.1 Migrate Recording Overlay

```typescript
// src/components/palantirui/PalRecordingOverlay.tsx
import React, { useEffect, useState } from "react";
import { Mic, MicOff, Square, Loader2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useAudioRecording } from "../hooks/useAudioRecording";
import { useRecording } from "../services/recordingManager";
import { animationManager } from "../utils/animations";
import { cn } from "../utils/cn";

export interface PalRecordingOverlayProps {
  isActive: boolean;
  isTranscribing?: boolean;
  statusText?: string;
}

const PalRecordingOverlay: React.FC<PalRecordingOverlayProps> = ({
  isActive,
  isTranscribing = false,
  statusText = "Listening...",
}) => {
  const [showControls, setShowControls] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [isTranscribingLocal, setIsTranscribingLocal] = useState(false);
  const overlayRef = useRef<HTMLDivElement>(null);

  const {
    state: audioState,
    startRecording,
    stopRecording,
  } = useAudioRecording();

  const {
    state: recordingState,
    start,
    stop,
    cancel,
    transcribeFile,
  } = useRecording();

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (isActive && overlayRef.current) {
      // Create entrance animation
      animationManager.createAnimation(overlayRef.current, {
        keyframes: [
          { opacity: 0, transform: 'scale(0.9)' },
          { opacity: 1, transform: 'scale(1)' },
        ],
        duration: 300,
      });

      // Create glow animation
      const glowElement = overlayRef.current.querySelector('.pal-glow-container');
      if (glowElement) {
        animationManager.createGlowAnimation(glowElement as HTMLElement, 'medium');
      }
    }
  }, [isActive]);

  // Rest of the component logic remains similar to the original
  // but with PalantirUI styling

  const isRecording = audioState.isRecording || recordingState.isActive;

  if (!isRecording || !mounted) return null;

  return (
    <div
      ref={overlayRef}
      className={cn(
        "fixed inset-0 z-50 pal-layer-5 flex items-center justify-center",
        "backdrop-blur-pal-xl transition-all duration-300"
      )}
      role="dialog"
      aria-modal="true"
      aria-labelledby="recording-status"
      aria-describedby="recording-instructions"
    >
      <div className="pal-glow-container">
        <div className="pal-depth-3 pal-geometric-border rounded-pal-xl p-8 text-center max-w-md mx-4">
          {/* Corner markers */}
          <div className="pal-corner-marker pal-corner-marker-tl" />
          <div className="pal-corner-marker pal-corner-marker-tr" />
          <div className="pal-corner-marker pal-corner-marker-bl" />
          <div className="pal-corner-marker pal-corner-marker-br" />
          
          {/* Status indicator */}
          <div className="flex flex-col items-center space-y-4">
            <div
              className={cn(
                "flex items-center justify-center w-20 h-20 rounded-full transition-all duration-300",
                isTranscribing ? "bg-palantir-accent-orange" : "bg-palantir-accent-red",
                "pal-glow-medium animate-pal-glow-pulse"
              )}
            >
              {isTranscribing ? (
                <Loader2 size={32} className="text-white animate-spin" />
              ) : (
                <Mic size={32} className="text-white" />
              )}
            </div>

            <div className="text-center space-y-2">
              <h2 id="recording-status" className="text-2xl font-semibold text-palantir-zinc-900 dark:text-palantir-zinc-100">
                {isTranscribing ? "Transcribing..." : "Recording..."}
              </h2>
              <p className="text-palantir-zinc-600 dark:text-palantir-zinc-400 text-lg" aria-live="polite">
                {statusText}
              </p>
              <p className="text-3xl font-ibm font-bold text-palantir-accent-blue" aria-live="polite">
                {formatDuration(currentDuration)}
              </p>
            </div>
          </div>

          {/* Controls */}
          <div
            className={cn(
              "flex flex-col items-center space-y-4 transition-all duration-300",
              showControls ? "opacity-100" : "opacity-30"
            )}
          >
            <div className="flex space-x-4">
              {isTranscribing ? (
                <button
                  className="px-4 py-2 bg-palantir-accent-red text-white rounded-pal flex items-center space-x-2 hover:bg-red-600 transition-colors"
                  onClick={handleStop}
                  aria-label="Stop transcription"
                >
                  <Square size={20} />
                  <span>Stop (Space)</span>
                </button>
              ) : (
                <button
                  className="px-4 py-2 bg-palantir-zinc-200 text-palantir-zinc-900 dark:bg-palantir-zinc-700 dark:text-palantir-zinc-100 rounded-pal flex items-center space-x-2 hover:bg-palantir-zinc-300 dark:hover:bg-palantir-zinc-600 transition-colors"
                  onClick={handleCancel}
                  aria-label="Cancel recording"
                >
                  <MicOff size={20} />
                  <span>Cancel (Esc)</span>
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PalRecordingOverlay;
```

## Testing and Validation

### 1. Component Testing

```typescript
// src/components/palantirui/__tests__/PalButton.test.tsx
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import PalButton from '../PalButton';

describe('PalButton', () => {
  it('renders with default props', () => {
    render(<PalButton>Test Button</PalButton>);
    const button = screen.getByRole('button', { name: 'Test Button' });
    expect(button).toBeInTheDocument();
    expect(button).toHaveClass('bg-palantir-accent-blue');
  });

  it('applies variant classes correctly', () => {
    render(<PalButton variant="secondary">Secondary Button</PalButton>);
    const button = screen.getByRole('button', { name: 'Secondary Button' });
    expect(button).toHaveClass('bg-palantir-zinc-200');
  });

  it('handles click events', () => {
    const handleClick = jest.fn();
    render(<PalButton onClick={handleClick}>Click me</PalButton>);
    
    fireEvent.click(screen.getByRole('button', { name: 'Click me' }));
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('shows corner markers for primary variant', () => {
    render(<PalButton variant="primary">Primary Button</PalButton>);
    const button = screen.getByRole('button', { name: 'Primary Button' });
    expect(button.querySelectorAll('.pal-corner-marker')).toHaveLength(4);
  });
});
```

### 2. Accessibility Testing

```typescript
// src/utils/accessibility.ts
import { axe, toHaveNoViolations } from 'jest-axe';

expect.extend(toHaveNoViolations);

export const testAccessibility = async (container: HTMLElement) => {
  const results = await axe(container);
  expect(results).toHaveNoViolations();
};
```

## Performance Optimization

### 1. Animation Performance

```typescript
// src/hooks/usePerformanceOptimizedAnimation.ts
import { useRef, useEffect } from 'react';

export const usePerformanceOptimizedAnimation = (
  elementRef: React.RefObject<HTMLElement>,
  animationKeyframes: Keyframe[],
  options: KeyframeAnimationOptions
) => {
  const animationRef = useRef<Animation | null>(null);

  useEffect(() => {
    if (!elementRef.current) return;

    // Use transform and opacity for better performance
    const optimizedKeyframes = animationKeyframes.map(keyframe => ({
      ...keyframe,
      // Ensure we're using performant properties
      transform: keyframe.transform || 'none',
      opacity: keyframe.opacity !== undefined ? keyframe.opacity : 1,
    }));

    animationRef.current = elementRef.current.animate(optimizedKeyframes, {
      ...options,
      // Use will-change for better performance
      composite: 'replace',
    });

    return () => {
      if (animationRef.current) {
        animationRef.current.cancel();
      }
    };
  }, [animationKeyframes, options]);

  return animationRef.current;
};
```

## Next Steps

1. **Implement Phase 1**: Update Tailwind config and create base CSS
2. **Create Base Components**: Implement PalButton, PalInput, PalToggle
3. **Implement Layout System**: Create PalSidebar, PalPanel, PalHeader
4. **Migrate Existing Components**: Gradually replace macOS components
5. **Add Animation System**: Implement mouse tracking and animations
6. **Test and Optimize**: Ensure performance and accessibility

This implementation guide provides the foundation for transforming ASR Pro's UI to the PalantirUI aesthetic while maintaining all existing functionality.