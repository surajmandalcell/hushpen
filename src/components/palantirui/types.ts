// PalantirUI System Types

// Color Palette Types
export type PalantirColor = 
  | 'zinc-50' | 'zinc-100' | 'zinc-200' | 'zinc-300' | 'zinc-400' 
  | 'zinc-500' | 'zinc-600' | 'zinc-700' | 'zinc-800' | 'zinc-900' | 'zinc-950';

export type PalantirAccentColor = 
  | 'blue' | 'cyan' | 'green' | 'orange' | 'red';

export type PalantirLayer = 
  | 'layer-1' | 'layer-2' | 'layer-3' | 'layer-4' | 'layer-5';

// Size Types
export type PalantirSize = 
  | 'xs' | 'sm' | 'md' | 'lg' | 'xl';

// Variant Types
export type PalantirVariant = 
  | 'default' | 'primary' | 'secondary' | 'ghost' | 'muted' | 'accent';

// Animation Types
export type PalantirAnimation = 
  | 'pal-fade-in-up' | 'pal-fade-in' | 'pal-card-fade-in' 
  | 'pal-glow-pulse' | 'pal-corner-appear' | 'pal-border-glow'
  | 'pal-slide-up' | 'pal-scale-in';

// Depth Types
export type PalantirDepth = 
  | 'pal-depth-1' | 'pal-depth-2' | 'pal-depth-3' | 'pal-depth-4' | 'pal-depth-5';

// Component State Types
export interface PalantirUIState {
  isDarkMode: boolean;
  accentColor: PalantirAccentColor;
  animationsEnabled: boolean;
  reducedMotion: boolean;
}

// Theme Configuration
export interface PalantirUITheme {
  colors: {
    zinc: Record<PalantirColor, string>;
    accent: Record<PalantirAccentColor, string>;
    layer: Record<PalantirLayer, string>;
  };
  animations: Record<PalantirAnimation, string>;
  breakpoints: {
    sm: string;
    md: string;
    lg: string;
    xl: string;
  };
}

// Component Configuration
export interface PalantirUIConfig {
  theme: PalantirUITheme;
  state: PalantirUIState;
  components: {
    button: PalantirButtonConfig;
    input: PalantirInputConfig;
    card: PalantirCardConfig;
    sidebar: PalantirSidebarConfig;
    header: PalantirHeaderConfig;
    panel: PalantirPanelConfig;
    modal: PalantirModalConfig;
  };
}

// Individual Component Configurations
export interface PalantirButtonConfig {
  defaultVariant: PalantirVariant;
  defaultSize: PalantirSize;
  withGlow: boolean;
  withCornerMarkers: boolean;
  animations: {
    hover: boolean;
    active: boolean;
    focus: boolean;
  };
}

export interface PalantirInputConfig {
  defaultVariant: PalantirVariant;
  defaultSize: PalantirSize;
  withGlow: boolean;
  withCornerMarkers: boolean;
  animations: {
    focus: boolean;
    error: boolean;
  };
}

export interface PalantirCardConfig {
  defaultVariant: PalantirVariant;
  defaultPadding: PalantirSize;
  withGlow: boolean;
  withCornerMarkers: boolean;
  withGeometricBorder: boolean;
  animations: {
    hover: boolean;
    appear: boolean;
  };
}

export interface PalantirSidebarConfig {
  defaultWidth: PalantirSize;
  collapsible: boolean;
  withGlow: boolean;
  withCornerMarkers: boolean;
  animations: {
    collapse: boolean;
    hover: boolean;
  };
}

export interface PalantirHeaderConfig {
  defaultHeight: PalantirSize;
  withWindowControls: boolean;
  withGlow: boolean;
  withCornerMarkers: boolean;
  animations: {
    appear: boolean;
  };
}

export interface PalantirPanelConfig {
  defaultVariant: PalantirVariant;
  defaultPadding: PalantirSize;
  withGlow: boolean;
  withCornerMarkers: boolean;
  withGeometricBorder: boolean;
  animations: {
    appear: boolean;
  };
}

export interface PalantirModalConfig {
  defaultSize: PalantirSize;
  withGlow: boolean;
  withCornerMarkers: boolean;
  closeOnBackdropClick: boolean;
  preventBodyScroll: boolean;
  animations: {
    appear: boolean;
    disappear: boolean;
  };
}

