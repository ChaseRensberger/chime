import { useState } from "react";
import { DownloadSimple } from "@phosphor-icons/react";
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
    <fieldset className="flex min-w-0 flex-wrap items-center gap-2">
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
          <span className="flex min-h-11 items-center justify-center rounded-sm border border-transparent px-3 text-xs text-muted-foreground transition-colors hover:bg-muted peer-checked:border-border peer-checked:bg-muted peer-checked:text-foreground peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-ring">
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
  return (
    <section aria-label="Install Chime" className="w-full max-w-lg text-left">
      <div className="flex justify-center border-b pb-3">
        <Choices
          label="Platform"
          value={platform}
          options={[
            { value: "linux", label: "Linux" },
            { value: "macos", label: "macOS" },
          ]}
          onChange={setPlatform}
        />
      </div>
      <div className="flex flex-col gap-4 py-4">
        <div className="grid items-center gap-2 sm:grid-cols-[8rem_1fr]">
          <span className="text-xs text-muted-foreground">Architecture</span>
          <span className="flex min-h-11 items-center px-3 text-xs">
            {platform === "linux" ? "x64" : "Apple silicon"}
          </span>
        </div>
        <div className="grid items-center gap-2 sm:grid-cols-[8rem_1fr]">
          <span className="text-xs text-muted-foreground">Package</span>
          {platform === "macos" ? (
            <span className="flex min-h-11 items-center px-3 text-xs">DMG</span>
          ) : (
            <Choices
              label="Package"
              value={format}
              options={[
                { value: "deb", label: "Debian" },
                { value: "appimage", label: "AppImage" },
              ]}
              onChange={setFormat}
            />
          )}
        </div>
        <a
          href={`${repository}/releases`}
          className={`${buttonVariants({ variant: "outline" })} min-h-11 w-full`}
        >
          <DownloadSimple aria-hidden="true" /> Get{" "}
          {platform === "macos"
            ? "macOS Apple silicon"
            : `Linux x64 ${format === "deb" ? "Debian" : "AppImage"}`}
        </a>
      </div>
    </section>
  );
}
