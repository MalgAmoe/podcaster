# Guest Preview v1

> Status: **partially implemented**. Browser-side trimming, Phoenix `/api/preview`, Rust `/preview`, and guest direct-preview submission are in place. Simple abuse controls and follow-up polish are still pending.

## Goal

Replace the current guest `3 files` limit with a simple guest preview flow:

- guests upload WAV or MP3 in the existing UI
- the browser trims it to the first 30 seconds in plain JS
- Phoenix sends that trimmed audio directly to Rust for synchronous processing
- Rust returns a processed 30-second WAV preview
- nothing in this path touches S3/R2, Oban, jobs, or billing

Signed-in users keep the current full upload/job flow unchanged.

For v1, reuse the existing guest-user/session model because it is already in place and is the simplest fit for the current platform. Abuse prevention should also stay intentionally simple in the first pass, with clear TODOs where stronger protection belongs later.

## Locked decisions

| Area | Decision |
|---|---|
| Guest result | Processed **30-second audio preview** |
| Guest download | Optional later; not required for v1 |
| Preview storage | None. No S3/R2/MinIO in the preview path |
| Browser trimming | Plain JS with Web Audio API |
| Guest input formats | WAV and MP3 only |
| Trim timing | Immediately after file selection, before submit |
| User-visible trim | User sees the trimmed clip as the actual guest input |
| Phoenix ↔ Rust transport | Direct request/response, no job orchestration |
| Rust hard limit | Rust decodes and enforces preview duration independently |
| Guest identity model | Reuse existing guest-user/session flow |
| Abuse prevention | Minimal v1 limiter + concurrency guard, with TODOs for stronger controls |

## Product behavior

### Guests

- Upload WAV or MP3
- Browser trims to the first 30 seconds
- Submit that clip for synchronous preview processing
- Receive a processed WAV preview and listen in the browser

### Signed-in users

- Keep the current upload → S3 → `/api/jobs` → Oban → Rust job flow
- No guest-preview logic should leak into the normal full-processing path

## Architecture

### Frontend

- Keep the current process page as similar as possible
- Change guest behavior from “upload full file and maybe hit the 3-file cap” to “prepare a preview clip locally first”
- Browser decodes WAV/MP3, trims to 30 seconds, and re-encodes to WAV
- The trimmed clip becomes the guest-side file used for preview submission
- Reuse the current comparison/player UI where practical

### Phoenix

- Add a dedicated `POST /api/preview` path
- Keep using `EnsureGuestUser`
- Accept multipart upload from the browser
- Forward raw audio bytes to Rust synchronously
- Return processed WAV bytes directly to the browser
- Do not create job records, presign uploads, enqueue Oban jobs, or touch billing in this path

### Rust

- Add a dedicated `POST /preview` endpoint in `poddyclip-api`
- Decode incoming audio
- Reject if decoded duration exceeds `PREVIEW_MAX_SECONDS + tolerance`
- Run the normal processing pipeline
- Return WAV bytes directly
- No S3, no webhook, no job state

## HTTP contract

### Browser → Phoenix

`POST /api/preview`

- request: multipart form with trimmed guest audio file
- success: `200 audio/wav`
- errors:
  - `422 invalid_input`
  - `422 too_long`
  - `429 rate_limited`
  - `503 preview_busy`
  - `502 upstream_error`

### Phoenix → Rust

`POST /preview`

- request: raw audio body
- success: `200 audio/wav`
- errors:
  - `422 decode_failed`
  - `422 too_long`
  - `500 internal`

## Abuse prevention, v1

Keep the first pass simple enough to finish:

- Reuse the current guest-user/session identity
- Add a lightweight preview-only rate limiter in Phoenix
- Add a simple in-process concurrency guard around the Rust preview call
- Let Rust be the hard enforcement point for duration

TODOs to leave in code at the right boundaries:

- IP-aware rate limiting
- stronger dual-key guest/IP budgets
- stricter endpoint-level body rejection before multipart parsing
- more detailed telemetry and abuse monitoring

## Important notes from current repo state

- Guest preview now bypasses S3 and the normal job path.
- The old guest `3 files` limit has been removed.
- `Plug.Parsers` is still global in Phoenix. For v1, do not block on a perfect pre-parser rejection design. Add a TODO instead.
- Browser-side trimming is a UX and bandwidth improvement, but Rust must still enforce the decoded-duration cap.
- Current guest preview UI is synchronous and intentionally does not have real backend progress reporting.

## Implementation order

Done:
1. Update this plan and align comments/docs with the new direction
2. Add browser-side WAV/MP3 → trimmed WAV handling in the Solid frontend
3. Add Phoenix preview endpoint
4. Add Rust `/preview` endpoint
5. Switch guest submit from the normal job flow to the new preview flow
6. Remove the old guest `3 files` limit behavior

Next:
1. Add simple Phoenix-side preview limiter
2. Add simple concurrency guard around the Phoenix → Rust preview call
3. Run fuller end-to-end browser validation and tighten UX details where needed

## Test plan

### Frontend/manual

- Guest WAV upload trims immediately to 30 seconds
- Guest MP3 upload trims immediately to 30 seconds
- User can see that the guest input is the clipped preview file
- Signed-in uploads remain unchanged

### Phoenix

- `/api/preview` success path
- limiter and busy-path responses
- route works with existing guest sessions

### Rust

- `/preview` success path
- decode failure handling
- over-limit rejection
- WAV response generation

### Integration

- Guest preview request never creates an S3 object or DB job
- Signed-in job flow still uses the existing job infrastructure

## Explicit non-goals for v1

- No full signup funnel redesign after preview
- No billing integration
- No perfect abuse prevention on day one
- No separate anonymous identity model just for previews
- No download UX requirement beyond optional later follow-up