// Context Types
export interface PalantirUIContextValue {
  config: PalantirUIConfig;
  updateConfig: (config: Partial<PalantirUIConfig>) => void;
  toggleDarkMode: () => void;
  setAccentColor: (color: PalantirAccentColor) => void;
  toggleAnimations: () => void;
}

// Event Types
export interface PalantirUIEvent {
  type: string;
  payload?: any;
  timestamp: number;
}

export interface PalantirUIComponentEvent extends PalantirUIEvent {
  component: string;
  action: 'hover' | 'focus' | 'click' | 'change';
}

// Animation Event Types
export interface PalantirUIAnimationEvent extends PalantirUIEvent {
  animation: PalantirAnimation;
  element: string;
  state: 'start' | 'end' | 'cancel';
}

// Export default configuration
export const defaultPalantirUIConfig: PalantirUIConfig = {
  theme: {
    colors: {
      zinc: {
        'zinc-50': '#fafafa',
        'zinc-100': '#f4f4f5',
        'zinc-200': '#e4e4e7',
        'zinc-300': '#d4d4d8',
        'zinc-400': '#a1a1aa',
        'zinc-500': '#71717a',
        'zinc-600': '#52525b',
        'zinc-700': '#3f3f46',
        'zinc-800': '#27272a',
        'zinc-900': '#18181b',
        'zinc-950': '#09090b',
      },
      accent: {
        'blue': '#3b82f6',
        'cyan': '#06b6d4',
        'green': '#10b981',
        'orange': '#f97316',
        'red': '#ef4444',
      },
      layer: {
        'layer-1': 'rgba(250, 250, 250, 0.05)',
        'layer-2': 'rgba(250, 250, 250, 0.1)',
        'layer-3': 'rgba(250, 250, 250, 0.15)',
        'layer-4': 'rgba(250, 250, 250, 0.2)',
        'layer-5': 'rgba(250, 250, 250, 0.25)',
      },
    },
    animations: {
      'pal-fade-in-up': 'palFadeInUp 0.3s ease-out',
      'pal-fade-in': 'palFadeIn 0.2s ease-out',
      'pal-card-fade-in': 'palCardFadeIn 0.4s ease-out',
      'pal-glow-pulse': 'palGlowPulse 2s ease-in-out infinite',
      'pal-corner-appear': 'palCornerAppear 0.5s ease-out',
      'pal-border-glow': 'palBorderGlow 3s ease-in-out infinite',
      'pal-slide-up': 'palSlideUp 0.3s ease-out',
      'pal-scale-in': 'palScaleIn 0.2s ease-out',
    },
    breakpoints: {
      sm: '640px',
      md: '768px',
      lg: '1024px',
      xl: '1280px',
    },
  },
  state: {
    isDarkMode: false,
    accentColor: 'blue',
    animationsEnabled: true,
    reducedMotion: false,
  },
  components: {
    button: {
      defaultVariant: 'primary',
      defaultSize: 'md',
      withGlow: false,
      withCornerMarkers: false,
      animations: {
        hover: true,
        active: true,
        focus: true,
      },
    },
    input: {
      defaultVariant: 'default',
      defaultSize: 'md',
      withGlow: false,
      withCornerMarkers: false,
      animations: {
        focus: true,
        error: true,
      },
    },
    card: {
      defaultVariant: 'default',
      defaultPadding: 'md',
      withGlow: false,
      withCornerMarkers: false,
      withGeometricBorder: false,
      animations: {
        hover: true,
        appear: true,
      },
    },
    sidebar: {
      defaultWidth: 'md',
      collapsible: true,
      withGlow: false,
      withCornerMarkers: false,
      animations: {
        collapse: true,
        hover: true,
      },
    },
    header: {
      defaultHeight: 'md',
      withWindowControls: false,
      withGlow: false,
      withCornerMarkers: false,
      animations: {
        appear: true,
      },
    },
    panel: {
      defaultVariant: 'default',
      defaultPadding: 'md',
      withGlow: false,
      withCornerMarkers: false,
      withGeometricBorder: false,
      animations: {
        appear: true,
      },
    },
    modal: {
      defaultSize: 'md',
      withGlow: false,
      withCornerMarkers: false,
      closeOnBackdropClick: true,
      preventBodyScroll: true,
      animations: {
        appear: true,
        disappear: true,
      },
    },
  },
};