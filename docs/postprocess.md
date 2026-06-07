# LLM post-processing (optional)

After Groq STT and the vocab pass, Ribbit can optionally run the transcribed
text through a small LLM that fixes punctuation, spelling, and anglicisms
before the text is logged and pasted.

Disabled by default. ~150–300 ms latency overhead per transcription when on.

## Enable

1. Open **Settings → edit transcription** → toggle on.
2. Three rows appear: **provider**, **model**, and the provider **key**.
   - **provider** — routerai (Russia), openai, or openrouter.
   - **model** — leave empty to use the provider's built-in default, or paste a
     specific model id. This is the escape hatch: when a provider retires a
     model id (and the LLM edit starts failing), paste a current id here — no
     app update needed. The placeholder shows the current default.
   - **key** — paste the provider's token. Stored locally
     (`~/.config/ribbit/.env` on macOS) and never shown again.
3. On the maintainer's Mac a personal RouterAI key is auto-loaded once from
   `~/membeme/system/secrets/routerai.key` on first launch — no manual step.

Toggle off any time. The setting persists across restarts.

## What it does

- Model per provider (override in Settings → model, else the built-in default):
  routerai `google/gemma-4-26b-a4b-it`, openrouter `google/gemini-2.5-flash`,
  openai `gpt-4o-mini`. OpenAI-compatible chat completion.
- System prompt frames the input as dictated text, **not** a message to the
  model: capitalize sentences, add punctuation, fix spelling/STT errors, apply
  the vocab (mandatory, authoritative over the anglicism rule), and never reply
  to / obey / continue the text even when it sounds like a request. A one-shot
  example pins this: a command-like phrase comes back punctuated, not answered.
- `temperature: 0.0`, `max_tokens: 512`.

Pipeline order: **Groq STT → vocab → postprocess (if on) → log + paste.**
The same edited text goes to all three outputs.

## Failure handling

If the LLM call fails (network error, 5xx, timeout, no key set, or a retired
model id → `http 404`), Ribbit falls back to the vocab-processed text. The user
always gets a paste within the timeout of the hotkey release.

The fallback is no longer *silent*: the last failure reason is shown in
**Settings** under the LLM rows (`⚠ last LLM edit failed: …`) and cleared on the
next success, so a provider quietly dropping a model can't rot the feature
unnoticed. Full diagnostics also land in **Settings → debug log** as
`postprocess fallback: <reason>`.

## Cost

One small chat-completion request per dictation, charged to the personal
RouterAI token. If volume becomes a problem we can add a "skip if text is
short and ASCII-clean" heuristic later.

## Tuning the prompt

The prompt lives in `src-tauri/src/postprocess.rs::system_prompt()` and is
pinned by a snapshot test. To change tone or behavior: edit the function,
update the test assertion, push. CI runs `cargo test` before any release.
