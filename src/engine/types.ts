/** UI-facing session contract. Native audio and desktop output belong in adapters. */
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
  modelLoaded?: boolean;
  shortcutCommand?: string;
}

export interface DictationEngine {
  readonly kind: "demo" | "native";
  getSnapshot(): EngineSnapshot;
  subscribe(listener: () => void): () => void;
  start(): void;
  finish(): void;
  cancel(): void;
  clearResult(): void;
  chooseModel?(): Promise<void>;
  copyResult?(): Promise<void>;
  dispose(): void;
}

export interface DemoEngine extends DictationEngine {
  setOutcome(outcome: DemoOutcome): void;
}
