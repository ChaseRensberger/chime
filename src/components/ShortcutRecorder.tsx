import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { Button } from "#components/core/button";

export function ShortcutRecorder({ busy, enabled }: { busy: boolean; enabled: boolean }) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState("");
  const [mods, setMods] = useState("");
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!recording) return;
    let active = true;
    let saving = false;
    const listener = listen("shortcut-current", () => {
      if (active && !saving) {
        setHint("Shortcut unchanged.");
        setRecording(false);
      }
    });
    const ready = listener.then(() => {
      if (active) return invoke("shortcut_record", { recording: true });
    });
    void ready.catch(() => {
      if (active) {
        setRecording(false);
        setHint("Couldn’t start recording a shortcut.");
      }
    });
    function end() {
      setRecording(false);
    }
    async function key(event: KeyboardEvent) {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (saving || event.repeat) return;
      if (event.code === "Escape") {
        end();
        return;
      }
      const modifiers = [
        ...(event.ctrlKey ? ["Control"] : []),
        ...(event.altKey ? ["Alt"] : []),
        ...(event.shiftKey ? ["Shift"] : []),
        ...(event.metaKey ? ["Super"] : []),
      ];
      setMods(
        modifiers
          .map((m) => (m === "Super" ? "⌘" : m === "Alt" ? "⌥" : m === "Control" ? "⌃" : "⇧"))
          .join(" "),
      );
      if (event.type === "keyup" || /^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code))
        return;
      if (!modifiers.length && !/^F(?:[1-9]|1\d|2[0-4])$/.test(event.code)) {
        setHint("Add a modifier, such as Option or Command.");
        return;
      }
      saving = true;
      try {
        await ready;
        if (!active) return;
        await invoke("shortcut_bind", { chord: [...modifiers, event.code].join("+") });
        if (active) {
          setHint("Shortcut saved.");
          end();
        }
      } catch (error) {
        if (active) setHint(String(error));
      } finally {
        saving = false;
      }
    }
    window.addEventListener("keydown", key, true);
    window.addEventListener("keyup", key, true);
    window.addEventListener("blur", end);
    return () => {
      active = false;
      void listener.then((stop) => stop()).catch(() => {});
      void ready.then(() => invoke("shortcut_record", { recording: false })).catch(() => {});
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("keyup", key, true);
      window.removeEventListener("blur", end);
    };
  }, [recording]);
  return (
    <>
      <Button
        ref={button}
        variant="outline"
        disabled={busy}
        onBlur={() => setRecording(false)}
        onClick={() => {
          setHint("");
          setMods("");
          setRecording(!recording);
          button.current?.focus();
        }}
      >
        {recording
          ? `${mods || "Press your shortcut"} · Esc to cancel`
          : enabled
            ? "Change shortcut…"
            : "Set shortcut…"}
      </Button>
      {hint && (
        <p role="status" className="supporting">
          {hint}
        </p>
      )}
    </>
  );
}
