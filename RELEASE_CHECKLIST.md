# Release smoke checklist

Manual checks to run after each release on **both Windows and macOS** before
declaring a version stable. Things that cannot be unit-tested (OS permissions,
native window chrome, hardware) live here.

5 minutes, both OS. If anything fails: file an issue, fix, push, repeat.

## Window chrome

- [ ] Window has rounded corners (10px radius), no jagged square edges
- [ ] Window is draggable by header area
- [ ] Minimize button (`_`) sends app to taskbar/dock
- [ ] Close button (`x`) hides to tray, app keeps running
- [ ] Tray icon click restores window to last position

## Microphone

- [ ] First launch on macOS: system prompts for mic access (TCC)
- [ ] After granting: mic indicator works (audio meter rises while speaking)
- [ ] Recording starts on hotkey press, ends on release
- [ ] Status text changes: idle → starting mic → listening → ribbiting → idle

## Transcription + paste

- [ ] Short phrase transcribed and pasted into the focused app
- [ ] Cyrillic phrase transcribed correctly
- [ ] Numbers / mixed RU+EN handled
- [ ] No double paste, no truncation
- [ ] Clipboard manager (Maccy / Paste / Alfred) gains exactly one entry per
      dictation — the transcript itself. No duplicates of the previous copy.

## Vocab replacement

- [ ] Open vocab panel (gear → abc), add a simple alias manually (`dev ← def`)
- [ ] Speak a phrase containing the alias — output shows the target
- [ ] Select a word in a log entry → popup appears → type target → Enter →
      the word in the log is replaced AND mapping persists to vocab.json
- [ ] Cyrillic alias works the same way
- [ ] Suggestion (existing similar target) appears when selecting a close word

## Hotkey + shortcuts

- [ ] Default `ctrl+alt+space` triggers recording
- [ ] Custom shortcut can be captured (Settings → shortcut → press → Enter)
- [ ] Esc cancels shortcut capture without saving

## Settings panel

- [ ] Gear opens settings; clicking again returns to log
- [ ] Always-on-top toggle works
- [ ] Language chips: add/remove work
- [ ] Sound pack switch plays test sound
- [ ] Version label opens the GitHub release page

## Auto-update

- [ ] Tray menu: "Check for updates" reports current state correctly
- [ ] Tray menu: the "Ribbit vX.Y.Z" item is clickable and opens the release
      list in the browser
- [ ] The release page for this version lists what changed, in bullets
- [ ] When an update is available: tray frog goes green, the item reads "Update to vX.Y.Z" and installs on click
- [ ] After install: app restarts on new version, settings preserved

## LLM post-processing (optional feature)

- [ ] Toggle off (default) → transcription behaves as before, no extra latency
- [ ] Toggle on + valid RouterAI key → dictate "привет это девопс инжинер" →
      log + paste show punctuation/casing fixed (e.g. "Привет, это DevOps-инженер")
- [ ] Toggle on but no key set → `debug.log` shows
      `postprocess enabled but no ROUTERAI_API_KEY — skipping`, paste falls back
      to vocab'd text without freezing
- [ ] Toggle on + network disabled → paste happens within ≤3s of stop with
      vocab'd text; `debug.log` shows `postprocess fallback: network error: ...`

---

## Why this file exists

Tests in `src/vocab.test.js` (vitest) and `src-tauri/src/vocab.rs` (cargo test)
cover the *pure logic* sides — vocab replacement algorithm, levenshtein,
case-preservation. They run in CI on every push.

Everything else above (OS permissions, native window chrome, mic, tray,
clipboard, hotkey hardware) is either expensive to automate or genuinely
needs eyes on the running app. This checklist is the cheap, reliable
guardrail for the parts machines can't easily check.
