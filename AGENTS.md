# Agent notes

Ribbit is push-to-talk dictation: hold a hotkey, speak, release, and the
transcript is typed into whatever window you were in. Tauri 2 — Rust records,
calls the speech provider, post-processes and inserts; the webview shows the
log and the settings. No frontend framework, no bundler — `src/` is served
as-is.

Start here: **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)**. Two more docs worth
knowing: [docs/postprocess.md](docs/postprocess.md) (what happens to a
transcript after the model returns it) and
[docs/update-channels.md](docs/update-channels.md) (stable/beta, the release
gates and how to roll one back).

Quick facts:

- Run it: `npm install && npm run tauri dev`
- Tests: `npm test` (vitest) and `cargo test --lib` in `src-tauri/`. Provider
  responses are replayed from recorded fixtures; the live checks against a real
  provider are the canaries in `.github/workflows/canary.yml` and at release
  time.
- Keys live in `0600` files written through `private.rs`; audio goes to the
  provider the user configured and nowhere else — no analytics, no telemetry.
- Versions are bumped by CI on push to `main` — do not edit the version in
  `src-tauri/tauri.conf.json` or `Cargo.toml` by hand.
- User-visible changes go in `CHANGELOG.md` under `## Unreleased`, one plain
  bullet per change; CI cuts that section into the release notes.
