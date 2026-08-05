---
name: Ribbit
description: A dark terminal notebook that hangs off the menu bar and keeps everything you said.
colors:
  ink-black: "#0a0e12"
  pond-black: "#0b0e13"
  slate-panel: "#0f1318"
  card-panel: "#0f141b"
  raised-row: "#1a1f28"
  popover-surface: "#1a1f2e"
  hairline: "#2a3040"
  card-hairline: "#1a2230"
  divider: "#1a1f28"
  cursor-green: "#4ade80"
  deep-green: "#166534"
  deep-green-hover: "#15803d"
  signal-amber: "#fbbf24"
  stale-yellow: "#facc15"
  alarm-red: "#f87171"
  destructive-red: "#ef4444"
  search-magenta: "#c25cce"
  selection-fuchsia: "#e879f9"
  text-bright: "#f3f4f6"
  text-primary: "#d1d5db"
  text-secondary: "#9ca3af"
  text-dim: "#858e9c"
  text-on-hit: "#ffffff"
typography:
  title:
    fontFamily: "JetBrains Mono, Cascadia Code, Fira Code, SF Mono, Consolas, monospace"
    fontSize: "1rem"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "0.02em"
  body:
    fontFamily: "JetBrains Mono, Cascadia Code, Fira Code, SF Mono, Consolas, monospace"
    fontSize: "0.8rem"
    fontWeight: 400
    lineHeight: 1.4
  label:
    fontFamily: "JetBrains Mono, Cascadia Code, Fira Code, SF Mono, Consolas, monospace"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.35
  micro:
    fontFamily: "JetBrains Mono, Cascadia Code, Fira Code, SF Mono, Consolas, monospace"
    fontSize: "0.7rem"
    fontWeight: 600
    letterSpacing: "0.06em"
rounded:
  hair: "2px"
  chip: "3px"
  control: "4px"
  field: "5px"
  card: "6px"
  popover: "8px"
  window: "10px"
  pill: "50%"
spacing:
  hair: "0.2rem"
  tight: "0.3rem"
  snug: "0.4rem"
  row: "0.6rem"
  section: "1rem"
components:
  button-primary:
    backgroundColor: "{colors.deep-green}"
    textColor: "{colors.cursor-green}"
    rounded: "{rounded.control}"
    padding: "0.4rem 0.8rem"
    typography: "{typography.body}"
  button-primary-hover:
    backgroundColor: "{colors.deep-green-hover}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-dim}"
    rounded: "{rounded.chip}"
    padding: "2px 8px"
  button-ghost-hover:
    backgroundColor: "{colors.hairline}"
    textColor: "{colors.text-primary}"
  button-outline:
    backgroundColor: "{colors.raised-row}"
    textColor: "{colors.text-dim}"
    rounded: "{rounded.control}"
    padding: "3px 7px"
    typography: "{typography.micro}"
  button-outline-hover:
    textColor: "{colors.cursor-green}"
  input-field:
    backgroundColor: "{colors.slate-panel}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
    padding: "0.3rem 0.4rem"
    typography: "{typography.label}"
  card-provider:
    backgroundColor: "{colors.card-panel}"
    rounded: "{rounded.card}"
    padding: "0.4rem 0.5rem"
  chip-alias:
    backgroundColor: "{colors.raised-row}"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.chip}"
    padding: "1px 3px 1px 5px"
  popover-surface:
    backgroundColor: "{colors.popover-surface}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.popover}"
    padding: "0.4rem"
---

# Design System: Ribbit

## Overview

**Creative North Star: "The Terminal Notebook"**

Ribbit looks like the window a developer keeps a log in: near-black, monospace end to end, and lit by exactly one colour — the green of a live cursor. It is not a dashboard and not a document; it is a notebook that the machine writes into while you speak, and that you glance at for a second before it goes away again. Everything in the visual system serves that glance. Type is small and dense because a day of dictations should fit without scrolling. Chrome is nearly absent because there is nothing here to operate — the log is the content, and the content is the interface.

The card itself is a single flat plane in near-black (`#0f1318`) with a 10px radius, hanging off the menu-bar icon. Depth is drawn with tone, not shadow: a settings block that belongs to a toggle sinks to a darker fill (`#0b0e13`); a row you are hovering rises to `#1a1f28`. Nothing lifts off the plane except things that genuinely float above the interface — tooltips, the search popup, the vocabulary popup — and those carry a real drop shadow because they must read as detached.

