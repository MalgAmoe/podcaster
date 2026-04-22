# Fly.io + RunPod Deployment Plan

## Summary

Target production split:

- **Phoenix** on **Fly.io**
- **Rust API** (`poddyclip-api`) on **RunPod Serverless**
- **R2** remains the production object store for full jobs

This fits the current codebase well:

- Phoenix already owns auth, billing, guest preview policy, job records, and the UI
- the Rust API already exposes HTTP endpoints Phoenix can call
- full jobs already use object storage keys instead of direct browser-to-Rust uploads
- guest preview is already a separate synchronous path and does not use storage

This document is a deployment plan for the current architecture, not a redesign.

## Runtime Shape

### Signed-in full jobs

1. Browser asks Phoenix for `/api/presign-upload`
2. Browser uploads the original file directly to **R2**
3. Phoenix creates the DB job and calls Rust `POST /jobs` with the R2 input key
4. Rust downloads from R2, processes the file, uploads the result back to R2
5. Rust calls Phoenix webhook `/api/internal/jobs/:id/status`
6. Phoenix updates job state and the browser UI

### Guest preview

1. Browser trims the upload locally to a 30-second WAV preview
2. Browser sends it to Phoenix `POST /api/preview`
3. Phoenix forwards the raw bytes to Rust `POST /preview`
4. Rust returns processed WAV bytes
5. Phoenix returns the preview to the browser

Important consequence:

- **R2 is only part of the full-job path**
- guest preview remains synchronous and stateless

## Why This Split Makes Sense

### Fly.io for Phoenix

Phoenix is a good fit for Fly:

- normal web app deployment model
- easy secret management
- straightforward Postgres story
- public HTTPS app with stable domain for the Rust webhook callback

### RunPod for Rust

`poddyclip-api` is the heavy compute side:

- MossFormer2 is the expensive part
- GPU-backed scaling belongs here
- Phoenix should not carry the audio-processing runtime burden

Use **RunPod load-balancing Serverless endpoints**, not queue-based endpoints.

Why:

- this Rust app is already a custom HTTP server
- it already exposes multiple custom routes (`/jobs`, `/preview`, `/health`)
- queue-based RunPod endpoints use a different handler model and are the wrong fit

## RunPod Shape

### Endpoint type

Use a **load-balancing Serverless endpoint**.

That lets the Rust service stay as an Axum HTTP server with its current route model.

Expected route style on RunPod:

- `POST /jobs`
- `DELETE /jobs/:id`
- `POST /preview`
- `GET /ping`

### Important RunPod constraints

Current RunPod load-balancing docs describe these constraints:

- workers need a `/ping` health endpoint
- workers use `PORT` / `PORT_HEALTH`
- request and response payload limit is `30 MB`
- per-request processing timeout is `5.5 min`

Implications for this app:

- `POST /preview` fits fine
  - 30-second WAV previews are small enough
  - preview should complete quickly
- `POST /jobs` also fits
  - the request itself returns quickly
  - the heavy work continues in a spawned background task inside the worker

Important note:

The current Rust app has `GET /health`, but RunPod load-balancing expects `GET /ping`.
Add a real `/ping` route before deploying there.

## Fly.io Shape

### First version

Keep Fly simple:

- one Fly app for Phoenix
- one Postgres database
- no Cloudflare changes required
- admin endpoint disabled unless explicitly needed

### What Fly owns

- public website and main domain
- Phoenix JSON API
- auth and sessions
- webhook callback target for Rust
- billing and account logic
- R2 presigned uploads

## R2 Shape

R2 stays the production object store for full jobs.

Both sides need access:

- **Phoenix**
  - presign upload
  - storage cleanup
  - download URLs
- **Rust**
  - download input
  - upload result
  - delete objects if needed

No architectural change is needed here.

## Required Code Changes Before First Deploy

### Rust API

1. Add `GET /ping`
   - RunPod readiness/health route
   - should return healthy only when the worker is ready

2. Add a production Dockerfile for `poddyclip-api`
   - build Rust binary
   - include runtime dependencies
   - use an NVIDIA CUDA/cuDNN runtime base
   - bake MossFormer2 assets into the image for v1

3. Decide model asset strategy
   - **simple first version:** bake model assets into the image
   - **later only if needed:** externalize model delivery after measuring real cold starts

4. If model assets are ever moved out of the image later, make model loading use an env-driven root path

