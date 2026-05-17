# LLM post-processing (optional)

After Groq STT and the vocab pass, Ribbit can optionally run the transcribed
text through a small LLM that fixes punctuation, spelling, and anglicisms
before the text is logged and pasted.

Disabled by default. ~150–300 ms latency overhead per transcription when on.

## Enable

1. Open **Settings → edit transcription** → toggle on.
2. A second row **routerai key** appears. Paste a personal RouterAI token
   from <https://routerai.ru/>. The key is stored locally
   (`~/.config/ribbit/.env` on macOS) and never shown again.
3. On the maintainer's Mac the key is auto-loaded once from
   `~/membeme/system/secrets/routerai.key` on first launch — no manual step.

Toggle off any time. The setting persists across restarts.

## What it does

- Model: `google/gemma-4-26b-a4b-it` (OpenAI-compatible chat completion).
- System prompt: "fix spelling, add punctuation, write anglicisms in
  Cyrillic where common; don't change meaning, return only the edited text."
- `temperature: 0.0`, `max_tokens: 512`.

Pipeline order: **Groq STT → vocab → postprocess (if on) → log + paste.**
The same edited text goes to all three outputs.

## Failure handling

If the LLM call fails (network error, 5xx, > 3 s timeout, no key set),
Ribbit silently falls back to the vocab-processed text. The user always
gets a paste within ~3 s of the hotkey release. Diagnostics land in
**Settings → debug log** as `postprocess fallback: <reason>` or
`postprocess enabled but no ROUTERAI_API_KEY — skipping`.

## Cost

One small chat-completion request per dictation, charged to the personal
RouterAI token. If volume becomes a problem we can add a "skip if text is
short and ASCII-clean" heuristic later.

## Tuning the prompt

The prompt lives in `src-tauri/src/postprocess.rs::system_prompt()` and is
pinned by a snapshot test. To change tone or behavior: edit the function,
update the test assertion, push. CI runs `cargo test` before any release.
