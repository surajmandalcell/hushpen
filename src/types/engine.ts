import type { ErrorCode } from "./contracts";

export interface EngineRuntimeState {
  status: string;
  mode?: string;
  modelId?: string;
  model?: string;
  detail?: string;
  progress?: number | null;
  error?: string | null;
  errorCode?: ErrorCode | null;
  updatedAt?: string;
}

export interface EngineModelInfo {
  id: string;
  displayName: string;
  detail: string;
  sizeLabel: string;
  installed?: boolean;
  diskBytes?: number;
  path?: string;
  downloadUrl?: string;
}
