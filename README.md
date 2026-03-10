# Ribbit

Voice-to-text dictation for Windows. Press a hotkey, speak, release — transcribed text is pasted into any active window.

Uses [Groq](https://groq.com/) Whisper API (free tier) for fast, accurate transcription.

![Ribbit window](https://raw.githubusercontent.com/olegperegudov/ribbit/main/src/frog.png)

## How it works

1. Press **Ctrl+Alt+Space** (customizable) anywhere on your desktop
2. Speak into your microphone
3. Press the hotkey again to stop recording
4. Ribbit transcribes your audio and pastes the text into the active window via Ctrl+V

All transcriptions are saved in a local log with timestamps, grouped by date.

## Quick start

### 1. Download

Grab the latest installer from [Releases](https://github.com/olegperegudov/ribbit/releases/latest) and run `Ribbit_x.x.x_x64-setup.exe`.

### 2. Get a free Groq API key

1. Go to [console.groq.com](https://console.groq.com/keys) and sign up (free, no credit card)
2. Click **API Keys** in the left menu, then **Create API Key**
3. Copy the key (starts with `gsk_`)

### 3. Paste the key into Ribbit

When you first launch Ribbit, you'll see a setup screen. Paste your key and click **Save**. Done.

## Features

- **Global hotkey** — works from any app, even when Ribbit is minimized
- **Auto-paste** — transcribed text goes straight into your active window
- **Transcription log** — searchable history with timestamps, grouped by date
- **Sound packs** — frog or ping audio feedback (switch in settings)
- **System tray** — runs quietly in the background, X hides to tray
- **Auto-update** — checks for updates daily, one-click install from settings
- **Customizable hotkey** — change it in settings (click the hotkey field, press your combo, hit Enter)
- **Always on top** — toggle in settings
- **Lightweight** — ~4 MB installer, minimal resource usage

## Settings

Click the gear icon to access:

| Setting | Description |
|---------|-------------|
| **Hotkey** | Click to customize, press new combo, Enter to save |
| **Always on top** | Keep window above other apps |
| **Sound** | Choose between frog and ping |
| **Debug log** | View internal logs for troubleshooting |
| **Version** | Click to view changelog on GitHub |
| **Check update** | Manually check for new versions |

## Tech stack

- [Tauri 2](https://tauri.app/) — Rust backend, HTML/CSS/JS frontend
- [Groq Whisper API](https://console.groq.com/docs/speech-text) — `whisper-large-v3-turbo` model
- [CPAL](https://github.com/RustAudio/cpal) — cross-platform audio input
- [Rodio](https://github.com/RustAudio/rodio) — audio playback
- [Enigo](https://github.com/enigo-rs/enigo) — keyboard simulation

## Building from source

```bash
# Prerequisites: Node.js, Rust toolchain
npm install
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/nsis/`.

## License

MIT
