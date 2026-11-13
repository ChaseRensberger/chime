import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { prepareRelease, versionFromTag } from "../scripts/prepare-release";

test("release tags accept stable and prerelease versions but reject ambiguous versions", () => {
  expect(versionFromTag("v1.2.3")).toBe("1.2.3");
  expect(versionFromTag("v1.2.3-beta.1")).toBe("1.2.3-beta.1");
  for (const tag of ["1.2.3", "v01.2.3", "v1.2", "v1.2.3-beta.01", "v1.2.3+build", "v1.2.3\n"]) {
    expect(() => versionFromTag(tag)).toThrow();
  }
});

test("release stamping preserves formatting, dependencies, and the independent engine version", async () => {
  const root = await mkdtemp(join(tmpdir(), "chime-release-"));
  const repo = resolve(import.meta.dir, "..");
  const paths = [
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
  ];
  const originals = await Promise.all(paths.map((path) => Bun.file(resolve(repo, path)).text()));
  try {
    for (const [i, path] of paths.entries()) await Bun.write(resolve(root, path), originals[i]);
    await expect(prepareRelease(root, "v01.2.3")).rejects.toThrow();
    expect(await Bun.file(resolve(root, "package.json")).text()).toBe(originals[0]);

    for (const tag of ["v9.8.7", "v9.8.8-beta.1"]) {
      const version = await prepareRelease(root, tag);
      for (const path of paths.slice(0, 2)) {
        expect((await Bun.file(resolve(root, path)).json()).version).toBe(version);
      }
      const cargo = Bun.TOML.parse(await Bun.file(resolve(root, paths[2])).text()) as {
        package: { version: string };
      };
      expect(cargo.package.version).toBe(version);
      const lock = Bun.TOML.parse(await Bun.file(resolve(root, paths[3])).text()) as {
        package: { name: string; version: string }[];
      };
      const originalLock = Bun.TOML.parse(originals[3]) as typeof lock;
      expect(lock.package.find((pkg) => pkg.name === "chime-desktop")?.version).toBe(version);
      expect(lock.package.filter((pkg) => pkg.name !== "chime-desktop")).toEqual(
        originalLock.package.filter((pkg) => pkg.name !== "chime-desktop"),
      );
      // A repeat run is safe, and the stamped files still pass the normal formatter.
      const before = await Promise.all(paths.map((path) => Bun.file(resolve(root, path)).text()));
      await prepareRelease(root, tag);
      expect(await Promise.all(paths.map((path) => Bun.file(resolve(root, path)).text()))).toEqual(
        before,
      );
      const format = Bun.spawnSync(
        [resolve(repo, "node_modules/.bin/oxfmt"), "--check", ...paths.slice(0, 2)],
        { cwd: root },
      );
      expect(format.exitCode).toBe(0);
    }

    await Bun.write(resolve(root, paths[3]), "version = 4\n");
    const before = await Bun.file(resolve(root, paths[0])).text();
    await expect(prepareRelease(root, "v9.9.9")).rejects.toThrow("Could not locate app version");
    expect(await Bun.file(resolve(root, paths[0])).text()).toBe(before);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
