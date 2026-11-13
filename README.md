# Chime

Local voice dictation by Rensberger. Rust, Tauri, and React. Linux and macOS are the targets; COSMIC on Wayland is the first test environment.

## Run

Install Bun, Rust, CMake, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Arch Linux, native audio needs `alsa-lib` and `libpulse`. Wayland clipboard and paste use `wl-clipboard` and `wtype`.

Launch Chime, then use **Settings → Choose model** to select a downloaded [Parakeet Unified English Q8_0 model](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf) (731 MB):

```sh
bun install
bun run dev
```

Click **Start dictation**, speak, then **Finish**. Chime uses the default microphone and transcribes on CPU in an isolated worker. The model stays loaded between recordings. Recording stops at 60 seconds. Copy the result to use it elsewhere.

Alt+Space uses the COSMIC binding described below. Escape cancels dictation or returns from Settings. macOS still needs native verification; automatic paste currently targets Wayland.

Audio stays in memory. Successful transcripts are saved locally for 7 days by default. **History** provides search, copy, deletion, and retention controls: Off, 24 hours, 7 days, 30 days, or Forever. Off stops new entries without deleting existing ones. Storage is bounded to 10,000 entries, 16 KiB per transcript, and 16 MiB of text, matching Hex's limits.

The model location and preferences are saved locally. Chime does not download models or send audio over the network. The browser preview's history lasts only for its current session.

For the simulated browser preview, run `bun run preview` and open <http://127.0.0.1:1420>.

## COSMIC shortcut

Run the installed app, or build locally with `bun run build --debug`. In COSMIC Settings, assign Alt+Space to the command shown on Chime's Settings page. For AppImages, keep the file at a stable path before assigning the shortcut. Update the binding if you move it. `--toggle` starts or finishes dictation without focusing Chime, then sends Ctrl+V to the focused app. During processing, it cancels instead.

Use `--toggle-terminal` for Ctrl+Shift+V. `--cancel` cancels without starting a recording; `--quit` exits Chime. Paste targets whichever app is focused when transcription finishes. If paste fails, the result remains available to copy. Desktops without the Wayland virtual-keyboard protocol need another paste backend.

## Development

`crates/chime-engine` wraps `transcribe.cpp` independently of Tauri. The desktop host owns capture through `handy-recorder`, session state, and worker cancellation. Recording bars follow microphone levels; the browser preview animates simulated levels.

Use the Bun version in `package.json` and the Rust toolchain in `rust-toolchain.toml`.

| Command           | Purpose                                              |
| ----------------- | ---------------------------------------------------- |
| `bun run dev`     | Desktop app with live reload                         |
| `bun run preview` | Simulated browser demo                               |
| `bun run check`   | Types, lint, formatting, and app-version consistency |
| `bun run test`    | Frontend and both Rust crates' tests                 |
| `bun run format`  | Format TypeScript, configuration, and Rust           |
| `bun run build`   | Release executable without an installer              |
| `bun run bundle`  | Linux `.deb` and AppImage packages                   |

The `desktop:dev` and `desktop:build` aliases remain available. `bun test` runs only frontend tests.

The ignored `default_microphone_smoke` test opens the default input for half a second. Run it explicitly with `cargo test --manifest-path src-tauri/Cargo.toml default_microphone_smoke -- --ignored`.

## Releases

GitHub Actions checks pull requests and `main` on Ubuntu 22.04 and macOS. Tags matching `v*` build Linux x86_64 packages and upload them to a draft GitHub Release. The workflow uses the repository's automatic `GITHUB_TOKEN`; no additional release token is needed.

To prepare a release:

1. Set the same version in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`. The engine crate has an independent version.
2. Run `cargo check --manifest-path src-tauri/Cargo.toml` to update the desktop lockfile, then run `bun run check` and `bun run test`.
3. Commit the version changes and lockfile. Tag that commit, for example with `git tag v0.1.0`, then push the commit and tag.
4. Download and test the packages from the draft release, edit its notes, and publish it. Tags such as `v0.2.0-beta.1` produce prereleases.

Packages appear in `src-tauri/target/release/bundle/` when built locally. Official Linux builds use Ubuntu 22.04 as the compatibility baseline and require an AVX2-class CPU with FMA, F16C, and BMI2. The speech model is downloaded separately. Debian packages declare `wl-clipboard` and `wtype` dependencies; AppImage users must install those helpers themselves. Paste also requires compositor support for Wayland virtual keyboards.

macOS currently has a build check, not published installers. Native testing, signing, and notarization are still pending. CI does not exercise microphone capture or paste into other applications.
