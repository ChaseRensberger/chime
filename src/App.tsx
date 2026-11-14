import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  Check,
  CircleNotch,
  ClockCounterClockwise,
  Copy,
  Microphone,
  SlidersHorizontal,
  Square,
  Waveform,
  X,
} from "@phosphor-icons/react";
import { Button } from "#components/core/button";
import { SettingsPage } from "./components/SettingsPage";
import { HistoryPage } from "./components/HistoryPage";
import { AudioVisualization } from "./components/AudioVisualization";
import { ShortcutHint } from "./components/ShortcutHint";
import type { HistorySource } from "./engine/history";
import type { DemoEngine, DictationEngine, DemoOutcome } from "./engine/types";
import { loadSettings, saveSettings } from "./lib/settings";
import type { Settings } from "./lib/settings";

type Page = "dictation" | "history" | "settings";

function Elapsed({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, []);
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  return (
    <span className="tabular-nums" aria-hidden="true">{`${Math.floor(seconds / 60)
      .toString()
      .padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`}</span>
  );
}

export function App({ engine, history }: { engine: DictationEngine; history: HistorySource }) {
  const state = useSyncExternalStore(engine.subscribe, engine.getSnapshot);
  const [settings, setSettings] = useState(loadSettings);
  const [page, setPage] = useState<Page>("dictation");
  const nav = useRef<HTMLElement>(null);
  const [outcome, setOutcome] = useState<DemoOutcome>("success");
  const [notice, setNotice] = useState("");
  const [copied, setCopied] = useState(false);
  const busy = ["starting", "recording", "processing"].includes(state.phase);
  const demo = engine.kind === "demo";

  useEffect(() => {
    if (settings.appearance === "system") delete document.documentElement.dataset.mode;
    else document.documentElement.dataset.mode = settings.appearance;
  }, [settings.appearance]);
  useEffect(() => {
    document.getElementById(`${page}-title`)?.focus();
  }, [page]);
  useEffect(() => {
    setCopied(false);
  }, [state.lastResult]);
  useEffect(() => {
    setNotice("");
  }, [state.phase, state.lastResult]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.isComposing || event.repeat) return;
      if (page !== "dictation") {
        if (event.key === "Escape") {
          setPage("dictation");
          requestAnimationFrame(() =>
            nav.current?.querySelector<HTMLButtonElement>(`[data-page="${page}"]`)?.focus(),
          );
        }
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable || target.closest("input, textarea, select"))
      )
        return;
      if (event.key === "Escape" && busy) {
        event.preventDefault();
        engine.cancel();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [engine, page, busy]);

  function changeSettings(next: Settings) {
    setSettings(next);
    if (!saveSettings(next))
      setNotice("Preferences apply to this session. Storage is unavailable.");
  }

  async function copyResult() {
    if (!state.lastResult) return;
    try {
      if (engine.copyResult) await engine.copyResult();
      else await navigator.clipboard.writeText(state.lastResult.text);
      setCopied(true);
      setNotice("Copied to clipboard.");
    } catch {
      setNotice("Clipboard unavailable. Select the transcript to copy it manually.");
    }
  }

  return (
    <div className="app-shell">
      <header className="flex items-center justify-between gap-3 border-b px-4 py-4 sm:px-8">
        <span className="text-lg font-semibold tracking-tight">chime</span>
        <nav ref={nav} aria-label="Main navigation" className="flex items-center gap-1">
          {(
            [
              ["dictation", "Dictation", Waveform],
              ["history", "History", ClockCounterClockwise],
              ["settings", "Settings", SlidersHorizontal],
            ] as const
          ).map(([value, label, Icon]) => (
            <Button
              key={value}
              data-page={value}
              variant={page === value ? "secondary" : "ghost"}
              size="sm"
              aria-label={label}
              aria-current={page === value ? "page" : undefined}
              onClick={() => setPage(value)}
            >
              <Icon aria-hidden="true" />
              <span className="hidden sm:inline">{label}</span>
            </Button>
          ))}
        </nav>
      </header>

      {page === "settings" ? (
        <SettingsPage
          settings={settings}
          onChange={changeSettings}
          outcome={outcome}
          demo={demo}
          modelPath={state.modelPath}
          modelLoaded={state.modelLoaded}
          shortcutCommand={state.shortcutCommand}
          busy={busy}
          models={engine.models}
          onOutcomeChange={(next) => {
            setOutcome(next);
            if (demo) (engine as DemoEngine).setOutcome(next);
          }}
        />
      ) : page === "history" ? (
        <HistoryPage source={history} result={state.lastResult} demo={demo} />
      ) : (
        <main
          className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-8 px-4 py-8 sm:px-8"
          aria-labelledby="dictation-title"
        >
          <h1 id="dictation-title" tabIndex={-1} className="sr-only">
            Dictation
          </h1>
          <section
            className="flex min-h-80 flex-1 flex-col items-center justify-center gap-6"
            aria-label="Dictation controls"
          >
            <AudioVisualization phase={state.phase} level={state.level ?? 0} demo={demo} />
            <div className="flex items-center gap-3 text-xs text-muted-foreground">
              <span role="status">
                {state.phase === "starting"
                  ? "Opening microphone"
                  : state.phase === "recording"
                    ? demo
                      ? "Demo"
                      : "Listening"
                    : state.phase === "processing"
                      ? "Transcribing"
                      : state.phase === "error"
                        ? "Try again"
                        : "Ready"}
              </span>
              {state.phase === "recording" && state.startedAt !== null && (
                <Elapsed startedAt={state.startedAt} />
              )}
            </div>
            <div className="flex flex-col items-center gap-4">
              <div className="flex items-center gap-2">
                <Button
                  size="lg"
                  disabled={
                    state.phase === "starting" ||
                    state.phase === "processing" ||
                    (!demo && state.modelAvailable === false)
                  }
                  aria-busy={state.phase === "processing"}
                  onClick={() => (state.phase === "recording" ? engine.finish() : engine.start())}
                >
                  {state.phase === "recording" ? (
                    <Square aria-hidden="true" />
                  ) : state.phase === "processing" ? (
                    <CircleNotch className="animate-spin" aria-hidden="true" />
                  ) : (
                    <Microphone aria-hidden="true" />
                  )}
                  {state.phase === "recording"
                    ? demo
                      ? "Finish demo"
                      : "Finish"
                    : state.phase === "processing"
                      ? "Transcribing…"
                      : state.phase === "starting"
                        ? "Opening…"
                        : demo
                          ? "Start demo"
                          : "Start dictation"}
                </Button>
                {busy && (
                  <Button variant="ghost" onClick={() => engine.cancel()}>
                    Cancel
                  </Button>
                )}
              </div>
              {!demo && (
                <div className="text-xs text-muted-foreground">
                  <ShortcutHint />
                </div>
              )}
              {!demo && state.modelAvailable === false && (
                <Button variant="outline" onClick={() => setPage("settings")}>
                  Set up speech model
                </Button>
              )}
            </div>
          </section>

          {state.lastResult && (
            <section aria-label="Last transcript" className="flex flex-col gap-3 border-t pt-4">
              <div className="flex items-center justify-end gap-3">
                <div className="flex items-center gap-1">
                  <Button variant="ghost" size="sm" onClick={() => void copyResult()}>
                    {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
                    {copied ? "Copied" : "Copy"}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Dismiss last transcript"
                    onClick={() => engine.clearResult()}
                  >
                    <X aria-hidden="true" />
                  </Button>
                </div>
              </div>
              <p className="line-clamp-3 select-text whitespace-pre-wrap break-words text-sm leading-6">
                {state.lastResult.text}
              </p>
            </section>
          )}
          <p
            role="status"
            className={`min-h-5 text-center text-xs leading-5 ${state.phase === "error" ? "text-destructive" : "text-muted-foreground"}`}
          >
            {notice || state.message || (demo ? "Demo preview · Microphone off" : "")}
          </p>
        </main>
      )}
    </div>
  );
}
