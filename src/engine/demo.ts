import type { DemoEngine, DemoOutcome, EngineSnapshot } from "./types";

export const SAMPLE_TRANSCRIPT =
  "Let's keep this simple. A shortcut, a few words, and back to what you were doing.";

/** No microphone, model, storage, or network access. All output is a fixed sample. */
export function createDemoEngine(processingMs = 1100): DemoEngine {
  let snapshot: EngineSnapshot = {
    phase: "idle",
    startedAt: null,
    lastResult: null,
    message: null,
  };
  let outcome: DemoOutcome = "success";
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;
  const listeners = new Set<() => void>();

  function update(next: EngineSnapshot) {
    snapshot = next;
    for (const listener of listeners) listener();
  }

  function cancelPending() {
    clearTimeout(timer);
    timer = undefined;
  }

  return {
    kind: "demo",
    getSnapshot: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    setOutcome(next) {
      outcome = next;
    },
    start() {
      if (disposed || snapshot.phase === "recording" || snapshot.phase === "processing") return;
      update({ ...snapshot, phase: "recording", startedAt: Date.now(), message: null });
    },
    finish() {
      if (disposed || snapshot.phase !== "recording") return;
      const durationMs = Date.now() - snapshot.startedAt!;
      const selectedOutcome = outcome;
      update({ ...snapshot, phase: "processing", startedAt: null });
      timer = setTimeout(() => {
        timer = undefined;
        if (selectedOutcome === "error") {
          update({
            ...snapshot,
            phase: "error",
            message: snapshot.lastResult
              ? "The speech engine stopped. Your last result is still here. Try again."
              : "The speech engine stopped. Nothing was added. Try again.",
          });
        } else if (selectedOutcome === "silence") {
          update({
            ...snapshot,
            phase: "idle",
            message: "No speech detected. Your last result hasn't changed.",
          });
        } else {
          update({
            phase: "complete",
            startedAt: null,
            lastResult: { text: SAMPLE_TRANSCRIPT, durationMs },
            message: null,
          });
        }
      }, processingMs);
    },
    cancel() {
      if (disposed || (snapshot.phase !== "recording" && snapshot.phase !== "processing")) return;
      cancelPending();
      update({
        ...snapshot,
        phase: "idle",
        startedAt: null,
        message: "Cancelled. Nothing was added.",
      });
    },
    clearResult() {
      if (disposed) return;
      update({ ...snapshot, lastResult: null });
    },
    dispose() {
      disposed = true;
      cancelPending();
      listeners.clear();
      snapshot = { phase: "idle", startedAt: null, lastResult: null, message: null };
    },
  };
}
