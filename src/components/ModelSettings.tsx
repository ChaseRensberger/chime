import { useEffect, useState } from "react";
import { Button } from "#components/core/button";
import type { ModelService, ModelSnapshot } from "../engine/models";

const stages: Record<string, string> = {
  preparing: "Preparing",
  choosing: "Choose a model file",
  downloading: "Downloading",
  verifying: "Verifying download",
  importing: "Importing",
  loading: "Loading model",
  ready: "Ready",
  stopped: "Stopped",
};

function megabytes(bytes: number) {
  return `${Math.round(bytes / 1_000_000)} MB`;
}

export function ModelSettings({ source, busy }: { source: ModelService; busy: boolean }) {
  const [snapshot, setSnapshot] = useState<ModelSnapshot | null>(null);
  const [error, setError] = useState("");
  const [connectionError, setConnectionError] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const next = await source.snapshot();
        if (!disposed) {
          setSnapshot(next);
          setConnectionError("");
        }
      } catch (error) {
        if (!disposed) setConnectionError(String(error));
      }
      if (!disposed) timer = setTimeout(refresh, 500);
    }
    void refresh();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [source]);

  async function run(action: "download" | "import" | "use" | "cancel", id?: string) {
    setPending(true);
    setError("");
    try {
      if (action === "cancel") await source.cancel();
      else await source.start(action, id);
      setSnapshot(await source.snapshot());
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  }

  const progress = snapshot?.progress;
  const working = pending || !!progress?.busy;
  const target = snapshot?.entries.find((entry) => entry.id === progress?.modelId);
  return (
    <div className="flex flex-col gap-4">
      <p className="supporting">
        Download a speech model or import a compatible GGUF file. After setup, dictation works
        offline.
      </p>
      {!snapshot && (
        <p role="status" className="supporting">
          Loading models…
        </p>
      )}
      {snapshot?.entries.map((model) => (
        <div
          key={model.id}
          className="flex flex-wrap items-center justify-between gap-3 rounded-sm border p-4"
        >
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <p className="break-words text-sm">{model.name}</p>
            <p className="supporting">
              {megabytes(model.size)}
              {" · "}
              {model.languages ?? "Local file"}
              {model.recommended ? " · Recommended" : ""}
              {model.active ? " · Selected" : model.installed ? " · Installed" : ""}
            </p>
          </div>
          <Button
            variant={model.installed ? "outline" : "default"}
            disabled={busy || working || (model.active && !model.languages)}
            onClick={() =>
              void run(model.installed && !model.active ? "use" : "download", model.id)
            }
          >
            {model.active
              ? model.languages
                ? "Verify / repair"
                : "Selected"
              : model.installed
                ? "Use"
                : model.partialSize > 0
                  ? "Resume download"
                  : "Download"}
          </Button>
        </div>
      ))}
      {progress?.busy && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3">
            <p role="status" className="supporting">
              {stages[progress.stage] ?? "Preparing"}
              {target ? ` · ${target.name}` : ""}
            </p>
            <Button
              variant="ghost"
              size="sm"
              disabled={pending || progress.stage === "choosing"}
              onClick={() => void run("cancel")}
            >
              Cancel
            </Button>
          </div>
          {progress.total > 0 && (
            <>
              <progress
                className="h-2 w-full accent-link"
                aria-label={stages[progress.stage] ?? "Model progress"}
                max={progress.total}
                value={progress.downloaded}
              />
              <p className="supporting tabular-nums">
                {megabytes(progress.downloaded)} / {megabytes(progress.total)}
              </p>
            </>
          )}
        </div>
      )}
      {(error || connectionError || progress?.error) && (
        <p role="alert" className="supporting text-destructive">
          {error || connectionError || progress?.error}
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" disabled={busy || working} onClick={() => void run("import")}>
          Import model…
        </Button>
        {progress?.error && target?.languages && target.installed && (
          <Button
            variant="ghost"
            disabled={busy || working}
            onClick={() => void run("download", target.id)}
          >
            Check / download {target.name}
          </Button>
        )}
      </div>
      <p className="supporting">
        Imports are copied into Chime’s model folder. Your original file stays where it is.
      </p>
      <p className="supporting">
        Recommended model:{" "}
        <a
          className="underline underline-offset-4"
          href="https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf"
          target="_blank"
          rel="noreferrer"
          onClick={(event) => {
            event.preventDefault();
            void source.openInfo("source").catch((error) => setError(String(error)));
          }}
        >
          Hugging Face
        </a>
        {" · "}
        <a
          className="underline underline-offset-4"
          href="https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/"
          target="_blank"
          rel="noreferrer"
          onClick={(event) => {
            event.preventDefault();
            void source.openInfo("license").catch((error) => setError(String(error)));
          }}
        >
          NVIDIA Open Model License
        </a>
      </p>
    </div>
  );
}
