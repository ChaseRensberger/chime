# Chime

Local voice dictation by Rensberger. Rust, Tauri, and React. Linux and macOS are the targets; COSMIC on Wayland is the first test environment.

## Run

Install Bun, Rust, CMake, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Arch Linux, native audio needs `alsa-lib` and `libpulse`. Wayland clipboard and paste use `wl-clipboard` and `wtype`.

Launch Chime, then use **Settings → Choose model** to select a downloaded [Parakeet Unified English Q8_0 model](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf) (731 MB):

```sh
bun install
bun run desktop:dev
```

Click **Start dictation**, speak, then **Finish**. Chime uses the default microphone and transcribes on CPU in an isolated worker. The model stays loaded between recordings. Recording stops at 60 seconds. Copy the result to use it elsewhere.

The in-window shortcut works on the Dictation page. Escape cancels dictation or returns from Settings. macOS still needs native verification; automatic paste currently targets Wayland.

Audio stays in memory. Successful transcripts are saved locally for 7 days by default. **History** provides search, copy, deletion, and retention controls: Off, 24 hours, 7 days, 30 days, or Forever. Off stops new entries without deleting existing ones. Storage is bounded to 10,000 entries, 16 KiB per transcript, and 16 MiB of text, matching Hex's limits.

The model location and preferences are saved locally. Chime does not download models or send audio over the network. The browser preview's history lasts only for its current session.

For the simulated browser preview, run `bun run dev` and open <http://127.0.0.1:1420>.

## COSMIC shortcut

Build with `bun run desktop:build --debug --no-bundle`. In COSMIC Settings, assign a custom keyboard shortcut to the executable command shown on Chime's Settings page. `--toggle` starts or finishes dictation without focusing Chime, then sends Ctrl+V to the focused app. During processing, it cancels instead.

Use `--toggle-terminal` for Ctrl+Shift+V. `--cancel` cancels without starting a recording; `--quit` exits Chime. Paste targets whichever app is focused when transcription finishes. If paste fails, the result remains available to copy. Desktops without the Wayland virtual-keyboard protocol need another paste backend.

## Development

`crates/chime-engine` wraps `transcribe.cpp` independently of Tauri. The desktop host owns capture through `handy-recorder`, session state, and worker cancellation. Recording bars follow microphone levels; the browser preview animates simulated levels.

```sh
bun run build
bun run lint
bun run format:check
bun test
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path crates/chime-engine/Cargo.toml
```

The ignored `default_microphone_smoke` test opens the default input for half a second. Run it explicitly with `cargo test --manifest-path src-tauri/Cargo.toml default_microphone_smoke -- --ignored`.
