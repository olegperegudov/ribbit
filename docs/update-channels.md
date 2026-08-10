# Update channels

Ribbit ships two update streams from one binary. The endpoint is chosen in
code (Settings → update channel), not baked per build, so switching channels
never needs a reinstall.

| channel | who gets it | endpoint |
|---|---|---|
| `stable` (default) | everyone | `releases/latest/download/latest.json` |
| `beta` | machines that opted in | `releases/download/beta/beta.json` |

## Release flow

1. Push to `main` → CI bumps the patch version, runs vitest + cargo tests,
   builds Windows + macOS, and publishes the release **as a prerelease**.
2. CI verifies the release's `latest.json` (`scripts/verify_manifest.mjs`:
   all three platforms, version == tag, no universal macOS bundle) and mirrors
   it into the fixed `beta` prerelease as `beta.json`. Beta machines update
   immediately; stable users see nothing yet.
3. After the beta bake-in, promote with the **Release control** workflow
   (Actions → Release control → Run workflow): action `promote`, tag `vX.Y.Z`.
   The release becomes `latest` and stable machines start updating.
4. If a promoted release turns out bad: same workflow, action `rollback`,
   tag = the previous good release. The bad one is demoted back to prerelease,
   the good one becomes `latest` again. No rebuild — the stable endpoint just
   follows whichever release is marked `latest`.

Both actions re-verify the target release's manifest before touching the
channel. Promoting what is already latest (or rolling back to it) is a no-op.

## Switching a machine to beta

Settings (gear) → **update channel** → `beta`. One action: the next update
check polls the beta endpoint (the auto-poll reads the setting from disk each
time — no restart). Switch back to `stable` the same way.

Note: on `stable`, the updater only sees releases *newer* than the installed
one — a machine that ran a beta build does not "downgrade" when switched back,
it simply waits for the next stable release past its version.

## Canary (nightly live check)

`.github/workflows/canary.yml` runs `scripts/canary.mjs` daily at 05:00 UTC
(and on demand). It pushes the audio fixtures in `test/fixtures/audio/`
through the live Groq pipeline — STT (`whisper-large-v3-turbo`), plus the LLM
edit pass (`llama-3.3-70b-versatile`) for fixtures that declare
`llm_expected` — and checks the tokens that carry each fixture's meaning.
Results land in the job summary; failures fail the workflow. Without
`GROQ_API_KEY` in repo secrets the canary skips with a notice (forks stay
green).

Unit tests replay *recorded* provider responses offline
(`src/pipeline_replay.test.js`, `postprocess.rs::replay_tests`, fixtures in
`test/fixtures/provider-responses/`); the canary is the live half of the pair.

### Adding an audio fixture

```sh
say -v Samantha -o /tmp/f.aiff "Please merge this pull request before noon"
afconvert -f WAVE -d LEI16@16000 /tmp/f.aiff test/fixtures/audio/en-merge.wav
```

(Russian voice: `Milena`.) Then add an entry to
`test/fixtures/audio/manifest.json`. Rules of thumb:

- `expected` tokens are case-insensitive substring checks against the
  transcript — use **stems** (`ветк`, not `ветки`) so harmless model drift
  doesn't turn the canary red.
- Assert only the words that carry the fixture's meaning.
- Add `llm_expected` when the fixture should also survive the LLM edit pass.

## Secrets the repo needs

| secret | used by |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | updater artifact signing (already set) |
| `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY` | macOS signing (already set) |
| `GROQ_API_KEY` | the canary — add to make it run |