### Phoenix

1. Add Fly deployment config
   - `fly.toml`
   - release/migrate strategy

2. Point Phoenix to RunPod with env config
   - `poddyclip_api_url`

3. Set webhook base URL to the Fly public domain

No major architectural Phoenix change should be required.

## Environment Variables

### Phoenix on Fly

Expected production env set:

- `DATABASE_URL`
- `SECRET_KEY_BASE`
- `PHX_HOST`
- `PORT`
- `PODDYCLIP_API_URL`
- `WEBHOOK_BASE_URL`
- `WEBHOOK_SECRET`
- `API_KEY`
- `S3_ENDPOINT` or equivalent R2 endpoint env
- `S3_BUCKET`
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` or current R2-compatible naming
- Polar secrets
- mailer secrets if email delivery is enabled

### Rust on RunPod

Expected production env set:

- `PORT`
- `PORT_HEALTH`
- `API_KEY`
- `WEBHOOK_SECRET`
- `CORS_ORIGINS`
- `R2`/S3-compatible storage credentials and endpoint
- `MODEL_ROOT` if model path is externalized
- timeout/concurrency envs as needed

## Recommended Rollout Order

### Phase 1: Stabilize Fly Phoenix

Deploy Phoenix to Fly first while keeping Rust on the current server.

Goal:

- prove Fly deployment
- prove Postgres
- prove R2 access
- prove domain / TLS / mail / billing env setup

### Phase 2: Containerize Rust

Build a production image for `poddyclip-api` and run it locally first.

Recommended shape:

- multi-stage build
- NVIDIA `cudnn-runtime` base for the final image
- no heavy RunPod/PyTorch base image
- one image containing app + runtime libs + model assets

Validate:

- `/jobs`
- `/preview`
- `/health`
- new `/ping`
- R2 connectivity
- webhook delivery

### Phase 3: Deploy Rust to RunPod

Deploy the Rust image to a **load-balancing Serverless endpoint**.

Validate:

- worker health
- cold starts
- MossFormer2 startup
- R2 download/upload
- webhook back to Fly

### Phase 4: Switch Phoenix to RunPod

Update `PODDYCLIP_API_URL` on Fly to the RunPod endpoint URL.

Then test end-to-end:

- guest preview
- signed-in full job
- result upload
- webhook completion
- download URL flow

## Operational Notes

### Guest preview

Guest preview stays on the current model:

- browser trim
- Phoenix synchronous proxy
- Rust synchronous preview

No queueing is introduced here.

### Preview shaping

Current guest preview shaping in Phoenix is node-local.

That is acceptable for the initial Fly deployment, especially if Phoenix is single-instance.
If Phoenix later scales horizontally, review:

- `PreviewGate`
- any ETS-based assumptions

### RunPod queueing

RunPod load-balancing endpoints do **not** provide queue semantics for overload.

That is acceptable here because:

- `POST /preview` is intentionally fail-fast
- `POST /jobs` returns quickly and the Rust app owns its own job task lifecycle

### Cloudflare

Optional later.

It is not required for the first production split.
Do not add it unless there is a concrete reason:

- caching
- WAF
- custom DNS/proxy decisions

## First-Version Recommendation

Keep the first deploy conservative:

- Fly Phoenix app
- Fly Postgres or equivalent production Postgres
- RunPod load-balancing endpoint for Rust
- R2 unchanged
- model assets baked into the Rust image first
- no Cloudflare changes yet

This is the least moving-parts version of the split.

## Open Questions

- Do we want model assets baked into the Rust image first, or externalized immediately?
- Do we want one Fly region first, or multi-region later?
- Do we want Rust `/ping` to report “ready” only after model/session init completes?
  - recommended: yes
- Do we want a separate staging Fly app and staging RunPod endpoint before prod cutover?
  - recommended: yes, if time allows

## References

- Fly.io docs: https://fly.io/docs/
- Fly app configuration: https://fly.io/docs/reference/configuration/
- Fly secrets: https://fly.io/docs/apps/secrets/
- Fly managed Postgres: https://fly.io/docs/mpg/overview/
- RunPod Serverless overview: https://docs.runpod.io/serverless/overview
- RunPod load-balancing overview: https://docs.runpod.io/serverless/load-balancing/overview
- RunPod network volumes: https://docs.runpod.io/storage/network-volumes
