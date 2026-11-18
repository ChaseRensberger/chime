import { invoke } from "@tauri-apps/api/core";
import type { DictationEngine, Transcript } from "./types";

export type Retention = "off" | "day" | "week" | "month" | "forever";
export interface HistoryEntry {
  id: number;
  timestampMs: number;
  text: string;
  durationMs: number;
}
export interface HistoryListing {
  entries: HistoryEntry[];
  total: number;
  matches: number;
  retention: Retention;
  error: string | null;
}
export interface HistorySource {
  list(query: string): Promise<HistoryListing>;
  remove(id: number): Promise<void>;
  clear(): Promise<void>;
  setRetention(retention: Retention, confirmed?: number): Promise<number>;
  copy(id: number): Promise<void>;
  dispose(): void;
}

export function createHistory(engine: DictationEngine): HistorySource {
  if (engine.kind === "native")
    return {
      list: (query) => invoke("history_list", { query }),
      remove: (id) => invoke("history_update", { action: "delete", id }),
      clear: () => invoke("history_update", { action: "clear" }),
      setRetention: (retention, confirmed = 0) =>
        invoke("history_update", { action: "retention", retention, confirmed }),
      copy: (id) => invoke("history_copy", { id }),
      dispose() {},
    };
  let entries: HistoryEntry[] = [];
  let retention: Retention = "week";
  let previous: Transcript | null = null;
  let sequence = 0;
  function prune() {
    const days = { off: 0, day: 1, week: 7, month: 30, forever: 0 }[retention];
    if (days)
      entries = entries.filter((entry) => entry.timestampMs >= Date.now() - days * 86400000);
  }
  const unsubscribe = engine.subscribe(() => {
    const { lastResult, phase } = engine.getSnapshot();
    if (phase === "complete" && lastResult && lastResult !== previous) {
      previous = lastResult;
      if (retention !== "off")
        entries.unshift({ id: ++sequence, timestampMs: Date.now(), ...lastResult });
      prune();
      entries = entries.slice(0, 10000);
    }
  });
  return {
    async list(query) {
      prune();
      const matches = entries.filter((entry) =>
        entry.text.toLowerCase().includes(query.trim().toLowerCase()),
      );
      return {
        entries: matches.slice(0, 200),
        matches: matches.length,
        total: entries.length,
        retention,
        error: null,
      };
    },
    async remove(id) {
      entries = entries.filter((entry) => entry.id !== id);
    },
    async clear() {
      entries = [];
    },
    async setRetention(value, confirmed = 0) {
      const days = { off: 0, day: 1, week: 7, month: 30, forever: 0 }[value];
      const cutoff = Date.now() - days * 86400000;
      const count = days ? entries.filter((entry) => entry.timestampMs < cutoff).length : 0;
      if (count > confirmed) return count;
      retention = value;
      if (days) entries = entries.filter((entry) => entry.timestampMs >= cutoff);
      return 0;
    },
    async copy(id) {
      const entry = entries.find((entry) => entry.id === id);
      if (!entry) throw new Error("Entry no longer available");
      await navigator.clipboard.writeText(entry.text);
    },
    dispose: unsubscribe,
  };
}
