/** UI-facing session contract. Native audio and desktop output belong in adapters. */
import type { ModelService } from "./models";
export type Phase = "idle" | "starting" | "recording" | "processing" | "complete" | "error";
export type DemoOutcome = "success" | "silence" | "error";

export interface Transcript {
  text: string;
  durationMs: number;
}

export interface EngineSnapshot {
  phase: Phase;
  startedAt: number | null;
  lastResult: Transcript | null;
  message: string | null;
  level?: number;
  modelPath?: string;
  modelName?: string;
  pasteFailed?: boolean;
  modelLoaded?: boolean;
  modelAvailable?: boolean;
  shortcutCommand?: string;
  shortcut?: {
    backend: "manual" | "portal" | "native";
    label: string | null;
    error: string | null;
    paused: boolean;
    mode: "toggle" | "hold";
    configuring: boolean;
  };
}

export interface DictationEngine {
  readonly kind: "demo" | "native";
  getSnapshot(): EngineSnapshot;
  subscribe(listener: () => void): () => void;
  start(): void;
  finish(): void;
  cancel(): void;
  clearResult(): void;
  models?: ModelService;
  copyResult?(): Promise<void>;
  dispose(): void;
}

export interface DemoEngine extends DictationEngine {
  setOutcome(outcome: DemoOutcome): void;
}