Green is not decoration. It marks the live and the done: the audio meter while you speak, a line the moment it lands on your clipboard, a saved key, a term that the vocabulary will now enforce. Because it appears only there, a green pixel anywhere in this window means something happened. Amber says a fallback provider is carrying the load right now; red says an edit failed or this click destroys something. Magenta and fuchsia belong to selection and search hits — the two places where the user, not the app, is pointing at text.

**Key Characteristics:**
- One accent (`#4ade80`) reserved for live and successful states; every other colour is a status or a neutral.
- One bundled monospace, 13px root, a five-step scale from 0.7rem to 1rem — dense, but never below the 11px floor.
- Flat by default; tonal layering for grouping, shadow only for genuinely floating layers.
- Hairline dividers (`#1a1f28`) instead of boxes; borders appear only on things you can type into or press.
- Dark only. There is no light theme and none is planned.

## Colors

A near-black pond with one green light in it: five neutral steps of near-black and grey text, a single accent, and three status hues that are allowed to shout because nothing else does.

### Primary
- **Cursor Green** (`#4ade80`): the live/done signal, and the only green in the app — its washes and borders are alphas of this one hex. A second, deeper green (`#22c55e`) used to carry the same meaning in half the washes. Audio meter fill, the flash on a copied line, saved-key check, enabled toggle knob, vocabulary target words, focus borders, links in the setup screen, and the "primary" tag on the first provider in a stack.
- **Deep Green** (`#166534`, hover `#15803d`): the filled surface of the two committing buttons (save key, add vocabulary entry). The only saturated fills in the product.

### Secondary
- **Signal Amber** (`#fbbf24`): a fallback provider is carrying the work right now. Used as text on an 8%-alpha wash with a 25%-alpha border, the one banner treatment in the system.
- **Stale Yellow** (`#facc15`): the per-entry dot meaning this line was never edited by the LLM.

### Tertiary
- **Alarm Red** (`#f87171`): failure prose — the post-processing error under the provider rows.
- **Destructive Red** (`#ef4444`): the hover colour of every remove control (alias ×, row ×, provider delete). Red is never resting state; it appears under the cursor.
- **Search Magenta** (`#c25cce`) and **Selection Fuchsia** (`#e879f9`): search hits and the live vocabulary selection. The user's own pointing, not the app's.

### Neutral
- **Ink Black** (`#0a0e12`): the debug console fill, the deepest surface in the app.
- **Pond Black** (`#0b0e13`): recessed grouping panels — the LLM block and the speech stack.
- **Slate Panel** (`#0f1318`): the window itself, and the fill of every input.
- **Card Panel** (`#0f141b`): a provider card, one step up from the panel it sits in.
- **Raised Row** (`#1a1f28`): hover fill, chips, `kbd`, small controls — and, at 1px, the divider between rows.
- **Hairline** (`#2a3040`): every border on a control the user can touch.
- **Text ramp**: bright `#f3f4f6` (the wordmark and screen titles) → primary `#d1d5db` (typed text, tooltips) → secondary `#9ca3af` (log lines, setting labels) → dim `#858e9c` (timestamps, units, hints, inactive icons, remove ×). Four steps, all of them legible: the ramp used to have two fainter steps (`#6b7280` at 3.86:1 and `#4b5563` at 2.47:1) that failed WCAG AA while carrying timestamps, the `?` hints and provider labels.

### Named Rules
**The One Light Rule.** Green means live or done — nothing else. If a green pixel is on screen and nothing happened, the green is wrong. One hex, `#4ade80`: every wash, border and hover is an alpha of it, never a second green.

**The Legible Ramp Rule.** Every grey in the ramp clears 4.5:1 on every surface it lands on (`#0a0e12` through `#1a1f28`). A new "just a bit fainter" grey is not a design choice, it is a bug — `src/contrast.test.js` recomputes the ratios from the stylesheet.

**The Red-On-Hover Rule.** Destructive controls rest in faint grey and turn red only under the cursor. Nothing in the resting interface is red except an actual failure message.

## Typography

**Body Font:** JetBrains Mono, bundled with the app (`src/fonts/*.woff2`, SIL OFL 1.1), falling back through Cascadia Code, Fira Code, SF Mono, Consolas, monospace
**Label/Mono Font:** the same family; the log's provider label drops to the platform UI mono stack (`ui-monospace, SFMono-Regular, Menlo`) so model ids read cleanly.

**Character:** One monospace family carries the entire product, from the wordmark to a 0.55rem tag. The type does not perform; it lines up. Because every glyph is the same width, columns of timestamps and log lines align without a grid, and mixed Cyrillic/Latin text in one sentence keeps its rhythm.

