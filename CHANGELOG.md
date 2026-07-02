# Changelog

All notable changes to Ribbit are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Patch versions are bumped automatically by CI on every release, so version
numbers increase quickly — each entry below maps to a published
[GitHub release](https://github.com/olegperegudov/ribbit/releases).

## [0.7.60] - 2026-07-02

### Fixed
- **Actually fixed: opening Ribbit from the menu bar icon no longer teleports
  you to another desktop.** The 0.7.59 attempt (giving the window
  `CanJoinAllSpaces | FullScreenAuxiliary`) was not enough — the teleport
  wasn't the *window's* doing, it was the *app's*. Ribbit ran as a regular
  Dock app, and macOS drags a regular app to its window's home Space whenever
  that app is activated to show a window. Ribbit now runs as a **menu-bar
  accessory** (no Dock icon), so showing the window no longer activates a
  Space-switching app: the window appears on whatever desktop you're on,
  full-screen or not. The window flags from 0.7.59 are kept — they're what
  lets it show over a full-screen app.
- **The minimize ("_") button no longer flashes-then-vanishes on macOS.** It
  called native minimize, which sends the window to the Dock; unminimizing it
  restored it on the window's home Space, then Ribbit's own show logic ran on
  your current Space — hence the half-second flash before it disappeared. On
  macOS both the minimize and close buttons now hide to the tray (native
  minimize is meaningless for a tray app). On Windows/Linux the minimize
  button still minimizes to the taskbar as before.

### Changed
- **No more Dock icon on macOS.** Ribbit is now a pure menu-bar utility — it
  lives in the top-right menu bar and responds to the global shortcut. This is
  the mechanism that stops the desktop-teleport above; it also means Ribbit no
  longer appears in the Dock or Cmd-Tab switcher.

## [0.7.59] - 2026-07-02

### Fixed
- **Opening Ribbit from the menu bar icon while in a full-screen app no longer
  throws you to an empty desktop.** The window used to "move to the active
  Space", but macOS doesn't let a normal window join a *full-screen* Space
  (a full-screen terminal, browser, …), so instead it yanked you to the first
  free desktop and showed the window there. The window is now a proper
  menu-bar-utility window: it belongs to no desktop and is allowed to appear
  on top of full-screen apps — click the frog icon and Ribbit shows up right
  where you are. Side effect of the same mechanism: while visible, the window
  follows you across desktops (hide it with X or the tray icon as before).

## [0.7.58] - 2026-07-02

Stability audit release: two data-loss paths in dictation, four small bugs,
and a dead-code sweep. No new features.

### Fixed
- **Long dictations are no longer silently truncated by the text cleanup.**
  The cleanup model's reply was capped at a fixed 512 tokens; a dictation past
  roughly 1.5–2 minutes came back cut off and the shortened text was pasted as
  if complete (the runaway-length guard only catches replies that are too
  *long*, not too short). The cap now scales with the input length, and a
  reply that still hits it (`finish_reason=length`) is rejected in favor of
  the strict dictionary pass — you always keep your full words.
- **A provider hiccup no longer loses the current dictation.** A rate limit,
  outage or timeout on the active provider used to fail the dictation
  outright — the fallback stack only helped *subsequent* ones (and only after
  the switch threshold). Now the same request walks down the stack
  immediately, for both speech-to-text and the text cleanup, while the sticky
  switch logic still moves the starting provider after repeated failures.
  Two related gaps closed alongside:
  - Providers with no key saved are skipped instead of blocking the stack
    (previously a key-less primary made every dictation fail even when a
    fully-configured fallback sat right under it).
  - Speech-to-text retries once on a dropped connection (stale pooled TLS
    after a long idle gap) — the same courtesy the cleanup step already had.
    Timeouts are not retried; they move on to the next provider instead.
- **The window X button hides to the tray again.** It actually minimized —
  on macOS that sends Ribbit to the Dock, and restoring later teleports you
  to the window's home Space (the exact bug the tray toggle had already
  fixed). X now uses the same hide path as the tray, and the tray menu label
  ("Show/Hide Ribbit") stays in sync.
- **"Check update" no longer double-fires.** After an update was found the
  button carried two click handlers at once, so clicking it started the
  install *and* a parallel re-check with flickering label text. One handler,
  one state.
- Word replacements applied to the on-screen log now treat every Unicode
  letter as a word character (é, ü, ł, …), matching the Rust engine that
  edits the pasted text — previously only Latin and Cyrillic counted, so
  e.g. an alias "caf" could match inside "café" in the log view.
