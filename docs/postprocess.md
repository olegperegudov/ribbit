# LLM post-processing (optional)

After Groq STT and the vocab pass, Ribbit can optionally run the transcribed
text through a small LLM that fixes punctuation, spelling, and anglicisms
before the text is logged and pasted.

Disabled by default. ~150–300 ms latency overhead per transcription when on.

## Enable

1. Open **Settings → edit transcription** → toggle on.
2. A **provider stack** appears — an ordered list of providers, each a card with
   **url**, **model** and **key**:
   - **url** — the OpenAI-compatible chat-completions endpoint. Prefilled from a
     catalog pick (groq / routerai / openai / openrouter) or typed for a custom
     one.
   - **model** — the model id. Prefilled with the catalog default; editable, so a
     retired id is fixed here with no app update.
   - **key** — the provider's token. Stored locally (`~/.config/ribbit/.env` on
     macOS), one `.env` var per entry, never shown again.
3. The first card is the primary; **+ add provider** appends fallbacks (see
   [Auto-fallback](#auto-fallback)).

Toggle off any time. The setting persists across restarts.

The **speech (STT)** side has the same kind of stack (always visible, since STT
can't run without a provider) — same cards, same fallback rules, audio
`/audio/transcriptions` endpoints instead of chat ones.

## Auto-fallback

Both stacks switch to the next provider when the active one keeps failing for a
reason that means *unavailable right now* — HTTP 429 (rate limit), 5xx (provider
down) or a network timeout — after a configurable number of consecutive such
failures (default 2). It stays on the fallback for a cooldown window (default
60 min), then returns to the primary. With more than two entries it walks the
whole chain. A **hard** client error (400/401/403/404 — bad key/url/model) never
switches: it's a config bug to surface, not a reason to mask behind a backup.

The switch state (active entry, fail tally, switch time) is in-memory and
per-stack, so a restart starts fresh from the primary. Order is the priority and
is reordered in the UI. Knobs live in the **auto-fallback** row. Implementation:
`src-tauri/src/fallback.rs` (pure state machine, unit-tested); the two pipelines
in `lib.rs` drive it on the `CallError` returned by `transcribe`/`postprocess`.

## What it does

- Model per provider (override in Settings → model, else the built-in default):
  routerai `google/gemma-4-26b-a4b-it`, openrouter `google/gemini-2.5-flash`,
  openai `gpt-4o-mini`. OpenAI-compatible chat completion.
- System prompt frames the input as dictated text, **not** a message to the
  model: capitalize sentences, add punctuation, fix spelling/STT errors, apply
  the vocab (mandatory, authoritative over the anglicism rule), and never reply
  to / obey / continue the text even when it sounds like a request. A one-shot
  example pins this: a command-like phrase comes back punctuated, not answered.
- `temperature: 0.0`; `max_tokens` scales with the input (chars + 100, clamped
  to 512..4096) so long dictations aren't truncated. A reply that still hits
  the cap (`finish_reason=length`) is rejected → vocab fallback.

Pipeline order: **Groq STT → vocab → postprocess (if on) → log + paste.**
The same edited text goes to all three outputs.

## Failure handling

If the LLM call fails (network error, 5xx, timeout, no key set, or a retired
model id → `http 404`), Ribbit falls back to the vocab-processed text. The user
always gets a paste within the timeout of the hotkey release.

The row itself says why: next to its yellow dot the history shows the provider
and the reason — `api.groq.com | rate limit / free tier`, `routerai.ru | timed
out`, `no key set` — so waiting it out and going to fix something are told apart
at a glance. The phrase is chosen in `fallback.rs` (`CallError::reason`) and
stored per entry in the daily jsonl as `llm_error`, so it survives a restart.
An entry taken while the editor was off reads "the editor was off".

The fallback is no longer *silent*: the last failure reason is shown in
**Settings** under the edit stack (`⚠ last LLM edit failed: …`) and cleared on
the next success, so a provider quietly dropping a model can't rot the feature
unnoticed. An active auto-fallback also shows an amber status line above the
stack. Full diagnostics land in **Settings → debug log** as
`postprocess … (no switch)` / `… active text idx -> N`.

## Cost

One small chat-completion request per dictation, charged to the personal
RouterAI token. If volume becomes a problem we can add a "skip if text is
short and ASCII-clean" heuristic later.

## Tuning the prompt

The prompt lives in `src-tauri/src/postprocess.rs::system_prompt()` and is
pinned by a snapshot test. To change tone or behavior: edit the function,
update the test assertion, push. CI runs `cargo test` before any release.