### Hierarchy
- **Title** (700, 1rem, 1.2, +0.02em): the `ribbit` wordmark, lowercase, in the brightest text colour. The only bold 1rem in the app.
- **Body** (400, 0.8rem, 1.4): a log line, a setup step. The reading size.
- **Label** (400, 0.7rem, 1.35): setting labels, tooltips, provider fields, status detail.
- **Micro** (600–700, 0.7rem, +0.04–0.06em): day separators, group heads, the PRIMARY tag, `kbd`, unit suffixes. Letter-spaced because monospace at this size needs air.

### Named Rules
**The 11px Floor Rule.** Nothing functional is set below 0.7rem (11.2px). The scale is five steps — 0.7 / 0.72 / 0.75 / 0.8 / 1rem — and it was eleven before, spanning 8.8px to 16px, which read as one flat texture and locked out anyone who does not have young eyes.

**The Shipped Type Rule.** The face is bundled, never borrowed from the OS. Ribbit's own screenshots showed Latin monospace and Cyrillic proportional inside one sentence, because the fallback resolves per glyph and the user dictates in two alphabets.

**The Lowercase Rule.** The product's own voice is lowercase — the wordmark, setting labels, status lines. Capitals appear only in content the user or a provider supplied, and in the one 0.55rem uppercase tag.

**The 13px Ceiling Rule.** Nothing in the interface is larger than the wordmark. If a new element needs to be bigger to be noticed, the layout is wrong, not the type.

## Layout

A single fixed card, roughly 400×440, padded 1rem, laid out as one vertical flex column: header (frog, audio meter, wordmark + status line, icon buttons), then exactly one body region — log, settings, vocabulary, or debug — which owns the remaining height and scrolls inside itself. There are no breakpoints and no responsive states; the only variable is how tall the user has dragged the window.

Rhythm is built from a small set of steps: 0.2rem between chips, 0.3–0.4rem inside a control cluster, 0.6rem between the header's own parts, 1rem around the card. Rows are separated by 1px hairlines rather than gaps, which is what lets a day of dictations stay legible in 440px. Grouped settings drop their per-row dividers and take a recessed fill instead — the group is the divider.

The document itself never scrolls. Every scroll region (`#log-entries`, `.settings-content`, `#debug-content`, `#vocab-list`) contains its own overscroll: on a borderless transparent window, a bounce drags the app's skin and bares the desktop behind it.

### Named Rules
**The One Region Rule.** The card shows exactly one body region at a time. Panels replace each other; they never stack, and nothing collapses the log to a sliver to make room.

## Elevation & Depth

Flat by default. Depth is tonal: darker means grouped or recessed (`#0b0e13` blocks, `#0a0e12` console), lighter means active or hovered (`#1a1f28`). Grouping blocks add an inset shadow (`inset 0 1px 3px rgba(0,0,0,0.35)`) to sit *into* the panel rather than on top of it — the only structural shadow in the system.

Drop shadows are reserved for layers that genuinely float above the interface and would otherwise be ambiguous.

### Shadow Vocabulary
- **Floating popup** (`box-shadow: 0 4px 16px rgba(0,0,0,0.4)`): search popup, vocabulary popup — DOM elements positioned over the card.
- **Tooltip** (`box-shadow: 0 4px 12px rgba(0,0,0,0.5)`): the custom tooltip, the topmost layer in the app.

### Named Rules
**The Flat-By-Default Rule.** A surface earns a drop shadow only by floating above the interface. Grouping, hover, and selection are expressed in tone, never in lift.

## Shapes

Corners scale with the size of the thing: 2px on the audio meter's 6px trough, 3px on chips and tiny toggles, 4px on controls, 5px on the popup text fields, 6px on cards and grouping blocks, 8px on floating popups, 10px on the window itself. Circles are reserved for two roles — the step numbers in setup and the hint mark — plus the 6px status dot under a log line.

Recessed groups use an asymmetric radius (`0 6px 6px 0`) with a 2px green left edge: the block is cut off flush at its left so the edge reads as a bracket tying those rows to the toggle above them, not as a decorative stripe. Borders are 1px `#2a3040` and appear only on things you can type into or press; everything else is separated by a 1px `#1a1f28` hairline or by tone alone. The scrollbar is a 3px thumb on a transparent track.

## Components

