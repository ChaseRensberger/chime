import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChoiceSelect } from "./ChoiceSelect";

interface Listing {
  devices: { id: string; name: string; default: boolean; occurrence: number }[];
  selected: string | null;
  missing: string | null;
}

export function MicrophonePicker({ demo, busy }: { demo: boolean; busy: boolean }) {
  const [listing, setListing] = useState<Listing | null>(null);
  const [listError, setListError] = useState("");
  const [saveError, setSaveError] = useState("");
  const [saving, setSaving] = useState(false);
  const refresh = useRef(() => {});
  useEffect(() => {
    if (demo) return;
    let active = true,
      pending = false,
      again = false;
    async function load() {
      if (pending) {
        again = true;
        return;
      }
      pending = true;
      try {
        const next = await invoke<Listing>("microphones_list");
        if (active) {
          setListing(next);
          setListError("");
        }
      } catch (e) {
        if (active) setListError(String(e));
      } finally {
        pending = false;
        if (active && again) {
          again = false;
          void load();
        }
      }
    }
    refresh.current = load;
    void load();
    window.addEventListener("focus", load);
    const timer = setInterval(() => {
      if (document.hasFocus()) void load();
    }, 5000);
    return () => {
      active = false;
      refresh.current = () => {};
      clearInterval(timer);
      window.removeEventListener("focus", load);
    };
  }, [demo]);
  const current = listing?.devices.find((d) => d.default);
  const hint = demo
    ? "Preview · Microphone off"
    : listing && !listing.devices.length
      ? "No microphone detected."
      : listing?.missing
        ? "Using the system default until it reconnects."
        : null;
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor="microphone">Microphone</label>
      <ChoiceSelect
        id="microphone"
        aria-describedby={hint ? "microphone-help" : undefined}
        disabled={demo || busy || saving || !listing}
        value={listing?.missing ? "missing" : (listing?.selected ?? "")}
        onOpen={() => refresh.current()}
        onChange={async (value) => {
          const id = value || null;
          setSaving(true);
          setSaveError("");
          try {
            await invoke("microphone_select", { id });
            refresh.current();
          } catch (e) {
            setSaveError(String(e));
          } finally {
            setSaving(false);
          }
        }}
        options={[
          { value: "", label: `System default${current ? ` · ${current.name}` : ""}` },
          ...(listing?.missing
            ? [{ value: "missing", label: `${listing.missing} · Disconnected`, disabled: true }]
            : []),
          ...(listing?.devices ?? []).map((d) => ({
            value: d.id,
            label: `${d.name}${d.occurrence ? ` (${d.occurrence + 1})` : ""}`,
          })),
        ]}
      />
      {hint && (
        <p id="microphone-help" className="supporting">
          {hint}
        </p>
      )}
      {(saveError || listError) && (
        <p role="alert" className="supporting text-destructive">
          {saveError || listError}
        </p>
      )}
    </div>
  );
}
