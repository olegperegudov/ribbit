# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

One person at a desk who types for a living — code, chat, tickets, notes — and would rather say a sentence than type it. Public download, but designed around its author's own day: he is the first user and the deciding taste. Bilingual (Russian and English, often inside one sentence), on macOS; Windows works and is less exercised.

The window is not where the work happens. The work happens in whatever app the user was already typing in: hold the hotkey, speak, let go, the text appears there. Ribbit's own window is opened for a second — did that come out right, let me grab that line again — and dismissed. Settings are visited rarely and deliberately.

## Product Purpose

Turn speech into text in the window you are already in, with no round trip through another app. Success is that the user stops noticing Ribbit: the text is simply there, correct, including the jargon and the names that every transcription service mangles.

## Positioning

A menu-bar dictation tool that keeps the transcript on your own disk and lets you fix the machine's mistakes permanently. Two mechanisms a neighbouring product would have to copy wholesale:

- **A vocabulary you teach in place.** Select the mangled word in the log, type what it should have been; from then on it is replaced silently, every time.
- **A provider stack with fallback.** Any number of speech and text-editing providers in priority order; after enough consecutive failures Ribbit drops to the next and climbs back after a cooldown. The thresholds belong to the user.

## Operating Context

- Lives in the menu bar / tray. Left click on the icon drops the window out of it; clicking anywhere else puts it away. Right click gives update, version, quit.
- Summoned by a global hotkey (⌃⌥Space on macOS, Ctrl+Alt+Space on Windows) held down while speaking — the window does not have to be open, or even visible, for a dictation.
- The window itself is a small card anchored to the icon: a running log of dictations grouped by day, a gear for settings, a magnifier for search across kept days.
- Text is delivered by synthetic keystrokes into the frontmost app, which makes macOS Accessibility and secure-input state part of the product's reality.
- Updates arrive in place: CI builds a release, the app offers it from the tray menu.

## Capabilities and Constraints

- Dictation log kept on local disk for a user-set number of days; a line is copied by clicking it. No cloud, no telemetry.
- Vocabulary: aliases, multi-word phrases, mandatory replacements, searchable list under the gear.
- Two-language mode: English terms inside a Russian sentence come back as English, not transliterated.
- Optional LLM post-processing of the raw transcript, with its own provider stack and fallback knobs.
- Free to run: works on a free Groq key, asked for on first launch. Not notarized by Apple (a paid programme), so first launch needs a manual "Open Anyway".
- Desktop app in a Tauri webview: one window, no browser chrome, no responsive breakpoints — a card of roughly 400×440 that the user may resize but does not move (it re-anchors to the tray icon on every open). Window size is not sacred.
- macOS is the exercised platform; the Windows build installs and lags in polish.

## Brand Commitments

- The pixel frog and the lowercase wordmark `ribbit` are the face of the app; both are binding.
- Dark only. There is no light theme and none is wanted.
- Voice in the product's own text: plain, dry, lowercase where the app already goes lowercase; no exclamation marks, no assistant chirp.
- Monospace type and the exact window dimensions are current facts, not commitments — a redesign may argue with them.

## Evidence on Hand

- Real UI screenshots in `docs/screenshots/` (log, vocabulary in place, vocabulary list, providers).
- The app icon and logo: `src/frog.png`, `src-tauri/icons/`.
- Public README with the actual feature copy and install instructions.
- No user research, testimonials, install counts, or benchmarks exist. Do not invent them.

## Product Principles

1. **The window is a glance, not a workspace.** Anything that costs the user a second on open is worse than a feature that is one click deeper.
2. **The transcript is the product's memory.** The log — readable, searchable, copyable, honest about failures — outranks every other surface.
3. **The machine's mistakes are fixable by the user, permanently.** Teaching a word once must beat correcting it every time.
4. **Nothing leaves the machine that the user did not send.** Local log, no telemetry, keys under the user's control.
5. **Silence when it works.** Status and errors surface only when they change what the user would do next.

## Accessibility & Inclusion

No standard was established as a requirement. Two product-specific realities: text is delivered through macOS Accessibility permissions (a revoked grant must be legible to a non-technical user), and the interface carries mixed Cyrillic/Latin text in every log line.
