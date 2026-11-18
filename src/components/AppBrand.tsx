import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import logo from "../../src-tauri/icons/tray.svg";

export function AppBrand({ demo }: { demo: boolean }) {
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<string | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (demo) return;
    let active = true;
    async function check() {
      try {
        const tag = await invoke<string | null>("update_check");
        if (active) setUpdate(tag);
      } catch {
        return;
      }
    }
    void getVersion()
      .then((v) => {
        if (active) setVersion(v);
      })
      .catch(() => {});
    const initial = setTimeout(check, 1000);
    const timer = setInterval(check, 60 * 60 * 1000);
    return () => {
      active = false;
      clearTimeout(initial);
      clearInterval(timer);
    };
  }, [demo]);
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
      <img src={logo} alt="" width="24" height="24" />
      <span className="text-lg font-semibold tracking-tight">Chime</span>
      {version && <span className="text-xs text-muted-foreground">v{version}</span>}
      {update && (
        <button
          className="text-xs underline"
          onClick={() => {
            setError("");
            void invoke("update_download").catch(() => setError("Couldn’t open the download."));
          }}
        >
          Download {update}
        </button>
      )}
      {error && (
        <span role="alert" className="text-xs">
          {error}
        </span>
      )}
    </div>
  );
}
