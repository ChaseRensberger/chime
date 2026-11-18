import { realpath } from "node:fs/promises";
import { resolve } from "node:path";

// dlopen libraries are invisible to linuxdeploy's ELF dependency scan.
// Explicitly include layer-shell in AppImages; Debian installs it as a dependency.
let config = {};
if (process.platform === "linux") {
  const machine = process.arch === "x64" ? 62 : process.arch === "arm64" ? 183 : null;
  if (!machine) throw new Error(`Unsupported Linux architecture: ${process.arch}`);
  async function matchesMachine(path: string) {
    const header = Buffer.from(await Bun.file(path).slice(0, 20).arrayBuffer());
    return (
      header.length === 20 &&
      header.readUInt32BE(0) === 0x7f454c46 &&
      header[4] === 2 &&
      header[5] === 1 &&
      header.readUInt16LE(18) === machine
    );
  }
  let library = process.env.CHIME_LAYER_SHELL_LIBRARY;
  if (!library) {
    const result = Bun.spawnSync(["ldconfig", "-p"]);
    if (result.exitCode !== 0)
      throw new Error("Could not locate the layer-shell runtime with ldconfig.");
    for (const line of result.stdout.toString().split("\n")) {
      const path = line.match(/^\s*libgtk-layer-shell\.so\.0\s+.*=> (.+)$/)?.[1]?.trim();
      if (path && (await matchesMachine(path))) {
        library = path;
        break;
      }
    }
  }
  if (!library)
    throw new Error("Install the gtk-layer-shell runtime before building Linux packages.");
  const source = await realpath(library);
  if (!(await matchesMachine(source))) throw new Error(`Wrong library architecture: ${source}`);
  config = {
    bundle: { linux: { appimage: { files: { "/usr/lib/libgtk-layer-shell.so.0": source } } } },
  };
}
await Bun.write(
  resolve(import.meta.dir, "../src-tauri/target/overlay-bundle.json"),
  JSON.stringify(config, null, 2),
);
