import { resolve } from "node:path";

export function versionFromTag(tag: string): string {
  const match =
    /^v((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?)$/.exec(
      tag,
    );
  if (!match || match[0] !== tag || match[2]?.split(".").some((part) => /^0\d+$/.test(part))) {
    throw new Error("Use a release tag such as v0.2.0 or v0.2.0-beta.1 (without build metadata).");
  }
  return match[1];
}

/** Stamp the release checkout without resolving or upgrading dependencies. */
export async function prepareRelease(root: string, tag: string) {
  const version = versionFromTag(tag);
  const paths = [
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
  ];
  const updates = await Promise.all(
    paths.map(async (path) => {
      const file = Bun.file(resolve(root, path));
      const text = await file.text();
      let pattern: RegExp;
      if (path.endsWith(".json")) {
        if (typeof JSON.parse(text).version !== "string")
          throw new Error(`Missing version in ${path}`);
        pattern = /^(  "version": ")[^"]+("[,\r]*$)/m;
      } else if (path.endsWith("Cargo.toml")) {
        pattern = /(\[package\][\s\S]*?^version = ")[^"]+(")/m;
      } else {
        pattern = /(\[\[package\]\]\r?\nname = "chime-desktop"\r?\nversion = ")[^"]+(")/;
      }
      if (!pattern.test(text)) throw new Error(`Could not locate app version in ${path}`);
      return {
        file,
        text: text.replace(pattern, (_, before, after) => `${before}${version}${after}`),
      };
    }),
  );
  // Validate all inputs before changing any files.
  for (const update of updates) await Bun.write(update.file, update.text);
  return version;
}

if (import.meta.main) {
  try {
    if (process.argv.length !== 3) throw new Error("Usage: bun run release:prepare <vX.Y.Z>");
    const version = await prepareRelease(resolve(import.meta.dir, ".."), process.argv[2]);
    console.log(`Prepared release ${version} in this checkout. No commit or tag was created.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
