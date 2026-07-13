<p align="center">
  <img src="src/frog.png" width="96" alt="Ribbit logo" />
</p>

<h1 align="center">Ribbit</h1>

<p align="center">
  Voice to text, anywhere on your desktop.<br/>
  Hold a hotkey, speak, let go — the text lands in the window you were already typing in.
</p>

<p align="center">
  <b>Free</b> — runs on a free <a href="https://console.groq.com/keys">Groq</a> key, no card<br/>
  <b>Yours</b> — the log lives on your disk, no cloud, no telemetry
</p>

## Get it

<p align="center">
  <a href="https://github.com/olegperegudov/ribbit/releases/latest/download/Ribbit_macOS_AppleSilicon.dmg"><img src="https://img.shields.io/badge/Download_for_macOS-Apple_Silicon-000?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS, Apple Silicon" /></a>&nbsp;
  <a href="https://github.com/olegperegudov/ribbit/releases/latest/download/Ribbit_macOS_Intel.dmg"><img src="https://img.shields.io/badge/Download_for_macOS-Intel-666?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS, Intel" /></a>&nbsp;
  <a href="https://github.com/olegperegudov/ribbit/releases/latest/download/Ribbit_Windows_Setup.exe"><img src="https://img.shields.io/badge/Download_for-Windows-0078D4?style=for-the-badge&logo=windows&logoColor=white" alt="Download for Windows" /></a>
</p>

Each button downloads the latest installer for that platform. Want an older build? Every version is on the [releases page](https://github.com/olegperegudov/ribbit/releases).

Then:

1. **Open it.** Apple isn't paid to trust us, so the first launch claims the app is *"damaged"*. It isn't — run `xattr -cr /Applications/Ribbit.app` once in Terminal, then open it normally. Updates after that install themselves.
2. **Paste a key.** Ribbit asks for one on the first launch: [console.groq.com](https://console.groq.com/keys) → *API Keys* → *Create API Key* → copy the `gsk_…` string. Free, no card.
3. **Hold ⌃⌥Space, speak, let go.** The text is typed into whatever window you were in. On Windows: `Ctrl+Alt+Space`.

Ribbit is built and used on macOS. The Windows build exists and installs, but it isn't tested nearly as much — expect rough edges.

## Everything you said, kept

Every dictation lands in a log, grouped by day. Click a line — it's on your clipboard.

![The transcript log](docs/screenshots/log.png)

## A word it keeps mishearing — fix it once

Transcription mangles names and jargon. Select the mangled word right there in the log and type what it should have been. From then on Ribbit swaps it silently, every time.

![Replacing a misheard word](docs/screenshots/vocab.png)

The whole list lives under the gear — add, search, remove.

![The vocabulary](docs/screenshots/vocabulary.png)

## Never wait on one provider

Add as many as you like: the top one does the work, the rest wait for the day it doesn't. After enough failures in a row — rate limit, outage, timeout — Ribbit drops to the next one, then climbs back to the top once the cooldown passes. Both numbers are yours. The model that polishes the text afterwards has the same stack.

![Speech providers and fallback](docs/screenshots/providers.png)

## Updates

The frog in the menu bar turns green when a new version is out. Click it, pick the update line — done.

## Privacy

- Audio goes to the provider you picked, for transcription, and nowhere else.
- Key, log and vocabulary stay in a folder on your machine.
- No analytics, no tracking, no other network calls.

## Under the hood

Stack, local build, tests, signing and the release pipeline → [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## License

MIT
