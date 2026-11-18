export type Target = "linux-x64" | "linux-arm64" | "macos";
export type Format = "deb" | "appimage" | "dmg";
export type Downloads = Partial<Record<Target, Partial<Record<Format, string>>>>;
type Asset = { name: string; state: string; browser_download_url: string };

export function releaseDownloads(repository: string, assets: Asset[]): Downloads {
  const downloads: Downloads = {};
  for (const asset of assets) {
    if (
      asset.state !== "uploaded" ||
      !asset.browser_download_url?.startsWith(`${repository}/releases/download/`)
    )
      continue;
    const match = asset.name.match(
      /^Chime_.+_(amd64|x86_64|arm64|aarch64|universal)\.(deb|AppImage|dmg)$/i,
    );
    if (!match) continue;
    const arch = match[1].toLowerCase();
    const format = match[2].toLowerCase() as Format;
    const x64 = arch === "amd64" || arch === "x86_64";
    if ((format === "dmg" && x64) || (format !== "dmg" && arch === "universal")) continue;
    const target = format === "dmg" ? "macos" : x64 ? "linux-x64" : "linux-arm64";
    (downloads[target] ??= {})[format] = asset.browser_download_url;
  }
  return downloads;
}
