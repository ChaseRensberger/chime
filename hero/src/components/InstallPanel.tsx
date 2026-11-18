import { useEffect, useState } from "react";
import { DownloadSimpleIcon } from "@phosphor-icons/react";
import { buttonVariants } from "#components/core/button-variants";

function Choices<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <fieldset className="grid min-w-0 grid-cols-2 rounded-sm bg-muted p-1">
      <legend className="sr-only">{label}</legend>
      {options.map((option) => (
        <label key={option.value} className="relative cursor-pointer">
          <input
            className="peer sr-only"
            type="radio"
            name={label}
            value={option.value}
            checked={value === option.value}
            onChange={() => onChange(option.value)}
          />
          <span className="flex min-h-11 items-center justify-center rounded-sm border border-transparent px-2 text-xs text-muted-foreground hover:text-foreground peer-checked:border-input peer-checked:bg-card peer-checked:font-semibold peer-checked:text-foreground peer-focus-visible:outline-2 peer-focus-visible:outline-offset-1 peer-focus-visible:outline-ring">
            {option.label}
          </span>
        </label>
      ))}
    </fieldset>
  );
}

export function InstallPanel({ repository }: { repository: string }) {
  const [platform, setPlatform] = useState<"linux" | "macos">("linux");
  const [format, setFormat] = useState<"deb" | "appimage">("deb");
  const [assets, setAssets] = useState<Partial<Record<"deb" | "appimage" | "dmg", string>>>({});
  useEffect(() => {
    const abort = new AbortController();
    async function load() {
      const response = await fetch(
        `https://api.github.com/repos${new URL(repository).pathname}/releases/latest`,
        { signal: abort.signal },
      );
      if (!response.ok) return;
      const release = await response.json();
      const found: typeof assets = {};
      for (const asset of release.assets ?? []) {
        if (
          asset.state !== "uploaded" ||
          !asset.browser_download_url?.startsWith(`${repository}/releases/download/`)
        )
          continue;
        const kind = /(?:amd64|x86_64)\.deb$/i.test(asset.name)
          ? "deb"
          : /(?:amd64|x86_64)\.AppImage$/i.test(asset.name)
            ? "appimage"
            : /(?:arm64|aarch64|universal)\.dmg$/i.test(asset.name)
              ? "dmg"
              : null;
        if (kind) found[kind] = asset.browser_download_url;
      }
      if (!abort.signal.aborted) {
        setAssets(found);
        setPlatform(!found.deb && !found.appimage && found.dmg ? "macos" : "linux");
        setFormat(found.deb ? "deb" : "appimage");
      }
    }
    void load().catch(() => {});
    return () => abort.abort();
  }, [repository]);
  const download = assets[platform === "macos" ? "dmg" : format];
  return (
    <section
      aria-labelledby="download-title"
      className="w-full max-w-2xl rounded-md border bg-card p-4 text-left sm:px-6 sm:py-5"
    >
      <div className="mb-3 flex items-baseline justify-between gap-3 border-b pb-3">
        <h2 id="download-title" className="text-base font-semibold tracking-tight">
          Download
        </h2>
        {download && (
          <span className="text-xs text-muted-foreground">
            {platform === "linux" ? "Linux · x64" : "Apple silicon"}
          </span>
        )}
      </div>
      {download && (
        <div className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-4 gap-y-2">
          <span aria-hidden="true" className="text-xs text-muted-foreground">
            Platform
          </span>
          <Choices
            label="Platform"
            value={platform}
            options={(
              [
                { value: "linux", label: "Linux" },
                { value: "macos", label: "macOS" },
              ] as const
            ).filter((option) =>
              option.value === "linux" ? assets.deb || assets.appimage : assets.dmg,
            )}
            onChange={setPlatform}
          />
          <span className="text-xs text-muted-foreground">Package</span>
          {platform === "macos" ? (
            <span className="flex min-h-[3.25rem] items-center px-3 text-xs">
              Disk image (.dmg)
            </span>
          ) : (
            <Choices
              label="Package"
              value={format}
              options={(
                [
                  { value: "deb", label: "Debian" },
                  { value: "appimage", label: "AppImage" },
                ] as const
              ).filter((option) => assets[option.value])}
              onChange={setFormat}
            />
          )}
        </div>
      )}
      <a
        href={download ?? `${repository}/releases`}
        className={`${buttonVariants()} mt-4 min-h-11 w-full focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2`}
      >
        <DownloadSimpleIcon aria-hidden="true" />{" "}
        {download
          ? `Get ${platform === "macos" ? "macOS Apple silicon" : `Linux x64 ${format === "deb" ? "Debian" : "AppImage"}`}`
          : "View releases"}
      </a>
    </section>
  );
}
