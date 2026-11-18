import { CaretDown, Desktop, Moon, Sun } from "@phosphor-icons/react";
import { Button } from "#components/core/button";
import { ChoiceSelect } from "./ChoiceSelect";
import type { Appearance, Settings } from "../lib/settings";
import { ShortcutHint } from "./ShortcutHint";
import type { DemoOutcome, EngineSnapshot } from "../engine/types";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModelSettings } from "./ModelSettings";
import type { ModelService } from "../engine/models";
import { FeedbackSettings } from "./FeedbackSettings";
import { OverlaySettings } from "./OverlaySettings";
import { Permissions } from "./Permissions";
import { ShortcutRecorder } from "./ShortcutRecorder";
import { MicrophonePicker } from "./MicrophonePicker";
import { SettingsIndex } from "./SettingsIndex";

export function SettingsPage({
  settings,
  section,
  onChange,
  outcome,
  onOutcomeChange,
  demo,
  modelPath,
  modelLoaded,
  shortcutCommand,
  shortcut,
  busy,
  models,
}: {
  settings: Settings;
  section: "model" | "permissions" | null;
  onChange(settings: Settings): void;
  outcome: DemoOutcome;
  onOutcomeChange(outcome: DemoOutcome): void;
  demo: boolean;
  modelPath?: string;
  modelLoaded?: boolean;
  shortcutCommand?: string;
  shortcut?: EngineSnapshot["shortcut"];
  busy: boolean;
  models?: ModelService;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const sections = useRef<HTMLDivElement>(null);
  const [shortcutError, setShortcutError] = useState("");
  async function changeShortcut(command: string, args?: Record<string, unknown>) {
    setShortcutError("");
    try {
      await invoke(command, args);
    } catch (error) {
      setShortcutError(String(error));
    }
  }
  useEffect(() => {
    if (section === "model") {
      const title = document.getElementById("model-title");
      title?.focus({ preventScroll: true });
      title?.scrollIntoView({ block: "start" });
    } else if (!section) heading.current?.focus();
  }, [section]);
  return (
    <main
      className="mx-auto flex w-full max-w-2xl flex-1 flex-col gap-6 px-4 py-8 sm:px-8 md:max-w-5xl"
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
      <div className="grid items-start gap-8 md:grid-cols-[minmax(0,1fr)_10rem]">
        <div ref={sections} className="flex min-w-0 flex-col divide-y">
          <section className="settings-section" aria-labelledby="input-title">
            <h2 id="input-title" className="section-label">
              Dictation
            </h2>
            <MicrophonePicker busy={busy} demo={demo} />
            {!demo && (
              <div className="flex flex-col gap-2">
                <p>Shortcut</p>
                {shortcut?.label && <ShortcutHint label={shortcut.label} />}
                {shortcut?.paused && (
                  <p className="supporting">Shortcut paused. Resume it from the tray menu.</p>
                )}
                {shortcut && (shortcut.backend === "native" || shortcut.backend === "portal") && (
                  <>
                    {shortcut.backend === "native" ? (
                      <ShortcutRecorder busy={busy} enabled={!!shortcut.label} />
                    ) : (
                      <Button
                        variant="outline"
                        disabled={shortcut.configuring}
                        onClick={() => void changeShortcut("shortcut_configure")}
                      >
                        {shortcut.configuring
                          ? "Waiting for shortcut…"
                          : shortcut.label
                            ? "Change shortcut…"
                            : "Set up shortcut…"}
                      </Button>
                    )}
                    {shortcut.label && (
                      <label className="flex flex-col gap-2">
                        Recording mode
                        <ChoiceSelect
                          aria-label="Recording mode"
                          disabled={busy}
                          value={shortcut.mode}
                          onChange={(mode) => void changeShortcut("shortcut_mode", { mode })}
                          options={[
                            { value: "toggle", label: "Toggle dictation" },
                            { value: "hold", label: "Hold to dictate" },
                          ]}
                        />
                      </label>
                    )}
                  </>
                )}
                {shortcutCommand &&
                  (shortcut?.backend === "manual" ||
                    (shortcut?.backend === "portal" && !shortcut.label)) && (
                    <>
                      <p className="supporting">
                        {shortcut.backend === "manual"
                          ? "Chime couldn’t reach a shortcuts portal. Bind these commands in your desktop’s keyboard settings."
                          : "Your desktop hasn’t assigned a shortcut. Set one up above, or bind these commands in its keyboard settings."}
                      </p>
                      {(
                        [
                          [
                            "Hold to dictate",
                            "If your desktop supports key release, bind start to the press and stop to the release.",
                            ["--start", "--stop"],
                          ],
                          [
                            "Toggle dictation",
                            "Press once to dictate, then again to transcribe and paste.",
                            ["--toggle"],
                          ],
                          [
                            "Paste last dictation",
                            "Paste your most recent transcript into the focused app.",
                            ["--paste-last"],
                          ],
                        ] as const
                      ).map(([name, help, flags]) => (
                        <div key={name} className="flex flex-col gap-1">
                          <p>{name}</p>
                          <p className="supporting">{help}</p>
                          {flags.map((flag) => (
                            <code
                              key={flag}
                              className="select-text break-all rounded-sm border p-2 text-xs"
                            >
                              {shortcutCommand} {flag}
                            </code>
                          ))}
                        </div>
                      ))}
                    </>
                  )}
                {(shortcutError || shortcut?.error) && (
                  <p role="alert" className="supporting text-destructive">
                    {shortcutError || shortcut?.error}
                  </p>
                )}
              </div>
            )}
          </section>
          {!demo && shortcut?.backend === "native" && (
            <Permissions focus={section === "permissions"} />
          )}
          {!demo && <FeedbackSettings />}
          {!demo && <OverlaySettings />}
          <section className="settings-section" aria-labelledby="model-title">
            <h2 id="model-title" tabIndex={-1} className="section-label">
              Speech model
            </h2>
            {models ? (
              <ModelSettings source={models} busy={busy} />
            ) : (
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
          {demo && (
            <details className="group py-5">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-sm py-1 text-muted-foreground">
                <span>Prototype controls</span>
                <CaretDown className="size-4 group-open:rotate-180" aria-hidden="true" />
              </summary>
              <div className="flex flex-col gap-2 pt-4">
                <label htmlFor="outcome">Next demo result</label>
                <ChoiceSelect
                  id="outcome"
                  value={outcome}
                  onChange={(value) => onOutcomeChange(value as DemoOutcome)}
                  options={[
                    { value: "success", label: "Sample transcript" },
                    { value: "silence", label: "No speech detected" },
                    { value: "error", label: "Engine interrupted" },
                  ]}
                />
              </div>
            </details>
          )}
        </div>
        <SettingsIndex sections={sections} />
      </div>
    </main>
  );
}
