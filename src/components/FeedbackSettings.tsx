import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "#components/core/button";
import { ChoiceSelect } from "./ChoiceSelect";
import { Checkbox } from "#components/core/checkbox";

interface Preferences {
  enabled: boolean;
  volume: number;
}

export function FeedbackSettings() {
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void invoke<Preferences>("feedback_preferences")
      .then((value) => {
        if (active) setPreferences(value);
      })
      .catch((error) => {
        if (active) setError(String(error));
      });
    return () => {
      active = false;
    };
  }, []);

  async function save(next: Preferences) {
    setPending(true);
    setError("");
    try {
      await invoke("feedback_save", { preferences: next });
      setPreferences(next);
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  }

  async function preview() {
    setPending(true);
    setError("");
    try {
      await invoke("feedback_preview");
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="settings-section" aria-labelledby="feedback-title">
      <h2 id="feedback-title" className="section-label">
        Recording sounds
      </h2>
      {preferences ? (
        <>
          <label className="flex min-h-11 items-center gap-3">
            <Checkbox
              checked={preferences.enabled}
              disabled={pending}
              onCheckedChange={(enabled) => void save({ ...preferences, enabled })}
            />
            Play recording sounds
          </label>
          <label className="flex flex-col gap-2">
            Volume
            <ChoiceSelect
              aria-label="Volume"
              disabled={pending || !preferences.enabled}
              value={String(preferences.volume)}
              onChange={(value) => void save({ ...preferences, volume: Number(value) })}
              options={[
                { value: "0.15", label: "Quiet" },
                { value: "0.35", label: "Balanced" },
                { value: "0.65", label: "Loud" },
                ...([0.15, 0.35, 0.65].includes(preferences.volume)
                  ? []
                  : [
                      {
                        value: String(preferences.volume),
                        label: `${Math.round(preferences.volume * 100)}%`,
                      },
                    ]),
              ]}
            />
          </label>
          <Button
            variant="outline"
            disabled={pending || !preferences.enabled || preferences.volume === 0}
            onClick={() => void preview()}
          >
            Preview sound
          </Button>
        </>
      ) : (
        !error && (
          <p role="status" className="supporting">
            Loading sound preferences…
          </p>
        )
      )}
      {error && (
        <p role="alert" className="supporting text-destructive">
          {error}
        </p>
      )}
    </section>
  );
}
