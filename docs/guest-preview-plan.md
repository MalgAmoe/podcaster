# Guest Preview — Design & Implementation Plan

> Status: **design locked, implementation deferred**. Scope = local-first; RunPod/production concerns out of scope here and tracked separately in `docs/rust-deploy-plan.md`.

## Goal

Replace the current "3 trials" guest limit with a **30-second processing preview** for guests, backed by real abuse prevention.

- Browser trims uploaded audio to the first 30 seconds, sends only that
- Backend re-validates decoded duration and refuses if over budget
- Rate limits on guest-cookie + IP to make sustained abuse costly
- Concurrency protection around the Rust call so bursts slow down rather than crash things
- No change to the full-processing flow for signed-in users

Signup-path UX is explicitly out of scope for this plan.

## Locked decisions

| Area | Decision |
|---|---|
| Browser upload format (v1) | **WAV** — produced by a Rust/WASM trimmer at upload time; Symphonia handles natively |
| Browser trimming mechanism | **Rust compiled to WASM**, called from SolidJS at upload time (not preview-only). WAV: parse RIFF + byte-slice first 30s, no decode. MP3: Symphonia progressive decode, stop at 30s, emit WAV. Guests already restricted to WAV/MP3 upload. |
| Trim timing | **At upload**, before any network call — user sees the 30s cut immediately, no post-upload surprise |
| Phoenix ↔ Rust transport | Raw audio body (`Content-Type: audio/wav`). Not multipart, not base64 |
| Preview flow | Synchronous HTTP: request carries result, no polling, no channel |
| Storage for previews | None — no S3/R2/MinIO for the preview path. Audio lives in memory only |
| Oban involvement | None in the preview path. Oban stays only on the full-job flow |
| Concurrency control | Custom GenServer semaphore: max running + max queued + max wait |
| Rate limiting | ETS-backed, dual-keyed (guest cookie + IP), deny on stricter budget |
| Duration enforcement | Rust decodes and enforces `≤ PREVIEW_MAX_SECONDS + PREVIEW_TOLERANCE_SECONDS` |
| Limits source of truth | Env vars read by both Phoenix and Rust (not request headers) |
| Response format (v1) | WAV out — defers encoder decision on the return path |
| DB persistence | None for previews in v1; rate-limit state is ETS (accepts "resets on restart") |

## HTTP contract

### Browser → Phoenix

```
POST /api/preview
  Content-Type: multipart/form-data
  Body: audio=<WAV file, trimmed to first 30s>

  200 OK
    Content-Type: audio/wav
    Body: processed audio bytes

  429 rate_limited       { "error": "rate_limited",  "retry_after_ms": 30000 }
  503 preview_busy       { "error": "preview_busy",  "retry_after_ms": 5000 }
  422 too_long           { "error": "too_long", "max_seconds": 30, "tolerance_seconds": 2 }
  422 invalid_input      { "error": "invalid_input" }
  502 upstream_error     { "error": "upstream_error" }
```

### Phoenix → Rust

```
POST /preview
  Content-Type: audio/wav
  Body: raw WAV bytes

  200 OK
    Content-Type: audio/wav
    Body: processed audio bytes

  422 too_long           { "error": "too_long" }
  422 decode_failed      { "error": "decode_failed" }
  500 internal           { "error": "internal" }
```

**No `X-Preview-Max-Seconds` header.** Rust reads its own limit from env. Phoenix reads its own limit from env for pre-flight hints. Defense in depth.

## Rust `/preview` handler

- Stateless. No S3, no webhooks, no job table.
- Reads raw body → decodes with Symphonia → checks decoded duration.
- If duration > `PREVIEW_MAX_SECONDS + PREVIEW_TOLERANCE_SECONDS`, reject `422 too_long` before running the pipeline.
- Runs the full audio pipeline (same `process_audio` flow as the main path).
- Encodes as WAV, returns bytes.
- Local dev: `cargo run` the Rust API, hit `/preview` directly — no MinIO needed.

## Phoenix semaphore (custom GenServer)

Three independent bounds:

| Bound | Default | Behaviour |
|---|---|---|
| Max concurrency | 2 | Requests over this wait in queue |
| Max queue depth | 10 | Queue full → reject immediately with `503 preview_busy` |
| Max wait time | 5000 ms | Waiter times out → reject with `503 preview_busy` |

All env-configurable. Custom GenServer chosen over `:poolboy` / `:sbroker` because the three-bound policy is trivial to express directly and the generic pools don't match the exact semantics we want.

### States exposed to callers

- `{:ok, token}` — acquired, caller proceeds
- `{:error, :queue_full}` — immediate rejection
- `{:error, :wait_timeout}` — queued but didn't get a slot in time

Both errors map to `503 preview_busy`. The distinction is only for telemetry.

### Where it's called

Inside the preview controller, wrapping the outbound Rust HTTP call specifically. Not as a plug — plug-level acquisition would grab the slot before body read, wasting capacity on malformed uploads.

## Rate limiter (ETS, dual-keyed)

- Keyed on **guest cookie** and **IP** independently
- Both buckets must allow the request; denial from either → `429 rate_limited`
- Starting placeholder budgets (tune via telemetry):
  - Per cookie: **5 previews / hour**
  - Per IP: **20 previews / hour**
