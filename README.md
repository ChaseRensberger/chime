# Chime

Local voice dictation by Rensberger. Turn speech into text without sending your audio to the cloud.

## How it works

1. Press your shortcut to start recording.
2. Speak, then press it again to finish.
3. Chime transcribes on your device and pastes into the focused app.

You can also record from Chime's window and copy the result. Find previous transcripts in History.

## Downloads

[GitHub Releases](https://github.com/ChaseRensberger/chime/releases)

The first packaged release has not been published yet. Linux uses Wayland; macOS support is in development.

## Getting started

In **Settings → Speech model**, download the recommended Parakeet Unified English model (731 MB). You can cancel and resume the download. Once the model is ready, transcription works offline.

To use a compatible local GGUF speech model, choose **Import model…**. Chime checks and copies the file into its model folder without changing your original.

In your desktop's keyboard settings, assign **Alt+Space** to the command shown in Chime's Settings. Press it once to record and again to transcribe.

On Wayland, clipboard and paste use `wl-clipboard` and `wtype`. Automatic paste requires a desktop that supports Wayland virtual keyboards. If paste is unavailable, use **Copy** and paste manually.

## Privacy

Audio is processed on your device and is never saved. No account is required.

Transcripts stay on your device for seven days by default. Change retention, disable new history entries, or delete transcripts from History. Turning history off keeps existing entries until you delete them.

## Support

[Report a bug or request a feature](https://github.com/ChaseRensberger/chime/issues).
