export type OverlayPlacement = "top" | "bottom";

export interface OverlaySettings {
  placement: OverlayPlacement;
  customBounds: {
    displayId: number;
    x: number;
    y: number;
  } | null;
}

export interface StartupSettings {
  supported: boolean;
  enabled: boolean;
  executablePath?: string;
  registeredExecutablePath?: string;
  autostartPath?: string;
  status?: string;
  requiresApproval?: boolean;
  detail?: string;
}

export interface TextEditorOption {
  id: string;
  label: string;
  detail: string;
  iconDataUrl?: string;
}