- ETS-backed, same pattern as `backend/lib/poddyclip_backend/rate_limiter.ex` (which today only gates data export — generalize, don't duplicate)

### Returns to controller

- `:ok` — proceed
- `{:error, :cookie_limited, retry_after_ms}`
- `{:error, :ip_limited, retry_after_ms}`

Controller maps both to `429` with the appropriate `retry_after_ms`. Distinction survives in telemetry.

## Pipeline order

1. `EnsureGuestUser` — ensures every request has a stable cookie-keyed identity for rate limiting
2. **Preview rate-limit plug** — rejects cheaply before body read
3. Preview controller — parses multipart body
4. Semaphore acquire inside controller
5. HTTP call to Rust `/preview`
6. Semaphore release, stream response back

## Env vars

| Var | Default | Used by |
|---|---|---|
| `PREVIEW_MAX_SECONDS` | 30 | Phoenix + Rust |
| `PREVIEW_TOLERANCE_SECONDS` | 2 | Rust (decoded-duration gate) |
| `PREVIEW_MAX_CONCURRENCY` | 2 | Phoenix (semaphore) |
| `PREVIEW_MAX_QUEUE_DEPTH` | 10 | Phoenix (semaphore) |
| `PREVIEW_MAX_WAIT_MS` | 5000 | Phoenix (semaphore) |
| `PREVIEW_RATE_COOKIE_PER_HOUR` | 5 | Phoenix (rate limiter) |
| `PREVIEW_RATE_IP_PER_HOUR` | 20 | Phoenix (rate limiter) |

## Telemetry (day one)

Events:

- `[:preview, :semaphore, :acquired]`
- `[:preview, :semaphore, :queued]`
- `[:preview, :semaphore, :rejected_full]`
- `[:preview, :semaphore, :rejected_timeout]`
- `[:preview, :rate_limiter, :cookie_limited]`
- `[:preview, :rate_limiter, :ip_limited]`
- `[:preview, :request, :start]`
- `[:preview, :request, :stop]`

Measurements on `:stop`:

- `queue_wait_ms` — time from controller entry to semaphore acquisition
- `rust_roundtrip_ms` — time from request sent to response received
- `total_ms` — end-to-end
- `bytes_in` / `bytes_out` — upload and response sizes

Purpose: tune the seven env vars above without guessing.

## Verify-before-coding items

Each could invalidate part of the design:

1. **`Plug.Parsers` ordering** — current `endpoint.ex:66-70` mounts `Plug.Parsers` globally. If the multipart body is parsed before route-scoped plugs run, the "cheap early reject" goal is lost. Either:
   - Move the preview rate-limit plug into `endpoint.ex` before `Plug.Parsers`, **or**
   - Scope a narrower `Plug.Parsers` to the preview route, dropping the global parser for that path.
   - Confirm with a test upload once wired: a rate-limited request should not transfer the full body.

2. **Preview route pipeline** — none of the current `/api/*` pipelines apply directly.
   - Must include `EnsureGuestUser`
   - Must **not** include `require_non_guest_user`
   - CSRF: mirror `/api/jobs` (scoped API, skips CSRF)

3. **Rust/WASM trimmer** — new crate (e.g. `crates/poddyclip-wasm-trim`) with two entrypoints: `trim_wav(bytes, seconds) -> Vec<u8>` (header parse + slice, no decode) and `trim_mp3_to_wav(bytes, seconds) -> Vec<u8>` (Symphonia progressive decode, stop at 30s, emit WAV). Build with `wasm-pack`, bundle via esbuild. Expected size ~500KB–1MB gzipped for Symphonia + mp3 feature. WAV parser must handle chunks in arbitrary order, IEEE float / extensible format codes, and reject RF64 / unknown bit depths cleanly.

4. **Rust `ProcessConfig` for preview** — does it reuse `ProcessConfig::new()`, or does preview want a trimmed config (e.g. skip analysis stages that assume long audio)? Likely reuse, but worth verifying on first integration.

## Implementation order (when we pick this up)

1. Verify body-parser ordering and route-pipeline plumbing (step 1 and 2 above)
2. Phoenix semaphore GenServer + telemetry events
3. Phoenix rate-limit plug (generalize `RateLimiter`) + telemetry events
4. `Processing.Client.preview/2` — raw-body HTTP call with typed error atoms
5. `PreviewController` gluing it all together
6. Rust `/preview` handler: decode → duration check → pipeline → WAV out
7. Rust/WASM trimmer crate + `wasm-pack` build; SolidJS calls it from `UploadZone` to trim at upload time, POST resulting WAV to `/api/preview`, render returned blob via `WaveformPlayer` (same pattern as `JobComplete.jsx`)
8. Flip the "guest blocked after 3 jobs" UX to "guest gets preview, signup for full"
9. Remove the dead `window.userTotalSeconds` branch in `UploadZone.jsx:103-105`

## What this explicitly does not touch

- Full-job processing pipeline for signed-in users
- Oban queues, webhooks, S3 upload flow
- Billing / seconds accounting
- Magic-link signup flow (already serves as bot-friction layer)
- RunPod deployment (tracked in `docs/rust-deploy-plan.md` — preview endpoint may later be deployed as a separate RunPod function, but that's a v2 concern)

## On-the-wire cost (v1 estimate)

30s of 44.1kHz stereo 16-bit WAV ≈ **5 MB**. Each preview = ~5 MB upload + ~5 MB download = **~10 MB of traffic**. Acceptable for v1. Telemetry `bytes_in` / `bytes_out` will confirm real aggregate; if this becomes a UX issue, switch browser output to Opus/WebM (Symphonia supports it).
