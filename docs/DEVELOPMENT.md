# Development

Everything a user does not need to know. The [README](../README.md) is the front door.

## Stack

- [Tauri 2](https://tauri.app/) — Rust backend, plain HTML/CSS/JS frontend, no bundler.
- Speech: any OpenAI-compatible endpoint. Default [Groq](https://console.groq.com/docs/speech-text) (`whisper-large-v3-turbo`).
- Text polish (optional): any OpenAI-compatible chat endpoint.
- [CPAL](https://github.com/RustAudio/cpal) — audio capture · [Rodio](https://github.com/RustAudio/rodio) — the quack.

Providers are a stack, not a setting: entry #1 is primary, the rest are fallbacks. The state machine lives in `src-tauri/src/fallback.rs` and is shared by both the speech and the text stack.

## Run it locally

```bash
npm install
npm run tauri dev
```

## Tests

```bash
npm test                           # frontend: vocabulary, search, window chrome
cd src-tauri && cargo test --lib   # backend: fallback, logger, vocab, config
```

Both run in CI before anything is built.

## Release

Every push to `main` is a release. CI bumps the patch version itself (`src-tauri/tauri.conf.json`, `Cargo.toml`, the version label in `src/index.html`), tags it, builds Windows and both macOS architectures, publishes the GitHub release and the `latest.json` the in-app updater reads.

Never bump the version by hand — CI does it, and a manual bump collides with its commit.

The macOS build is **per-arch, never universal**: a lipo'd bundle does not anchor the microphone grant, and the permission silently stops sticking. CI fails the build if `latest.json` ever points at a universal bundle.

Each release also carries version-less copies of the installers (`Ribbit_macOS_AppleSilicon.dmg`, `Ribbit_macOS_Intel.dmg`, `Ribbit_Windows_Setup.exe`) so the README buttons can link straight at a file that survives the next bump.

## Signing

macOS builds are signed with a stable self-signed certificate ("Ribbit Code Signing"), not ad-hoc. macOS binds the Accessibility and microphone grants to the *signature*, so a stable certificate means the user grants them once, at install, and never again — an ad-hoc signature changes with every build and re-asks after each update.

Not notarized (that needs a paid Apple account), so the first open still needs `xattr -cr`.

## Debugging

DevTools are off in release builds — `console.log` is invisible. Use the `js_debug_log` command instead; it writes into the debug log the user can open under *Settings → Debug log*.

```js
invoke("js_debug_log", { msg: `event: ${JSON.stringify(payload)}` });
```

Remove the probes in the same change that fixes the bug.

## Where things live

| | |
|---|---|
| Config, key, vocabulary | `dirs::config_dir()/ribbit/` — `~/Library/Application Support/ribbit/` on macOS, `%APPDATA%\ribbit\` on Windows |
| Transcript log | same folder, `logs/`, one file per day, pruned to the retention window |
