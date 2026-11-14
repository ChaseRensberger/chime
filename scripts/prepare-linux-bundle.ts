import { realpath } from "node:fs/promises";
import { resolve } from "node:path";

// dlopen libraries are invisible to linuxdeploy's ELF dependency scan.
// Explicitly include layer-shell in AppImages; Debian installs it as a dependency.
let config = {};
if (process.platform === "linux") {
  let library = process.env.CHIME_LAYER_SHELL_LIBRARY;
  if (!library) {
    const result = Bun.spawnSync(["ldconfig", "-p"]);
    if (result.exitCode !== 0)
      throw new Error("Could not locate the layer-shell runtime with ldconfig.");
    library = result.stdout
      .toString()
      .split("\n")
      .find((line) => line.includes("libgtk-layer-shell.so.0 ") && line.includes("x86-64"))
      ?.split("=>")[1]
      ?.trim();
  }
  if (!library)
    throw new Error("Install the gtk-layer-shell runtime before building Linux packages.");
  const source = await realpath(library);
  config = {
    bundle: { linux: { appimage: { files: { "/usr/lib/libgtk-layer-shell.so.0": source } } } },
  };
}
await Bun.write(
  resolve(import.meta.dir, "../src-tauri/target/overlay-bundle.json"),
  JSON.stringify(config, null, 2),
);
