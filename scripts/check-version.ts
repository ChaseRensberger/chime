import { resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const pkg = await Bun.file(resolve(root, "package.json")).json();
const tauri = await Bun.file(resolve(root, "src-tauri/tauri.conf.json")).json();
const cargo = Bun.TOML.parse(await Bun.file(resolve(root, "src-tauri/Cargo.toml")).text()) as {
  package: { version: string };
};
const versions = {
  "package.json": pkg.version,
  "src-tauri/tauri.conf.json": tauri.version,
  "src-tauri/Cargo.toml": cargo.package.version,
};
const tag = process.argv[2];

if (new Set(Object.values(versions)).size !== 1 || (tag && tag !== `v${pkg.version}`)) {
  console.error("App versions must match each other and the release tag:", versions);
  if (tag) console.error(`Received ${tag}; expected v${pkg.version}`);
  process.exit(1);
}
console.log(`App version ${pkg.version}${tag ? ` matches ${tag}` : " is consistent"}.`);
