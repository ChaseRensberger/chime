import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const icons = join(import.meta.dir, "../src-tauri/icons");
const source = join(icons, "tray.svg");
const temp = await mkdtemp(join(tmpdir(), "chime-icons-"));
try {
  for (const sizes of [[], ["-p", "64", "-p", "256"]]) {
    const result = Bun.spawnSync(["bunx", "tauri", "icon", source, "-o", temp, ...sizes]);
    if (result.exitCode !== 0) throw new Error(result.stderr.toString());
  }
  for (const [from, to] of [
    ["icon.icns", "icon.icns"],
    ["256x256.png", "icon.png"],
    ["64x64.png", "tray.png"],
    ["64x64.png", "tray-linux.png"],
  ])
    await copyFile(join(temp, from), join(icons, to));
  for (const alias of ["icon.svg", "tray-linux.svg"]) await copyFile(source, join(icons, alias));
} finally {
  await rm(temp, { recursive: true, force: true });
}