- Vocabulary entries containing quotes no longer break the list markup
  (the HTML escaper now escapes quotes for attribute contexts).

### Removed
- Ghost usage-statistics module: every dictation was recorded into a separate
  SQLite database that no screen ever read. Transcript history (the log you
  see and search) lives in the daily jsonl files and is unaffected; the
  `usage.db` file simply stops growing and can be deleted by hand if desired.
  Drops the bundled SQLite dependency from the build.
- Dead code sweep: unused `show_from_tray` / `get_fallback_state` commands, a
  bootstrap path reading a secrets file that no longer exists, and a config
  write every 30 minutes whose value nothing ever read (it could also race a
  concurrent settings change and lose it). Stale "3s timeout" comments
  corrected to the real 5s.

## [0.7.56] - 2026-06-26

### Changed
- **The green/yellow LLM indicator moved under each message, and now names the
  provider that ran.** Before, a small dot sat at the far right of the row.
  Now it sits on its own line directly below the transcript, and when the text
  cleanup actually ran (green) it's followed by the endpoint and model that did
  it — e.g. `routerai.ru | meta-llama/llama-3.3-70b-instruct`. This makes the
  new auto-fallback legible at a glance: you can see, message by message, which
  provider (and which fallback rung) was live at that moment. A yellow dot — no
  cleanup happened — stands alone with no label. Hover the label to see the full
  endpoint/model if it's truncated.

## [0.7.55] - 2026-06-25

### Added
- **Provider stacks with automatic fallback — for both speech-to-text and the
  text-cleanup step.** Settings now shows two ordered lists of providers instead
  of a single one each: **speech providers** and (under *edit transcription*)
  the edit providers. The first entry is the primary and is always tried first;
  add more with **+ add provider** and they become fallbacks, in order. When the
  primary keeps failing for a reason that means *it's unavailable right now* —
  rate-limited (HTTP 429), a server error (5xx), or a timeout — Ribbit switches
  to the next provider, uses it for a while, then returns to the primary
  automatically. This is exactly the "Groq is fast but its free limit ran out,
  fall back to RouterAI until it resets" case: set Groq as primary and RouterAI
  as the fallback, and the switch happens on its own.
  - **You tune the behavior**: switch after *N* consecutive failures (default
    2) and return to the primary after *M* minutes (default 60) — both in the
    new **auto-fallback** row.
  - **It never switches on a real mistake.** A bad key, wrong URL or retired
    model id (400/401/403/404) is shown as an error, not hidden behind a slower
    backup — because the backup would fail the same way.
  - **It's never silent.** While a fallback is active, Settings shows an amber
    line — which provider you're on and when the primary will be retried.
  - **Each provider has its own URL, model and key**, so you can point an entry
    at any OpenAI-compatible endpoint. Your existing setup is migrated into the
    new stacks automatically on first launch — nothing to re-enter.

## [0.7.54] - 2026-06-25

### Fixed
- **The text cleanup step sometimes answered your dictation instead of typing
  it.** When the dictated phrase looked like a question or a command (e.g.
  asking for "an example of what a space dictionary looks like"), the cleanup
  model would occasionally drop its editor role and reply — pasting a wall of
  invented text where the transcript should have been. The prompt already tells
  the model it's a filter and must never answer; this adds a last-resort safety
  net behind it: if the "edited" text comes back implausibly longer than what
  you said (the signature of an answer, not an edit), Ribbit discards it and
  falls back to the strict dictionary pass, so you get your own words — never
  someone else's answer — in the paste.

## [0.7.53] - 2026-06-24

### Fixed
- **Microphone broke on macOS after updating to 0.7.52.** Pressing the record
  hotkey threw up repeated "Ribbit wants to access the microphone" prompts and
  recording never worked, even after allowing every one. 0.7.52 had switched the
  Mac build to a single **universal** (arm64 + Intel) binary to fix the updater
  manifest; that turned out to break microphone access, because a universal,
  ad-hoc-signed bundle doesn't anchor a macOS permission grant to a stable code
  identity — so the system kept re-asking and the grant never stuck. Reverted to
  the proven **per-architecture** Mac builds (a native arm64 binary for Apple
  Silicon, native Intel for older Macs): the permission grant sticks again, with
  just the usual single re-prompt the first time you record after an update. The
  updater manifest stays correct via the build ordering already in place, and a
  CI guard now also refuses to publish a universal macOS bundle, so this exact
  regression can't ship again.

