# Changelog

All notable changes to Ribbit are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Patch versions are bumped automatically by CI on every release, so version
numbers increase quickly — each entry below maps to a published
[GitHub release](https://github.com/olegperegudov/ribbit/releases).

## [Unreleased]

### Changed

- **Dictionary terms are now enforced deterministically, not by the LLM.** The
  editor model was handed the vocab table and asked to map terms itself — and it
  either ignored mandatory replacements (dictated "ТУТУ" stayed "ТУТУ" instead of
  "TO-DO") or invented "corrections" of its own (a dictated term came back as an
  unrelated "Qwen"). The model no longer sees the dictionary at all: it only
  punctuates, fixes ordinary spelling, and is explicitly forbidden from touching
  or inventing terms, names, acronyms, and Latin words. Term mapping is done
  afterwards by the exact `vocab::apply` pass, which runs over the edited text and
  guarantees the table verbatim. Add an alias to the dictionary and it is now
  applied every time, regardless of the model's mood.

### Fixed

- **A network blip no longer costs half a minute.** On a lossy link a dictation
  that normally takes ~1s took 25–33s (4 of 14 dictations on 2026-07-14, while
  the Wi-Fi was dropping 30–90% of packets; median latency the whole week before
  was 1.0s). Two amplifiers, both in Ribbit:
  - *No connect timeout anywhere.* A handshake that can't complete on a lossy
    link hangs for tens of seconds and dies anyway — one STT call sat 17s before
    the transport error, then the retry succeeded in a second. Connect is now
    capped (4s for STT, 3s for the edit) while the response timeouts stay as
    they were: the provider being slow and the link being broken are different
    animals.
  - *The edit walked the whole text stack with no ceiling.* Three providers ×
    (5s timeout + a retry that also re-paid the timeout) turned one blip into a
    26.5s edit — on a transcript that had been ready in half a second. The walk
    now has an 8s budget and stops when it's spent, timeouts are no longer
    retried (a retry only stacks another wait on a user already waiting; the
    next stack entry is the right move), and the raw transcript goes through
    `vocab::apply` as before. The audio stack deliberately keeps no budget —
    a dropped dictation is unrecoverable, so there it's right to wait.

  Worst case for the LLM edit: ~10s instead of 26.5s, and the text still lands.
  Tests: the stack walk stops once the budget is spent, and never skips its
  first rung even with a zero budget (a request must always reach a provider).

- **Closing the window no longer kills the app.** The window was destroyed on
  close (the cross, or ⌘W — macOS installs its own Close item whenever an app
  sets no menu of its own), and Ribbit has exactly one window: with it gone the
  hotkey opened nothing, the tray item opened nothing, and Tauri, seeing no
  windows left, exited the process — menu-bar icon and all. Closing now hides,
  as it always should have. A test reads `tauri.conf.json` and fails if a window
  is declared there without a hide-on-close guard, so the next window cannot
  reintroduce this. Same bug, same fix, in Quill and CopyPaster.

### Security

- **The debug log records events, not speech.** It carried a 60–80 character
  preview of every transcription and every LLM edit, appended forever and swept
  by nothing — the retention sweep only ever looked at `*.jsonl`, so "keep 7
  days" was a promise the app did not keep for this file. Lines now say how many
  characters came back, not which ones, and the file starts empty on every
  launch (an append-only log on an app that runs for weeks is also a slow disk
  leak). The transcript still lives in the history, where it expires.

- **Keys, transcripts and the vocabulary are written owner-only (0600).** `fs::write`
  obeys the umask, which is 022 on a default macOS account: on a fresh install the
  `.env` holding the API keys, the dictation log and the config all landed
  world-readable, and any process on the machine could read them. Everything now
  goes through `private.rs` (folders 0700, files 0600, the mode set on the open
  handle so a file an older build left at 0644 is narrowed rather than kept).
- **A provider endpoint must be `https://`.** It was possible to type an `http://`
  URL, after which the API key travelled the network in the clear on every
  request. Refused when typed, with a test.
- **The release workflow pins every action to a commit SHA** (was `@v4`, `@v0`,
  `@stable` — tags their owners can move) and hands the updater signing key only
  to the step that signs, instead of exporting it into the environment of every
  step in the job. These jobs hold the key that signs auto-updates: an upstream
  compromise there ships a signed malicious update to every user. Dependabot now
  watches the pins.
- **Content-Security-Policy is set** (`default-src 'self'`, no remote script, image
  or connection). The frontend never talks to the network itself — only Rust does —
  so an injected script now has nowhere to send anything. Verified against all
  three frontends under the real policy: zero violations.
- The maintainer's personal key path is gone from the docs.

### Changed

- **The README screenshots were re-shot.** All four had half a screen of empty
  background under the content, which reads as a broken picture rather than as
  air; the provider shot showed the whole settings page instead of the provider
  stack it illustrates, and the log carried an entry whose lone yellow dot ("not
  rephrased") looked like a defect to anyone who had not read the code. The
  harness now fits the viewport to the content and crops the settings page to
  the relevant band.
- **`docs/DEVELOPMENT.md` explains why the transcript is typed, not pasted** —
  a clipboard paste would clobber whatever the user had saved and litter every
  clipboard manager, including our own CopyPaster.
- **The README is a shop window now, not a manual.** It opens with three fat
  buttons that download the installer for a platform *directly* — the old ones
  linked at `/releases/latest`, which is a page, so "download" meant landing in a
  list of files and picking one. A direct link can only be made to a name that
  survives the next version bump, so CI now also uploads version-less copies of
  each installer (`Ribbit_macOS_AppleSilicon.dmg`, `Ribbit_macOS_Intel.dmg`,
  `Ribbit_Windows_Setup.exe`) next to Tauri's own assets, which the updater keeps
  reading.
- **"Releases" points at `/releases`, not `/releases/latest`.** The old link
  showed one release and its five files — read as "variants of the same thing".
  The plain `/releases` page lists every version, so anyone can roll back if we
  ship something broken.
- **Screenshots.** The log (click a line to copy), replacing a misheard word in
  place, the vocabulary, and the provider stack with its fallback. Taken with the
  headless harness (`web_eye/_ribbit_shot.mjs`) — the app renders them with its
  own code, so the README cannot show an interface that does not exist.
- **The technical half of the README moved to `docs/DEVELOPMENT.md`** — stack,
  local build, tests, release pipeline, signing, where the files live. The front
  page tells a person how to install and use the app, nothing else.

- **Updating moved to the frog in the menu bar.** A left click on the tray icon
  now opens a menu — *Check for updates*, *Show Ribbit*, the version, *Quit* —
  instead of silently toggling the window. Two reasons: a click that only flips a
  hidden window gives no sign the app is even alive, and the update had to be
  hunted for at the bottom of the settings panel, which is opened about once a
  month.

  When a release lands, the frog turns green (same emerald badge as CopyPaster's
  parrot) and the first menu item becomes *Update to vX.Y.Z* — one item, two
  jobs, so there is never a dead "check" sitting next to a live "update". The
  background poll and the manual check now go through the same `announce_update`,
  so a release found either way gives exactly the same signal.

- **The settings panel lost its update button** — and the gear its green glow.
  The row now just says where updating went. All three apps behave alike again.

## [0.7.75] - 2026-07-13

### Fixed

- **The window closes again, and stops hovering (macOS).** 0.7.73/0.7.74 made it
  worse, not better: the panel came back the instant it was dismissed, and over a
  full-screen app it still covered everything. Both came from the yield-on-blur
  logic:
  - `orderBack:` does not merely lower a window, it *orders it in*. Called from
    the resign-key handler on a panel the user had just closed with X, it put the
    window straight back on screen — vanish-and-reappear, impossible to close.
  - The full-screen check (`visibleFrame == frame`) did not fire on a real
    full-screen Space, so the panel took the `orderBack` branch there too — where
    lowering means nothing, since a `FullScreenAuxiliary` companion is always
    drawn over the full-screen window.

  Replaced both with one rule: when focus leaves the panel, it hides to the tray.
  Works the same in a window and over a full-screen app, and there is nothing left
  to resurrect it. Always-on-Top suspends it.

- **The tray icon still closes an open window.** With auto-hide-on-blur, clicking
  the tray icon *is* what takes focus away, so by the time the click is handled the
  panel already looks hidden and a naive toggle would show it right back. The tray
  now recognises a panel that auto-hid within the last 400 ms as "closed by this
  very click" and leaves it closed.

## [0.7.72] - 2026-07-13

### Fixed

- **The window stops floating over everything (macOS).** With Always-on-Top off,
  Ribbit still covered every other window and could only be dismissed with X or
  minimize. Two causes, both in `mac_window::setup_panel`:
  - The panel carried `CanJoinAllSpaces`, which pins a window to *every* Space
    like the menu bar. It resurfaced on top of each desktop the user switched to.
    Replaced with `MoveToActiveSpace` — the panel follows the user to the Space it
    is summoned on. `FullScreenAuxiliary` stays, so a tray summon over a
    full-screen app still works.
  - A non-activating `NSPanel` never activates its app, so AppKit does not reorder
    it when another window takes focus — `orderFrontRegardless` left it on top for
    good. It now yields on `windowDidResignKey`: `orderBack`, i.e. exactly what an
    ordinary window does when you click a different one. Always-on-Top suspends
    the yield.
  - On a **full-screen** Space `orderBack` is a no-op — the full-screen window *is*
    the Space and the panel is a `FullScreenAuxiliary` companion drawn over it, so
    it kept covering the app no matter where the user clicked (reported on 0.7.73).
    There the panel hides to the tray instead, which is the only available meaning
    of "get out of the way". Full-screen is detected by the screen's `visibleFrame`
    matching its `frame` — no menu bar means a full-screen Space.

  Measured first, not guessed: the live window sat at level 0 (`CGWindowLayer`),
  so the level was never the culprit, and a standalone AppKit probe reproduced
  normal ordering for the same attribute set — which is what pointed at the
  all-Spaces pinning and the missing yield.

- **Always-on-Top is remembered.** The toggle wrote nothing to disk and the
  checkbox never reflected the saved value — it silently reset on every restart.
  Now persisted (`always_on_top` in config) and restored at startup.

- **Post-processing no longer answers the dictation.** The LLM editor is supposed
  to punctuate the transcript, but sometimes obeyed it instead. Only the
  *longer-than-input* case was caught (`is_runaway_edit`); the common failure is
  the opposite. From the log: "Так, я тебя остановил. Напиши, пожалуйста, саммари
  проблемы, которые ты мне выше написал" came back as "Саммари проблемы, которые
  Вы мне выше написали." — shorter than the input, so the guard waved it through.
  Added a word-recall guard (`drops_the_dictation`): an honest edit keeps the
  dictated words (prefix match, so spelling/morphology fixes still pass); below
  60% recall the response is rejected and the caller falls back to strict
  `vocab::apply`, keeping the user's own words.

- **Window no longer rubber-bands.** A two-finger swipe elastically dragged the
  whole page inside the frame; because the window is borderless + transparent
  (rounded macOS corners), the exposed strip showed the desktop behind it. The
  document is now pinned (`html, body`: `height: 100%`, `overflow: hidden`,
  `overscroll-behavior: none`) and every scroll region (`#log-entries`,
  `.settings-content`, `#debug-content`, `#vocab-list`) keeps its overscroll to
  itself. Pinned by `src/window_chrome.test.js` — the bounce only reproduces on a
  real macOS build, so the CSS is guarded by a test rather than by memory.

## [0.7.71] - 2026-07-11

### Fixed
- **The tray icon summons the window again when it's buried behind another
  app.** `toggle_main_window` decided show-vs-hide purely on the panel's
  `isVisible`, which on macOS stays true even when the panel is fully covered
  by the window you dictated into. Before 0.7.70 the panel was pinned to the
  always-on-top floating level, so it was never covered and `isVisible` was a
  fine proxy for "on top". 0.7.70 dropped that level (to give the Always-on-Top
  toggle sole ownership), so with the toggle off the panel now sits at the
  normal level and gets covered — and a tray click took the hide branch,
  `orderOut`-ing an already-buried panel: visually a no-op, "the tray does
  nothing until I quit and relaunch". The toggle now hides only when the panel
  is the **key** window (genuinely frontmost) and otherwise raises it to front,
  the Spotlight/Raycast behaviour (`panel_is_key`).

## [0.7.70] - 2026-07-04

### Fixed
- **The window is no longer always-on-top.** `setup_panel` forced the NSPanel
  to the floating window level (`set_level(Floating)` + `is_floating_panel`),
  which kept Ribbit above every other window and silently overrode the
  Always-on-Top toggle in settings. The panel now stays at the normal level:
  it still comes to front when summoned from the tray (`show_and_make_key`),
  still appears on the current Space and over full-screen apps (collection
  behavior unchanged), but other windows can cover it afterwards. The
  Always-on-Top toggle is now the single owner of the window level.

## [0.7.69] - 2026-07-04

### Fixed
- **Microphone works again — the hardened runtime was missing the audio-input
  entitlement.** Root cause of the post-0.7.67 "mic captures pure silence, no
  prompt, no row in Privacy → Microphone" failure, confirmed on the machine:
  signing with a real identity makes the Tauri bundler apply the **hardened
  runtime** (`flags=0x10000(runtime)` — ad-hoc builds weren't hardened), and a
  hardened process may only use the microphone if it carries
  `com.apple.security.device.audio-input`. Without it macOS denies instantly
  and silently — `requestAccessForMediaType:` returned `granted=false` in 3 ms
  with no consent prompt, even right after a `tccutil reset Microphone`. Added
  `entitlements.plist` with the audio-input entitlement and wired it via
  `bundle.macOS.entitlements`, so every signed build carries it. On first
  launch the 0.7.68 launch-time request now actually shows the system mic
  prompt; grant it once and it sticks (same signing identity, so nothing else
  resets).

## [0.7.68] - 2026-07-04

### Fixed
- **Microphone can be re-granted after the one-time reset — the app now asks for
  it.** The ad-hoc → cert switch in 0.7.67 reset TCC once, which removed Ribbit
  from Privacy → Microphone. That pane has no manual "+", and cpal never prompts
  on its own — it just opens a silent stream (recording captured pure silence,
  RMS 0). The app now calls `AVCaptureDevice requestAccessForMediaType:` on
  launch, which forces the system prompt and registers Ribbit in the Microphone
  list. Grant it once and recording works; also self-heals any future reset.
- **The focus ring on the search icon / indicator dot when opening the window is
  gone for real.** The 0.7.67 attempt gated it on `:focus-visible`, but WKWebView
  treats the programmatic focus-on-show as focus-visible, so the ring stayed. The
  ring is now cleared outright on the header buttons and the log indicator dots.

## [0.7.67] - 2026-07-04

### Fixed
- **Text insertion no longer breaks on every update — stable self-signed
  signing.** Ribbit was ad-hoc signed, so each release got a fresh cdhash. A TCC
  grant's designated requirement pins that cdhash, so after every update the
  Accessibility / synthetic-keystroke grant went stale: System Settings still
  showed "allowed" while the events were filtered at kCGHIDEventTap, and dictated
  text silently stopped inserting at the cursor. CI now signs both macOS arches
  with a stable self-signed code-signing cert (secrets `APPLE_CERTIFICATE` /
  `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY`); the requirement then
  anchors to the **certificate**, not the cdhash, so the grant survives updates
  cert-to-cert. Ported from Quill.
- **`tcc_reset` now keys off the signing identity, not the cdhash.** Under stable
  signing, resetting on every cdhash change would wipe a good grant on each build.
  It now resets exactly once — on the ad-hoc → cert migration — after which
  cert-signed builds share the "Ribbit Code Signing" identity and never reset
  again. Updating to this release drops the Accessibility grant one last time;
  re-grant it once and it stays.
- **No selection ring on the LLM indicator when the window opens.** Showing the
  window from the tray let the webview restore focus to the first tabbable
  element (a log indicator dot), which painted a blue focus ring on it — it read
  as though the indicator was pre-selected. Focus rings are now gated on
  `:focus-visible`, so only keyboard navigation shows one; opening the UI leaves
  nothing highlighted.

## [0.7.66] - 2026-07-03

### Fixed
- **Restored the working tray behavior — undoes the 0.7.64 "roll back to
  0.7.58" revert.** That revert stripped out the NSPanel window and the
  accessory activation policy, which brought back the two regressions the user
  saw: the Dock icon reappeared and clicking the tray teleported them to
  desktop 1. The revert was made on the belief the NSPanel route "never landed"
  — but that read was confounded by in-app-updater lag (the good 0.7.63 build
  wasn't the one being tested). This release brings the 0.7.63 machinery back:
  non-activating NSPanel (surfaces on the current Space, over full-screen apps,
  without activating the app) + accessory policy (no Dock icon, pure menu-bar
  utility).
- **The log tooltip no longer pops open when the window is shown.** The
  `[data-hint]` status dots are tabbable, so surfacing the window let the
  webview restore focus to one and the `focusin` handler opened its tooltip —
  the "rephrased" badge looked pre-selected. The focus-driven tooltip is now
  gated on `:focus-visible`, so only real keyboard navigation triggers it, not
  programmatic focus.

## [0.7.63] - 2026-07-03

### Fixed
- **Tray icon now shows Ribbit on the desktop you're on — including full-screen
  desktops.** Root cause, finally pinned from the app's own debug log on the
  machine: clicking the tray ran the show call every time with no error, but
  the window never appeared. macOS simply **will not display an ordinary window
  over another app's full-screen Space** — and the user runs full-screen
  desktops. No amount of window/app flags on a normal window can change that
  (the five prior attempts — MoveToActiveSpace, CanJoinAllSpaces +
  FullScreenAuxiliary, accessory policy, dropping set_focus, orderFrontRegardless
  — were all fighting an unwinnable fight).
  The `main` window is now converted to a **non-activating NSPanel** (the exact
  mechanism Spotlight/Raycast use): it surfaces on the current Space over
  full-screen apps, takes keyboard for the settings/vocab fields, and does none
  of it by activating the app — so no teleport to desktop 1, and the earlier
  "extra click before typing" caveat is gone too.

### Internal
- Added `tauri-nspanel` (macOS only) and enabled the `macos-private-api` Tauri
  feature. Removed the now-obsolete raw-NSWindow Space hacks
  (`apply_spaces_behavior`, `orderFrontRegardless` show path) in favor of the
  panel's own API — one mechanism, not a pile.

## [0.7.62] - 2026-07-02

### Fixed
- **The tray icon shows the window again — on the desktop you're on, no
  teleport.** 0.7.61 removed the `set_focus()` that caused the teleport, but
  that call was doing double duty: it both *surfaced* the window and
  *activated* the app. Without it the window stopped appearing at all (clicking
  the tray icon did nothing). The window is now surfaced with AppKit's
  `orderFrontRegardless`, the primitive built for exactly this: it brings the
  window to the front of the current Space **without** activating the app —
  so it appears where you are (full-screen or not) and never drags you to
  desktop 1. `show()` couldn't do this (a background menu-bar app surfaces
  nothing) and `set_focus()` overshot (it activated and teleported);
  `orderFrontRegardless` is the middle path.
- The tray toggle on macOS now keys off visibility alone (the window is never
  the "focused" app window, since it's never force-activated).

## [0.7.61] - 2026-07-02

### Fixed
- **The real teleport fix: clicking the tray icon now shows Ribbit on the
  desktop you're actually on.** The previous three attempts added window flags
  (`CanJoinAllSpaces`, `FullScreenAuxiliary`) and switched Ribbit to a menu-bar
  accessory app — all necessary, but none touched the actual trigger. Showing
  the window called `set_focus()`, and Tauri's `set_focus` on macOS runs
  `activateIgnoringOtherApps`, which force-activates the whole app; that
  activation is what dragged you to the window's home desktop (desktop 1). On
  macOS the window is now shown with `show()` alone (orders it onto the current
  Space without activating the app), so no more teleport — from any desktop,
  full-screen or not. The three earlier ingredients stay because the show only
  lands correctly with all of them in place.
- One consequence on macOS: a window opened from the tray is placed on your
  current Space but is not force-focused (that's the activation we removed), so
  clicking into a Settings/vocab text field once before typing is expected.

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
