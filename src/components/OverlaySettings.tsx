import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "#components/core/button";
import { NativeSelect } from "#components/core/native-select";

type Position = "top" | "bottom" | "hidden";
interface Status {
  position: Position;
  backend: string | null;
  limitation: string | null;
}

export function OverlaySettings() {
  const [status, setStatus] = useState<Status | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void invoke<Status>("overlay_status")
      .then((value) => {
        if (active) setStatus(value);
      })
      .catch((error) => {
        if (active) setError(String(error));
      });
    return () => {
      active = false;
    };
  }, []);

  async function save(position: Position) {
    setPending(true);
    setError("");
    try {
      await invoke("overlay_save", { position });
      setStatus((previous) => previous && { ...previous, position });
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="settings-section" aria-labelledby="overlay-title">
      <h2 id="overlay-title" className="section-label">
        Recording overlay
      </h2>
      <p className="supporting">
        See when Chime is listening or transcribing without leaving your app.
      </p>
      {status && (
        <>
          <label className="flex flex-col gap-2">
            Position
            <NativeSelect
              value={status.position}
              disabled={pending}
              onChange={(event) => void save(event.target.value as Position)}
            >
              <option value="bottom">Bottom</option>
              <option value="top">Top</option>
              <option value="hidden">Hidden</option>
            </NativeSelect>
          </label>
          <Button
            variant="outline"
            disabled={pending || status.position === "hidden" || !!status.limitation}
            onClick={() => {
              setError("");
              void invoke("overlay_preview").catch((error) => setError(String(error)));
            }}
          >
            Preview overlay
          </Button>
          {status.limitation && (
            <p className="supporting" role="status">
              {status.limitation}
            </p>
          )}
        </>
      )}
      {!status && !error && (
        <p className="supporting" role="status">
          Loading overlay preferences…
        </p>
      )}
      {error && (
        <p className="supporting text-destructive" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