## [0.7.52] - 2026-06-24

### Fixed
- **In-app update failed on Apple-Silicon Macs.** The release manifest
  (`latest.json`) listed only the Intel macOS build — the `darwin-aarch64` entry
  was missing, so Apple-Silicon Macs couldn't find a matching download and the
  updater reported "update failed". Root cause was in CI: the two Mac
  architectures were built as separate steps that each regenerated and
  re-uploaded the manifest, and the Intel upload raced ahead of the arm64 one and
  dropped its entry. macOS now ships as a single **universal** (arm64 + Intel)
  binary whose manifest always carries both Mac keys; the macOS job runs after
  the Windows one so the two no longer race on the same file; and a new CI guard
  fails the build if any platform key is ever missing from the published
  manifest, so a broken updater can never ship silently again.

## [0.7.51] - 2026-06-24

### Changed
- **Settings keys renamed by role.** Two rows both read "groq key", which was
  confusing — they serve different purposes. Renamed and made provider-agnostic:
  **speech key** (turns your voice into text — a Groq or OpenAI key) and **edit
  key** (the key for whichever LLM provider cleans up the text). When the edit
  provider is Groq, the edit key and speech key are the same underlying token, so
  saving one now marks the other saved automatically.
- **LLM settings grouped under the toggle.** The provider / model / edit-key rows
  now sit in one recessed panel with a green left edge directly under the "edit
  transcription" toggle, so it reads as belonging to it; the panel collapses
  entirely when the toggle is off. The speech key moved up next to languages, and
  debug log moved down to the footer next to the version.

## [0.7.50] - 2026-06-19

### Fixed
- **Dictations could vanish.** The previous release cut the speech-to-text
  request timeout to 20s; when Groq's free tier was busy and a transcription ran
  longer, the request was aborted and — because a failed transcription has no
  text to fall back to — the whole dictation was lost (looked like the hotkey
  "didn't register"). Restored the generous 120s timeout: a slow call now waits
  instead of dropping your words.

### Changed
- **Audio is downsampled to 16 kHz before upload.** The Mac mic can't capture at
  16 kHz and falls back to 48 kHz, which made every upload ~3× larger than the
  model needs (a 40s dictation was a ~3.9 MB file). Sending 16 kHz — the model's
  native rate, so no accuracy loss — cuts the upload threefold and noticeably
  speeds up transcription, most visibly on longer dictations.

## [0.7.49] - 2026-06-19

### Changed
- **Text-edit step now defaults to Groq** (`llama-3.3-70b-versatile`) instead of
  RouterAI `gemma-4-26b`. RouterAI was the dominant source of paste lag: from
  ~810 dictations in the log, ~28% timed out after ~10s and fell back to
  unedited text, and another ~10% needed a retry (~8s). A 26B model on a
  congested router is overkill for "fix the punctuation of one short phrase".
  Groq reuses the speech-to-text key (no new setup), runs on LPUs, and returns
  in ~0.5–1s — removing both the 10s timeouts and the silent fallbacks. Existing
  users switch with one click in Settings → edit transcription → provider; the
  model id stays editable there.
- **Speech-to-text request timeout cut from 120s to 20s.** Groq's free tier
  occasionally queues a transcription for tens of seconds (observed up to 89s in
  the log — ~2% of calls run over 5s). A stuck call now fails fast so you can
  re-dictate in a couple of seconds instead of watching a frozen "Ribbiting…".

## [0.7.48] - 2026-06-07

### Fixed
- The "edit transcription" LLM stopped working for anyone on the **openrouter**
  provider: its hardcoded model `google/gemini-2.0-flash-001` was retired by
  OpenRouter and every request returned `http 404`, so the feature silently
  fell back to plain vocab — looking like "the LLM does nothing". Updated the
  built-in openrouter default to `google/gemini-2.5-flash`.

### Added
- **Editable model per provider** (Settings → edit transcription → *model*).
  Leave it empty for the built-in default, or paste a specific model id. This is
  the real fix for the rot above: when a provider drops a model id, you swap it
  here in five seconds instead of waiting for an app release. Persists per
  provider; the placeholder shows the current default.
- **Last-error note** under the LLM rows (`⚠ last LLM edit failed: …`), cleared
  on the next success. The post-process fallback used to be fully silent —
  diagnosable only by digging through the debug log. Now a provider quietly
  retiring a model, an expired key, or a network failure is visible in Settings.

