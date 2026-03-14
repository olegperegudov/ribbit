<p align="center">
  <img src="src/frog.png" width="96" alt="Ribbit logo" />
</p>

<h1 align="center">Ribbit</h1>

<p align="center">
  Simple, local voice-to-text dictation for Windows and macOS.<br/>
  Hold a hotkey, speak, release — transcribed text is pasted into any active window.
</p>

<p align="center">
  <i>Hold the key, croak your thoughts,<br/>
  Ribbit turns your quacks to code.<br/>
  Bugs get squashed just like they should —<br/>
  Pray the transcription comes out good!</i> 🐸
</p>

<p align="center">
  <b>Free to use</b> — just needs a free <a href="https://console.groq.com/keys">Groq</a> account (no credit card, generous free tier)<br/>
  <b>Private by design</b> — no cloud storage, no telemetry, your data stays on your machine
</p>

## How it works

1. **Hold** **Ctrl+Alt+Space** (customizable, **Cmd+Alt+Space** on macOS) anywhere on your desktop
2. Speak into your microphone
3. **Release** to stop recording
4. Ribbit transcribes your audio and pastes the text into the active window

All transcriptions are saved in a local log with timestamps, grouped by date.

## Quick start

### 1. Download

Grab the latest installer from [Releases](https://github.com/olegperegudov/ribbit/releases/latest):
- **Windows** — `Ribbit_x.x.x_x64-setup.exe`
- **macOS** — `Ribbit_x.x.x_aarch64.dmg` (Apple Silicon) or `Ribbit_x.x.x_x64.dmg` (Intel)

### 2. Get a free Groq API key

1. Go to [console.groq.com](https://console.groq.com/keys) and sign up (free, no credit card needed)
2. Click **API Keys** in the left menu, then **Create API Key**
3. Copy the key (starts with `gsk_`)

### 3. Paste the key into Ribbit

When you first launch Ribbit, you'll see a setup screen. Paste your key and click **Save**. Done.

## Features

- **Hold-to-record** — hold the hotkey to record, release to transcribe
- **Auto-paste** — transcribed text goes straight into your active window via Ctrl+V
- **Works everywhere** — global hotkey works from any app, even when Ribbit is minimized
- **Local and private** — no cloud sync, no data collection (only a free Groq account needed for the API)
- **Transcription log** — history with timestamps, grouped by date
- **Sound packs** — frog or ping audio feedback (switch in settings)
- **System tray** — runs quietly in the background, X hides to tray
- **Auto-update** — checks for updates daily, one-click install from settings
- **Customizable hotkey** — click the hotkey field in settings, press your combo, hit Enter
- **Lightweight** — ~4 MB installer, minimal resource usage

## Privacy

- Audio is sent to Groq's API for transcription only — not stored on their servers
- Your API key is saved locally in `%LOCALAPPDATA%/ribbit/.env`
- Transcription history is stored locally in `%LOCALAPPDATA%/ribbit/logs/`
- No analytics, no tracking, no telemetry — zero network calls besides the Groq API
- Fully open source — inspect every line of code

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
