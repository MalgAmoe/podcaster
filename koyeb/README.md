# Koyeb GPU API Deployment

This repo can run on Koyeb as a GPU-backed `Web Service` using the existing Rust API in [Dockerfile.api](/Users/malg/biz/podcaster/Dockerfile.api).

This is the intended shape:

1. Phoenix keeps Oban as the queue and orchestrator.
2. Oban submits jobs to the Rust API over HTTP.
3. Koyeb wakes or scales the GPU service as needed.
4. The Rust API downloads from S3/R2, processes audio, uploads the result, and calls back Phoenix.

This is closer to the old pod model than the RunPod stdin worker model.

## Recommended Koyeb Service

- Service type: `Web Service`
- Instance type: start with `gpu-nvidia-l4`
- Minimum instances: `0` if you accept cold starts
- Maximum instances: `1` initially
- Health check: HTTP `GET /health`
- Exposed port: `3000`

Why `Web Service`:

- Koyeb scale-to-zero wakes on incoming Internet traffic.
- The Rust API already binds to `0.0.0.0:$PORT`.
- Phoenix already knows how to call the Rust API over HTTP.

## Build and Push the Image

Use the existing helper:

```bash
API_IMAGE_REPO=your-registry/your-image \
API_IMAGE_TAG=api-koyeb \
API_PLATFORM=linux/amd64 \
./dev.sh build-api-push
```

Or directly:

```bash
docker buildx build \
  --platform linux/amd64 \
  -f Dockerfile.api \
  -t your-registry/your-image:api-koyeb \
  --push \
  .
```

## Koyeb Service Settings

Create a `Web Service` from the pushed container image.

Set:

- Port: `3000`
- Health check path: `/health`
- Instance type: `gpu-nvidia-l4`
- Scaling:
  - Min instances: `0`
  - Max instances: `1`

If Koyeb exposes startup grace / health settings, give the service room to load models before marking it unhealthy.

## Environment Variables for the Rust API

Use [poddyclip-api.env.example](/Users/malg/biz/podcaster/koyeb/poddyclip-api.env.example) as the baseline.

Required:

- `API_KEY`
- `S3_ENDPOINT`
- `S3_BUCKET`
- `S3_REGION`
- `S3_ACCESS_KEY`
- `S3_SECRET_KEY`

Recommended:

- `PORT=3000`
- `RUST_LOG=info`
- `JOB_TIMEOUT_SECONDS=600`
- `MAX_FILE_SIZE_MB=2048`

Optional:

- `CORS_ORIGINS`
- `RESULT_RETENTION_SECONDS`
- `PREVIEW_MAX_SECONDS`
- `PREVIEW_TOLERANCE_SECONDS`
- `PREVIEW_MAX_CONCURRENCY`

## Phoenix Changes

For the Koyeb path, use the direct pod/API backend, not the RunPod queue backend.

Set Phoenix environment roughly like:

```bash
PODDYCLIP_PROCESSING_BACKEND=pod
PODDYCLIP_API_URL=https://your-koyeb-service-url
API_KEY=the-same-api-key-configured-on-the-rust-service
WEBHOOK_BASE_URL=https://your-phoenix-app
WEBHOOK_SECRET=your-webhook-secret
```

Notes:

- `PODDYCLIP_API_URL` should point at the public Koyeb service URL.
- `API_KEY` in Phoenix must match the Rust API `API_KEY`.
- `WEBHOOK_BASE_URL` and `WEBHOOK_SECRET` remain how Phoenix authenticates Rust callbacks.

## Expected Tradeoff

This avoids the RunPod mixed-fleet GPU problem, but it does not remove cold starts.

With `min instances = 0`:

- first request after sleep wakes the GPU service
- model load happens on that first request
- later requests can reuse the same warm instance

If cold starts are too painful, increase `min instances` to `1`.

## What Not to Use

Do not use the current `poddyclip-serverless` stdin worker model on Koyeb.

Koyeb is a better fit for:

- long-lived HTTP service
- scale-to-zero on a web endpoint

This repo already has that service in `poddyclip-api`.