## [0.7.47] - 2026-06-01

### Fixed
- The LLM "edit transcription" feature no longer answers the dictation as
  if it were a chat message. Dictating a command-like phrase such as
  "подожди, давай начнём с аудита исправлений" used to come back as a
  reply ("Хорошо, давай начнём с аудита"): the model treated the
  transcript as something addressed to it. The post-processing prompt was
  too thin — it said "don't add comments" but never said the input is not
  a conversation. Rewrote it to frame the input as raw dictated text that
  must be reformatted, never replied to / obeyed / continued, with an
  explicit one-shot example (command-like phrase → same phrase, just
  punctuated). The vocab section is now stated as mandatory and
  authoritative over the anglicism-to-Cyrillic rule, so terms and app
  names (Dev, Prod, Alrosa, …) keep their dictionary spelling. Snapshot
  test updated to pin the new framing and the anti-reply example.

## [0.7.46] - 2026-05-21

### Added
- Quick search now highlights the matched letters inside each log
  entry — searching "алр" paints the "Алр" of "Алросы" pink-purple, so
  the hit is visible at a glance instead of having to eyeball the row.

## [0.7.45] - 2026-05-21

### Changed
- Release builds now retry once automatically when uploading the
  finished app fails on a transient network glitch with GitHub. Several
  releases in a row had been marked failed purely because of a momentary
  upload timeout, even though the build itself was fine. Internal CI
  reliability only — no change to the app.

## [0.7.44] - 2026-05-21

### Added
- Every dictation now records a per-stage timing breakdown into the
  daily log — speech-to-text, the LLM editor, and text insertion are
  timed separately, alongside the audio length, character count, idle
  gap since the previous dictation, and the models used. This makes it
  possible to tell, after the fact, which stage is responsible when a
  dictation feels slow. No effect on the dictation experience itself.

## [0.7.43] - 2026-05-21

### Fixed
- The hover tooltip on the LLM status dot now reliably appears — the
  hover target was only 6px wide and practically unhittable; it is now
  an 18px transparent area around the same visible dot.

### Changed
- That tooltip now reads simply "rephrased" / "not rephrased".

## [0.7.42] - 2026-05-21

### Changed
- The update auto-check now repeats every 30 minutes while Ribbit is
  running, not just once at launch — the gear lights up on its own when
  a release ships during the day, no manual "check update" needed.

## [0.7.41] - 2026-05-21

### Added
- Quick search: a magnifier button in the header opens a small popup;
  typing filters the log live to transcripts containing a word that
  starts with the query (case-insensitive, Cyrillic-aware).
- History retention setting — choose how many days of transcripts to
  keep on disk (rolling window, today plus the previous N-1 days).
  Default is 7 days, up from the previous fixed 24-hour window.

### Changed
- Day separators in the log are now a faded rule with the weekday and
  date centered in it (e.g. "tu, may 5th").

## [0.7.40] - 2026-05-21

### Fixed
- The app title in the header is no longer selectable, so a drag that
  starts elsewhere can't leave it stuck in a highlighted state.

## [0.7.38] - 2026-05-19

### Fixed
- Moved the per-entry LLM status dot left so it no longer overlaps the
  history scrollbar.

## [0.7.37] - 2026-05-19

### Changed
- LLM post-processing now retries once on transient network errors and
  reports the failure kind, instead of silently leaving the transcript
  unedited.

## [0.7.36] - 2026-05-19

### Changed
- The LLM status dot uses the app's themed tooltip on hover.

## [0.7.35] - 2026-05-19

### Changed
- Settings polish: lowercase labels, themed LLM provider dropdown, and
  regrouped rows for a cleaner layout.

## [0.7.34] - 2026-05-19

### Added
- Pluggable LLM providers for transcript post-processing — pick the
  service that edits your dictation.
- Your custom vocabulary is now passed into the LLM prompt, so edits
  respect your preferred spellings.
- Per-entry status dot showing whether a transcript was edited by the LLM.

## [0.7.33] - 2026-05-19

### Added
- Visible confirmation in settings when the RouterAI key is saved.

## [0.7.32] - 2026-05-19

### Fixed
- Rapid multi-clicks no longer start a stray text selection across
  history entries.

## [0.7.31] - 2026-05-18

### Changed
- Transcripts are now typed directly at the cursor instead of going
  through the clipboard.

## [0.7.30] - 2026-05-18

