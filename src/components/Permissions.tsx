import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "#components/core/button";

const items = [
  ["microphone", "Microphone", "Record your voice for dictation."],
  ["accessibility", "Accessibility", "Send the paste shortcut to the focused app."],
  ["automation", "Automation", "Let Chime use System Events for automatic paste."],
] as const;
type Kind = (typeof items)[number][0];
type Status = Record<Kind, string>;

export function Permissions({ focus }: { focus: boolean }) {
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const visible = Boolean(status || error);
  useEffect(() => {
    if (focus && visible) document.getElementById("permissions-title")?.focus();
  }, [focus, visible]);
  useEffect(() => {
    let active = true;
    const refresh = () => {
      void invoke<Status | null>("permissions_status")
        .then((s) => {
          if (active) setStatus(s);
        })
        .catch(() => {
          if (active) setError("Couldn’t read permission settings.");
        });
    };
    refresh();
    window.addEventListener("focus", refresh);
    const timer = setInterval(() => {
      if (document.hasFocus()) refresh();
    }, 4000);
    return () => {
      active = false;
      window.removeEventListener("focus", refresh);
      clearInterval(timer);
    };
  }, []);
  if (!status && !error) return null;
  return (
    <section className="settings-section" aria-labelledby="permissions-title">
      <h2 tabIndex={-1} id="permissions-title" className="section-label">
        Permissions
      </h2>
      {status &&
        items.map(([kind, label, reason]) => (
          <div key={kind} className="flex items-center justify-between gap-3">
            <div>
              <p>
                {label} · {status[kind]}
              </p>
              <p className="supporting">{reason}</p>
            </div>
            {status[kind] !== "Allowed" && (
              <Button
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  setError("");
                  try {
                    await invoke("permissions_action", { kind });
                    setStatus(await invoke<Status>("permissions_status"));
                  } catch {
                    setError(
                      "Couldn’t open permission settings. Open Privacy & Security in System Settings.",
                    );
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                {status[kind] === "Needs access" ? "Open settings" : "Request access"}
              </Button>
            )}
          </div>
        ))}
      {error && (
        <p role="alert" className="supporting">
          {error}
        </p>
      )}
    </section>
  );
}
