# ribbit-server

Ribbit's speech pipeline over HTTP for devices that can record but can't run
Ribbit — first user: the Steam Deck dictating into WoW chat. A WAV goes in,
the finished text comes out: same provider stacks with failover, hallucination
cleanup, LLM editor and vocabulary as the desktop app (`../core`, shared code).

## API

`POST /v1/transcribe` — body: WAV (any rate/channels, int or float PCM, ≤ 60 s,
≤ 4 MB), header `Authorization: Bearer <token>`.

| Status | Body | Meaning |
|---|---|---|
| 200 | `{"text", "edited", "audio_secs"}` | `text` is `""` for silence or < 0.3 s |
| 401 | `{"error"}` | missing or wrong token |
| 422 | `{"error"}` | not a WAV / longer than 60 s |
| 429 | `{"error"}` | more than 30 requests in a minute |
| 502 / 504 | `{"error"}` | every STT provider failed / over 45 s |

`GET /health` → `ok` (no auth, no data).

## Security

- Listens only on the host's tailnet address (`RIBBIT_LISTEN`), host network,
  so it is unreachable from the internet regardless of the firewall.
- Token: the host keeps only its SHA-256; compared in constant time.
- Provider keys live only on the host (`config/ribbit/.env`, 0600, uid 10001).
- Container: non-root 10001, read-only root fs, no capabilities,
  no-new-privileges, 256 MB / 128 pids.
- Audio and text are never stored; logs hold durations, timings, char counts.

## Host setup (once)

```
/opt/ribbit-server/
  .env                  RIBBIT_LISTEN=<tailnet-ip>:8790
                        TAG=
  config/ribbit/        config.json  (audio_providers, text_providers,
                                      languages, postprocess_enabled,
                                      fallback_threshold, fallback_cooldown_mins)
                        vocab.json, .env (provider keys)
  secrets/token.sha256  sha256 hex of the client token
```

`config/` and `secrets/` owned by 10001, dirs 0700, files 0600. The client
keeps the token itself; the host never sees it in plain form.

## Deploy

`server/deploy.sh <ssh-host>` — builds the image on the host from the current
commit (`git archive HEAD core server`), starts it, prunes dangling images.
Rollback: set `TAG=` in `/opt/ribbit-server/.env` to an older commit, whose
image is still on the host, and `docker compose up -d`.

## Tests

`cargo test` in `core/` and in `server/` (both run in CI's Linux test job).