### Fixed
- macOS: clicking the tray icon opens the window on the active Space
  instead of teleporting it to another desktop.

## [0.7.29] - 2026-05-18

### Changed
- Removed automatic copying of transcripts to the clipboard.

### Fixed
- Vocabulary now matches aliases that contain dots.

## [0.7.28] - 2026-05-18

### Fixed
- Gave the scrollbar its own gutter and adjusted window height so content
  always fits.

## [0.7.27] - 2026-05-18

### Fixed
- Cleaner window corners via a frame view; smaller default window height.

## [0.7.26] - 2026-05-18

### Added
- Native rounded window corners on macOS.

### Changed
- The settings panel is now scrollable.

## [0.7.25] - 2026-05-18

### Fixed
- Ribbit no longer restores your previous clipboard contents after pasting.

## [0.7.24] - 2026-05-17

### Added
- User-facing documentation for the optional LLM transcript-editing feature.

## [0.7.23] - 2026-05-15

### Fixed
- CI: use an absolute Cargo path on macOS so unit tests run reliably.

## [0.7.22] - 2026-05-15

### Fixed
- Post-processing no longer strips text too aggressively when removing
  speaker labels.
- macOS builds pin the stable Rust toolchain.

## [0.7.21] - 2026-05-15

### Added
- Optional LLM editing of transcripts via RouterAI — clean up filler words
  and punctuation automatically.

## [0.7.20] - 2026-05-15

### Added
- Test coverage for the vocabulary feature (Vitest + Cargo tests) as a
  regression guardrail.

## [0.7.19] - 2026-05-14

### Added
- In-app debug log for diagnosing issues without a development build.

## [0.7.18] - 2026-05-08

### Security
- Updated vulnerable transitive dependencies in the Rust backend.

## [0.7.17] - 2026-05-08

### Changed
- Custom themed tooltips for the settings hint icons.

## [0.7.16] - 2026-05-06

### Fixed
- macOS: accessibility permission is reset automatically after an update,
  so pasting keeps working instead of silently failing.
- Corrected the sort order of the transcription history.

## [0.7.15] - 2026-05-05

### Fixed
- macOS: text in history entries can be selected again.

## [0.7.14] - 2026-05-05

### Added
- In-app updater enabled for macOS (signed app bundle).

## [0.7.13] - 2026-05-05

### Fixed
- macOS: the Cmd-based hotkey is captured correctly.
- Fixed a crash that could happen right after transcription.

## [0.7.12] - 2026-04-30

### Added
- Live audio-level meter next to the frog while recording.

### Changed
- Error messages stay visible for 5 seconds instead of clearing the moment
  recording stops.

## [0.7.11] - 2026-03-23

### Fixed
- The vocabulary popup keeps the selected word highlighted while it's open.

## [0.7.10] - 2026-03-23

### Added
- Vocabulary: instant delete and editable keys, with detection of merges
  when a key collides with an existing one.

### Fixed
- Copying a log entry now copies the vocabulary-replaced text rather than
  the raw transcript.

## [0.7.9] - 2026-03-17

### Fixed
- Resolved a sound-playback crash caused by an audio-buffer lifetime issue.

## [0.7.8] - 2026-03-17

### Fixed
- More reliable sound playback: a fresh audio stream with a cached fallback.

## [0.7.7] - 2026-03-16

### Fixed
- Sounds play even when the Ribbit window is not focused.

## [0.7.6] - 2026-03-15

### Added
- Vocabulary panel header with usage hints.

## [0.7.5] - 2026-03-15

### Fixed
- The window auto-resizes to fit the settings panel, removing the scrollbar.

## [0.7.4] - 2026-03-15

### Changed
- Settings are always visible.
- Always-on-top is now off by default.

## [0.7.3] - 2026-03-14

### Fixed
- New entries use 24-hour time.
- The selected language is passed through correctly when multiple
  languages are configured.

## [0.7.2] - 2026-03-14

### Fixed
- Restored pasting on Windows; Unicode-based insertion is now used only on
  macOS.

## [0.7.1] - 2026-03-14

### Changed
- History entries display time in 24-hour format.

## [0.7.0] - 2026-03-14

### Added
- **Vocabulary** — define custom word replacements so Ribbit consistently
  spells names and terms the way you want.
- **macOS support** — Ribbit now runs on both Windows and macOS.
- Click-to-copy for history entries.

### Changed
- UI improvements throughout.
- Clearer messaging that a free Groq account is required.