### Buttons
- **Primary (commit):** deep green fill (`#166534`), green text, 4px radius, `0.4rem 0.8rem`. Save-key and add-vocabulary only — the two actions that write something durable. Hover deepens to `#15803d`.
- **Ghost (header icons):** no fill, muted grey, 3px radius, `2px 8px`. Hover fills `#2a3040` and lifts the glyph to `#d1d5db`. Focus rings are suppressed here: showing the window from the tray programmatically focuses the first tabbable element and WKWebView renders that as `:focus-visible`, flashing a blue ring on open.
- **Outline (settings row controls):** `#1a1f28` fill, hairline border, micro type, `3px 7px`. Hover turns text green and the border 30%-alpha green. `kbd`, the sound select, the language select, and the small setting buttons all share this shape so a settings row reads as one family.
- **Mini (provider reorder/delete):** 20×20 square, transparent fill, hairline border. Disabled drops to 30% opacity; the delete variant borders and colours red on hover.

### Chips
- **Alias / language chip:** `#1a1f28` fill, hairline border, 3px radius, 0.6–0.65rem, with a faint `×` that turns red on hover. Asymmetric padding (`1px 3px 1px 5px`) keeps the label optically centred next to its remove button.
- **PRIMARY tag:** 0.55rem uppercase green text in a 40%-alpha green outline, marking the provider currently doing the work.

### Cards / Containers
- **Provider card:** `#0f141b` on the recessed block, 1px `#1a2230` border, 6px radius, `0.4rem 0.5rem`, contents stacked at 0.3rem. Deliberately one tone above its container and one below the window, so a stack of them reads as a list, not as boxes.
- **Recessed group** (`.llm-block`, `#audio-stack`): `#0b0e13`, 2px 40%-alpha green left edge, `0 6px 6px 0` radius, inset shadow, and no row dividers inside.

### Inputs / Fields
- **Style:** `#0f1318` fill (darker than the surface they sit on), 1px `#2a3040`, 4–5px radius, monospace at 0.7–0.8rem, native outline off.
- **Focus:** border shifts to 40%-alpha green. No glow, no ring, no colour change to the text.
- **Sizes:** numeric knobs are fixed-width and centre-aligned (40–48px); text fields flex to fill their row with `min-width: 0` so a long value never widens the card.

### Navigation
There is no nav. The header's two icons swap the body region (gear → settings, magnifier → search), and the panel header carries a single × back to the log. Region switching is instant and unanimated.

### Signature Component: the log entry
A row of `timestamp · text · optional status line`, separated by a 1px hairline, fading in over 0.3s as it arrives. The whole row is a click target that copies the text; on success it washes 10%-alpha green and its text turns green for a beat, which is the only "toast" the product has. Under the text, when post-processing is on, a 6px dot (green = edited, yellow = not) and the endpoint/model that produced it. The dot is drawn via `::before` inside a 16px transparent hit area — a 6px hover target would make its tooltip feel broken.

### Signature Component: the audio meter
A 6×32px vertical trough (`#1a2030`) beside the frog, filling green from the bottom with the live input level, opacity-faded in when recording starts. It is the one animated data surface in the app, and the only element where an animated `height` is correct: the height *is* the datum.

## Do's and Don'ts

### Do:
- **Do** keep green for live and done states only (`#4ade80`), and let its rarity do the work.
- **Do** express grouping with a darker fill and a 2px green left edge, and drop the per-row dividers inside that group.
- **Do** put every scrollable region's overscroll on `contain`, and never let `html, body` scroll — the window is transparent and borderless, and a bounce bares the desktop.
- **Do** size new type inside the five-step 0.7–1rem scale, in the bundled monospace, lowercase.
- **Do** give small hit targets an invisible 16px box around the visible mark.
- **Do** state failures in words in the interface (`#f87171` prose), not only in the debug log — and never in the success colour.
- **Do** give a destructive control a second click (`armConfirm`) when what it destroys took the user time to build.
- **Do** fold what is configured once (provider url/model/key, a dictation longer than four lines) behind one click, and leave what is read often open.

### Don't:
- **Don't** introduce a second accent hue. Amber, yellow, red, and magenta are already spoken for as status and selection; a new decorative colour has nowhere to live.
- **Don't** add a light theme or a theme switch.
- **Don't** lift resting surfaces with drop shadows — only genuinely floating layers (tooltip, popups) cast one.
- **Don't** animate layout properties for effect; opacity and transform carry state changes. The audio meter's height is the sole exception, and it is data.
- **Don't** grow an element past the 1rem wordmark to make it noticeable.
- **Don't** rely on the platform focus ring on click-only controls in the header — WKWebView paints it on programmatic focus when the window opens.
