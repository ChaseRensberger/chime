import { invoke } from "@tauri-apps/api/core";
import type { DictationEngine, EngineSnapshot } from "./types";
import { nativeModels } from "./models";

export function createNativeEngine(): DictationEngine {
  let snapshot: EngineSnapshot = {
    phase: "idle",
    startedAt: null,
    lastResult: null,
    message: null,
  };
  const listeners = new Set<() => void>();
  let disposed = false;
  let pending = false;
  let timer: ReturnType<typeof setTimeout>;
  function update(next: EngineSnapshot) {
    if (disposed) return;
    if (
      snapshot.lastResult?.text === next.lastResult?.text &&
      snapshot.lastResult?.durationMs === next.lastResult?.durationMs
    )
      next.lastResult = snapshot.lastResult;
    snapshot = next;
    listeners.forEach((listener) => listener());
  }
  async function refresh() {
    try {
      update(await invoke<EngineSnapshot>("session_snapshot"));
    } catch (error) {
      update({ ...snapshot, phase: "error", message: String(error) });
    }
    if (!disposed) timer = setTimeout(refresh, 80);
  }
  async function command(action: string) {
    if (pending || disposed) return;
    pending = true;
    try {
      await invoke("session_action", { action });
    } catch (error) {
      update({ ...snapshot, phase: "error", message: String(error) });
    } finally {
      pending = false;
    }
  }
  void refresh();
  return {
    kind: "native",
    getSnapshot: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    start: () => void command("start"),
    finish: () => void command("finish"),
    cancel: () => void command("cancel"),
    clearResult: () => void command("clear"),
    models: nativeModels,
    copyResult: () => invoke("copy_result"),
    dispose() {
      disposed = true;
      clearTimeout(timer);
      listeners.clear();
      void invoke("session_action", { action: "cancel" });
    },
  };
}
