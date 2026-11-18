import { invoke } from "@tauri-apps/api/core";

export interface ModelEntry {
  id: string;
  name: string;
  size: number;
  installed: boolean;
  active: boolean;
  recommended: boolean;
  languages: string | null;
  partialSize: number;
}

export interface ModelSnapshot {
  entries: ModelEntry[];
  progress: {
    modelId: string | null;
    busy: boolean;
    stage: string;
    downloaded: number;
    total: number;
    error: string | null;
  };
}

export interface ModelService {
  snapshot(): Promise<ModelSnapshot>;
  start(action: "download" | "import" | "use", modelId?: string): Promise<void>;
  cancel(): Promise<void>;
  openInfo(page: "source" | "license"): Promise<void>;
}

export const nativeModels: ModelService = {
  snapshot: () => invoke("models_snapshot"),
  start: (action, modelId) => invoke("models_start", { action, modelId: modelId ?? null }),
  cancel: () => invoke("models_cancel"),
  openInfo: (page) => invoke("models_open_info", { page }),
};
