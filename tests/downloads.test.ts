import { expect, test } from "bun:test";
import { releaseDownloads } from "../hero/src/downloads";

test("release assets stay paired with the correct CPU and package", () => {
  const repository = "https://github.com/ChaseRensberger/chime";
  const url = (suffix: string) => `${repository}/releases/download/v0.0.1/Chime_0.0.1_${suffix}`;
  const assets = [
    "arm64.deb",
    "amd64.AppImage",
    "amd64.deb",
    "aarch64.AppImage",
    "arm64.dmg",
    "x86_64.dmg",
  ].map((suffix) => ({
    name: `Chime_0.0.1_${suffix}`,
    state: "uploaded",
    browser_download_url: url(suffix),
  }));
  expect(releaseDownloads(repository, assets)).toEqual({
    "linux-arm64": { deb: url("arm64.deb"), appimage: url("aarch64.AppImage") },
    "linux-x64": { deb: url("amd64.deb"), appimage: url("amd64.AppImage") },
    macos: { dmg: url("arm64.dmg") },
  });
  expect(releaseDownloads(repository, [assets[0]])).toEqual({
    "linux-arm64": { deb: url("arm64.deb") },
  });
});
