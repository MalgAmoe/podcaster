# Rust API → RunPod Serverless Deployment Plan

> Status: **draft, to be validated**. Based on RunPod docs as of 2026-04. Pin exact versions and test each phase before committing to the design.

## Architecture

Axum HTTP server on a **load-balancing Serverless endpoint** (not queue-based). Keeps the existing Phoenix ↔ Rust API contract intact — the per-stage webhook progress flow, auth, and route design all survive unchanged. Phoenix just points at the RunPod endpoint URL instead of the direct Rust host.

- **Image**: Rust binary + CUDA runtime + cuDNN + ONNX Runtime GPU (`libonnxruntime.so`)
- **Network volume**: MossFormer2 weights, mounted at `/runpod-volume/models/mossformer2/`
- **Endpoint**: load-balancing, GPU, min CUDA 12.x filter, no SKU lock-in

## Versions (pinned)

| Component | Version |
|---|---|
| Base image | `nvidia/cuda:12.6.0-cudnn-runtime-ubuntu22.04` (pinned tag, cuDNN 9) |
| `ort` crate | `=2.0.0-rc.12`, features `["cuda", "load-dynamic"]` |
| ONNX Runtime GPU | `v1.24.3` (`libonnxruntime.so` shipped in image) |
| CUDA / cuDNN | 12.x / 9.x |
| GPU pool | A4000/A5000/3090/4090/L4/A40/… (filter by min CUDA 12.x) |

## Rust app changes

- Env-driven model path:
  - `MODEL_ROOT=crates/poddyclip/models` for local dev
  - `MODEL_ROOT=/runpod-volume` for prod
- `GET /ping` returns 200 only after MossFormer2 session init completes — this is RunPod's readiness gate, and it's the "init hook."
- Initialize ORT session once at startup, not per-request.
- Set `ORT_DYLIB_PATH` to the bundled `libonnxruntime.so`.

## Endpoint configuration

- **Endpoint type**: load-balancing Serverless
- **Execution timeout**: raised above default 600s — podcast jobs run 5–90 min (default would silently kill long jobs)
- **Idle timeout**: modest bump if traffic is bursty enough that warm reuse matters
- **FlashBoot**: on (default)
- **Env vars**: `MODEL_ROOT=/runpod-volume`, plus any secrets (declared on the endpoint, not inherited from local `.env`)
- **Network volume**: single volume for v1; multi-region only if availability becomes a real problem

## Model delivery

Populate the network volume via RunPod's **S3-compatible API** — no temporary Pod rental needed. One-time upload from your local machine:

```
s3://<volume-id>/models/mossformer2/model.onnx
s3://<volume-id>/models/mossformer2/mel_fb.bin
```

Update the model later the same way — no image rebuild required.

## Deployment workflow

### Phase 1 — Bootstrap (one-time, Pod)

Rent a GPU Pod, get Rust + Axum + `ort` + CUDA + MossFormer2 loading end-to-end. Codify the working setup into the repo as `Dockerfile`, entrypoint, and env-var declarations. Tear down the Pod. **Any fix that exists only inside the Pod and not in Git is not real** — it must flow back to the repo before the Pod dies.

### Phase 2 — Endpoint creation (one-time)

- Build image once, push to GHCR (`ghcr.io/malgamoe/poddyclip-api:<sha>`)
- Create load-balancing Serverless endpoint pointing at that image
- Attach the pre-populated network volume
- Set env vars (`MODEL_ROOT`, etc.)

### Phase 3 — Ongoing (no Pods)

```
local dev → git push → GH Actions builds ghcr.io/…:<sha>
         → CI calls RunPod endpoint update API → rolling release
```

Immutable tags only (commit SHA, never `latest`) — rollback is just pointing the endpoint at the previous SHA.

## Cold-start expectations

Realistic first-request latency: **several seconds**, not milliseconds. Don't plan around the "sub-250ms" FlashBoot figure — that's image revival, not ORT + 500MB model init. Levers, in order of impact:

1. Init session at worker startup, not per-request (covered by `/ping` gate)
2. Keep the image lean (only what CUDA/cuDNN/ORT/binary need)
3. If `/runpod-volume` read proves slow, copy the `.onnx` to container-local `/tmp` at startup and load from there (measure first)
4. Raise idle timeout to favor warm reuse

## Cost posture

Break-even between always-on Pod and Serverless for A4000-class sits at roughly **43–63% GPU utilization**. For sporadic podcast jobs, Serverless is fine and scales to zero. If a steady queue of long jobs emerges most of the day, revisit — a Pod gets cheaper fast. Don't over-optimize on day one; revisit once real traffic numbers exist.

## Operational rules

1. **Pinned image tags, always** — commit SHA, not `latest`
2. **Volume and image are independent** — code changes redeploy, model changes are S3 uploads
3. **Pods are disposable dev machines**, never production
4. **Endpoint env vars are declared explicitly**, not inherited from local `.env`
5. **Don't lock GPU SKU**, set min CUDA filter and let RunPod schedule across the pool

## Changes to the current repo

- Add `Dockerfile` targeting the pinned CUDA base, installing `libonnxruntime.so` 1.24.3
- Update `Cargo.toml` for `ort` pin and features (`cuda`, `load-dynamic`)
- Refactor model-loading to read `MODEL_ROOT` env var (currently hardcoded path)
- Add `GET /ping` handler to Axum with the readiness gate
- Add GH Actions workflow: build image, push to GHCR, call RunPod endpoint update

Everything else in the stack (Phoenix, SolidJS, webhook contract, billing, auth) is unaffected.

## Open items to validate

- [ ] Confirm `ort 2.0.0-rc.12` + ONNX Runtime GPU `v1.24.3` actually build cleanly with the `cuda` + `load-dynamic` feature combo
- [ ] Confirm the exact `libonnxruntime.so` filename(s) shipped in ORT 1.24.3 GPU release tarball
- [ ] Benchmark `/runpod-volume` read speed vs. container-local for the ONNX load on first request
- [ ] Verify RunPod's S3 API supports multipart upload for the 500MB+ model file
- [ ] Measure actual cold-start time with FlashBoot for the final image
- [ ] Decide on GH Actions → endpoint-update auth (API key in GH secret; rotate policy)
- [ ] Confirm Phoenix webhook delivery works from the RunPod worker network (no egress restrictions)
