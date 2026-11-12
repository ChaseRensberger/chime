import { CaretDown, Desktop, Moon, Sun } from "@phosphor-icons/react";
import { Button } from "#components/core/button";
import { NativeSelect } from "#components/core/native-select";
import type { Appearance, Settings, Shortcut } from "../lib/settings";
import type { DemoOutcome } from "../engine/types";
import { useEffect, useRef, useState } from "react";

export function SettingsPage({
  settings,
  onChange,
  outcome,
  onOutcomeChange,
  demo,
  modelPath,
  modelLoaded,
  shortcutCommand,
  busy,
  onChooseModel,
}: {
  settings: Settings;
  onChange(settings: Settings): void;
  outcome: DemoOutcome;
  onOutcomeChange(outcome: DemoOutcome): void;
  demo: boolean;
  modelPath?: string;
  modelLoaded?: boolean;
  shortcutCommand?: string;
  busy: boolean;
  onChooseModel?: () => Promise<void>;
}) {
  const [choosing, setChoosing] = useState(false);
  const [modelError, setModelError] = useState("");
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    heading.current?.focus();
  }, []);
  async function chooseModel() {
    setChoosing(true);
    setModelError("");
    try {
      await onChooseModel?.();
    } catch (error) {
      setModelError(String(error));
    } finally {
      setChoosing(false);
    }
  }
  return (
    <main
      className="mx-auto flex w-full max-w-2xl flex-1 flex-col gap-6 px-4 py-8 sm:px-8"
      aria-labelledby="settings-title"
    >
      <h1
        id="settings-title"
        ref={heading}
        tabIndex={-1}
        className="text-2xl font-semibold tracking-tight outline-none"
      >
        Settings
      </h1>
      <div className="flex flex-col divide-y">
        <section className="settings-section" aria-labelledby="input-title">
          <h2 id="input-title" className="section-label">
            Dictation
          </h2>
          <div className="flex flex-col gap-2">
            <label htmlFor="microphone">Microphone</label>
            <NativeSelect id="microphone" disabled aria-describedby="microphone-help">
              <option>System default</option>
            </NativeSelect>
            <p id="microphone-help" className="supporting">
              {demo
                ? "Browser preview. Your microphone is off."
                : "Uses your system's default input. Audio stays in memory."}
            </p>
          </div>
          <div className="flex flex-col gap-2">
            <label htmlFor="shortcut">In-window shortcut</label>
            <NativeSelect
              id="shortcut"
              value={settings.shortcut}
              onChange={(event) =>
                onChange({ ...settings, shortcut: event.target.value as Shortcut })
              }
              aria-describedby="shortcut-help"
            >
              <option value="Control+Shift+Space">Ctrl + Shift + Space</option>
              <option value="Control+Shift+Period">Ctrl + Shift + .</option>
            </NativeSelect>
            <p id="shortcut-help" className="supporting">
              Works while Chime is focused on the Dictation page.
            </p>
          </div>
          {!demo && (
            <div className="flex flex-col gap-2">
              <p>Desktop shortcut</p>
              <p className="supporting">
                In COSMIC Settings → Keyboard → Custom Shortcuts, assign a key to this command:
              </p>
              <code className="select-text break-all rounded-sm border p-2 text-xs">
                {shortcutCommand}
              </code>
              <p className="supporting">
                Toggles recording without focusing Chime, then pastes into the focused app. Use
                --toggle-terminal for Ctrl+Shift+V. While processing, the shortcut cancels.
              </p>
            </div>
          )}
        </section>
        <section className="settings-section" aria-labelledby="model-title">
          <h2 id="model-title" className="section-label">
            Speech model
          </h2>
          <div className="flex flex-col gap-1">
            <p className="break-all">
              {demo
                ? "Parakeet Unified English"
                : modelPath?.split(/[\\/]/).pop() || "No model selected"}
            </p>
            <p className="supporting">
              {demo
                ? "0.6B · Q8_0 · 731 MB"
                : modelLoaded
                  ? "Loaded · CPU · kept warm"
                  : "Loads on first dictation · CPU"}
            </p>
          </div>
          <p className="supporting">
            {demo
              ? "Not loaded in the browser preview."
              : "Choose a downloaded GGUF speech model. Chime remembers the file location."}
          </p>
          {!demo && (
            <>
              <Button
                variant="outline"
                disabled={busy || choosing}
                onClick={() => void chooseModel()}
              >
                {choosing ? "Choosing…" : "Choose model…"}
              </Button>
              {modelError && (
                <p role="alert" className="supporting text-destructive">
                  {modelError}
                </p>
              )}
            </>
          )}
        </section>
        <section className="settings-section" aria-labelledby="appearance-title">
          <h2 id="appearance-title" className="section-label">
            Appearance
          </h2>
          <div className="grid grid-cols-3 gap-2" role="group" aria-label="Appearance">
            {(
              [
                ["system", Desktop],
                ["light", Sun],
                ["dark", Moon],
              ] as const
            ).map(([mode, Icon]) => (
              <Button
                key={mode}
                variant={settings.appearance === mode ? "secondary" : "outline"}
                aria-pressed={settings.appearance === mode}
                onClick={() => onChange({ ...settings, appearance: mode as Appearance })}
                className="capitalize"
              >
                <Icon aria-hidden="true" />
                {mode}
              </Button>
            ))}
          </div>
        </section>
        <section className="settings-section" aria-labelledby="retention-title">
          <h2 id="retention-title" className="section-label">
            Transcript history
          </h2>
          <p className="supporting">
            {demo
              ? "Demo history lasts for this browser session."
              : "Successful transcripts are saved locally for 7 days by default. Change retention or delete entries on the History page. Audio is never saved."}
          </p>
        </section>
        {demo && (
          <details className="group py-5">
            <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-sm py-1 text-muted-foreground">
              <span>Prototype controls</span>
              <CaretDown className="size-4 group-open:rotate-180" aria-hidden="true" />
            </summary>
            <div className="flex flex-col gap-2 pt-4">
              <label htmlFor="outcome">Next demo result</label>
              <NativeSelect
                id="outcome"
                value={outcome}
                onChange={(event) => onOutcomeChange(event.target.value as DemoOutcome)}
              >
                <option value="success">Sample transcript</option>
                <option value="silence">No speech detected</option>
                <option value="error">Engine interrupted</option>
              </NativeSelect>
              <p className="supporting">
                Choose a scenario, close Settings, then start and finish a demo. This choice resets
                when you reload.
              </p>
            </div>
          </details>
        )}
      </div>
    </main>
  );
}
