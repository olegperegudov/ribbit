# Changelog

All notable changes to Ribbit are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Patch versions are bumped automatically by CI on every release, so version
numbers increase quickly — each entry below maps to a published
[GitHub release](https://github.com/olegperegudov/ribbit/releases).

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