## [0.6.4] - 2026-03-13

### Changed
- History keeps the last 24 hours of transcriptions instead of only today.

## [0.6.3] - 2026-03-12

### Added
- Tooltip hint for the language selector.

## [0.6.2] - 2026-03-12

### Changed
- Old transcription logs are cleaned up on startup.

## [0.6.1] - 2026-03-12

### Changed
- Ribbit checks for updates on every launch.

## [0.6.0] - 2026-03-12

### Added
- Language selector in settings.
- Raw audio playback — listen back to what you recorded.

### Removed
- Sound playback modifiers (speed/amplify).

## [0.5.5] - 2026-03-10

### Changed
- Replaced the ping sound with real wood-knock sounds.

## [0.5.4] - 2026-03-10

### Fixed
- Fixed a doubled quack sound; deepened the ping sound.

## [0.5.3] - 2026-03-10

### Fixed
- Fixed a doubled sound on key release and the ping sometimes not playing.

## [0.5.2] - 2026-03-10

### Added
- Ribbit auto-follows the system default audio device.

### Changed
- New frog sound.

## [0.5.1] - 2026-03-10

### Changed
- Bassier ping sound.
- README with logo and a privacy section.

## [0.5.0] - 2026-03-10

### Added
- Clickable version label that opens the changelog on GitHub.

### Fixed
- Windows updater build: resolved signing-key corruption and migrated off
  deprecated updater APIs.

## [0.4.0] - 2026-03-10

### Added
- Automatic update check on startup, with visual indicators when an update
  is available.

## [0.3.0] - 2026-03-10

### Added
- Sound pack selector — choose between the frog and ping sound packs.

## [0.2.0] - 2026-03-10

Initial public release.

### Added
- Hold-to-talk voice dictation: hold a hotkey, speak, release, and the
  transcribed text is pasted into the active window.
- Speech-to-text powered by Groq Whisper (`whisper-large-v3-turbo`).
- Customizable global hotkey.
- System tray with show/hide and quit; frameless window with custom
  controls and a pixel-frog icon.
- Native sounds for recording start/stop (Rust `rodio`).
- Always-on-top toggle.
- Transcription history limited to the current day.
- First-run setup screen with a step-by-step guide to getting a free
  Groq API key.
- Automatic updates via the Tauri updater; NSIS installer on Windows that
  installs over the previous version.

### Fixed
- Russian keyboard-layout paste.
- Audio garbling from stereo input (now downmixed to mono).
- Startup crash on Windows caused by the Win key being reserved for global
  shortcuts.
- Transcription hang on multi-byte UTF-8 (Russian) text.

[0.7.38]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.38
[0.7.37]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.37
[0.7.36]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.36
[0.7.35]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.35
[0.7.34]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.34
[0.7.33]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.33
[0.7.32]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.32
[0.7.31]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.31
[0.7.30]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.30
[0.7.29]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.29
[0.7.28]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.28
[0.7.27]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.27
[0.7.26]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.26
[0.7.25]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.25
[0.7.24]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.24
[0.7.23]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.23
[0.7.22]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.22
[0.7.21]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.21
[0.7.20]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.20
[0.7.19]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.19
[0.7.18]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.18
[0.7.17]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.17
[0.7.16]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.16
[0.7.15]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.15
[0.7.14]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.14
[0.7.13]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.13
[0.7.12]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.12
[0.7.11]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.11
[0.7.10]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.10
[0.7.9]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.9
[0.7.8]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.8
[0.7.7]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.7
[0.7.6]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.6
[0.7.5]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.5
[0.7.4]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.4
[0.7.3]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.3
[0.7.2]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.2
[0.7.1]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.1
[0.7.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.7.0
[0.6.4]: https://github.com/olegperegudov/ribbit/releases/tag/v0.6.4
[0.6.3]: https://github.com/olegperegudov/ribbit/releases/tag/v0.6.3
[0.6.2]: https://github.com/olegperegudov/ribbit/releases/tag/v0.6.2
[0.6.1]: https://github.com/olegperegudov/ribbit/releases/tag/v0.6.1
[0.6.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.6.0
[0.5.5]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.5
[0.5.4]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.4
[0.5.3]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.3
[0.5.2]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.2
[0.5.1]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.1
[0.5.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.5.0
[0.4.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.4.0
[0.3.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.3.0
[0.2.0]: https://github.com/olegperegudov/ribbit/releases/tag/v0.2.0
