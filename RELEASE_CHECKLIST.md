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

- [ ] "check update" button reports current state correctly
- [ ] When an update is available: button glows, click downloads + installs
- [ ] After install: app restarts on new version, settings preserved

---

## Why this file exists

Tests in `src/vocab.test.js` (vitest) and `src-tauri/src/vocab.rs` (cargo test)
cover the *pure logic* sides — vocab replacement algorithm, levenshtein,
case-preservation. They run in CI on every push.

Everything else above (OS permissions, native window chrome, mic, tray,
clipboard, hotkey hardware) is either expensive to automate or genuinely
needs eyes on the running app. This checklist is the cheap, reliable
guardrail for the parts machines can't easily check.
