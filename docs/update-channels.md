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
2. **Stage checks on real OS runners** — the build is installed and started
   before anyone is offered it:
   - Windows: the NSIS installer runs silently (`/S`), the installed app's
     `ProductVersion` must equal the tag, and the process must still be alive
     10s after launch. A tray app has no window to assert on — a living
     process is the smoke signal. The app is killed and uninstalled after.
   - macOS: `codesign --verify --deep --strict`, `Info.plist` version == tag,
     and the bundle executable must stay alive 10s after launch (Intel build:
     the launch is skipped with a notice when the runner has no Rosetta —
     codesign and version are still verified).
   - Startup touches no hardware — the mic opens only on hotkey, the
     mic-permission request is async — so a clean runner user is a fair stage.
3. **Release canary**: `scripts/canary.mjs` pushes the audio fixtures through
   the live Groq pipeline (same script as the nightly canary). Without
   `GROQ_API_KEY` it skips with a notice and does not block anything, and a
   fixture the provider rate-limits (HTTP 429 — Groq's daily token allowance)
   is reported as skipped rather than failed: "not right now" says nothing
   about whether the pipeline still works.
4. CI verifies the release's `latest.json` (`scripts/verify_manifest.mjs`:
   all three platforms, version == tag, no universal macOS bundle) and mirrors
   it into the fixed `beta` prerelease as `beta.json`. Beta machines update
   immediately; stable users see nothing yet.
5. With every gate green — tests, both builds, both stage checks, the canary —
   the `promote` job marks the release `latest` and stable machines start
   updating. Nothing is left to bake by then, and a release nobody promotes is
   a release nobody receives. Any red gate stops the pipeline before this job,
   so the build stays a prerelease that only beta machines can see.
6. If a promoted release turns out bad: the **Release control** workflow
   (Actions → Release control → Run workflow), action `rollback`,
   tag = the previous good release. The bad one is demoted back to prerelease,
   the good one becomes `latest` again. No rebuild — the stable endpoint just
   follows whichever release is marked `latest`.

Any failed stage check fails the workflow, so a build that cannot start never
becomes a prerelease anyone can install. Both channel actions re-verify the
target release's manifest before touching the channel; promoting what is
already latest (or rolling back to it) is a no-op.

## Switching a machine to beta

Settings (gear) → **update channel** → `beta`. One action: the next update
check polls the beta endpoint (the auto-poll reads the setting from disk each
time — no restart). Switch back to `stable` the same way.

Note: on `stable`, the updater only sees releases *newer* than the installed
one — a machine that ran a beta build does not "downgrade" when switched back,
it simply waits for the next stable release past its version.

## Nightly canary (guard between releases)

`.github/workflows/canary.yml` runs `scripts/canary.mjs` daily at 05:00 UTC
(and on demand) against `main`. It pushes the audio fixtures in
`test/fixtures/audio/` through the live Groq pipeline — STT
(`whisper-large-v3-turbo`), plus the LLM edit pass
(`llama-3.3-70b-versatile`) for fixtures that declare `llm_expected` — and
checks the tokens that carry each fixture's meaning. The release-time canary
(step 3 above) checks the pipeline at ship time; the nightly one catches a
provider retiring a model id or degrading *between* releases. Results land in
the job summary; failures fail the workflow. Without `GROQ_API_KEY` in repo
secrets the canary skips with a notice (forks stay green).

Unit tests replay *recorded* provider responses offline
(`src/pipeline_replay.test.js`, `postprocess.rs::replay_tests`, fixtures in
`test/fixtures/provider-responses/`); the canaries are the live half of the pair.

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
| `GROQ_API_KEY` | both canaries — add to make them run |
