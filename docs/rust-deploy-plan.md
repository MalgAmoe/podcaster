# Rust API → RunPod Serverless Deployment Plan

> Status: **draft, updated from working pod validation**. Based on repo state and RunPod docs as of 2026-04.

## Architecture

Axum HTTP server on a **load-balancing Serverless endpoint** (not queue-based). Keeps the existing Phoenix ↔ Rust API contract intact — the per-stage webhook progress flow, auth, and route design all survive unchanged. Phoenix just points at the RunPod endpoint URL instead of the direct Rust host.

- **Image**: custom Docker image with Rust binary + CUDA runtime + cuDNN + ONNX Runtime GPU libs + baked-in MossFormer2 model assets
- **Endpoint**: load-balancing, GPU, min CUDA 12.x filter, no SKU lock-in

## Versions (pinned)

| Component | Version / choice |
|---|---|
| Base image | NVIDIA `cudnn-runtime` Ubuntu image, pinned tag |
| Build pattern | multi-stage Docker build |
| `ort` crate | current repo pin with `cuda` enabled |
| ONNX Runtime GPU | bundled runtime/provider libs in the image |
| CUDA / cuDNN | pinned to a compatible pair for the chosen ORT build |
| GPU pool | A4500-class and similar, filter by compatible CUDA generation |

## Rust app changes

- Keep the first production image simple:
  - bake `model.onnx` and `mel_fb.bin` into the image
  - do not require a network volume for v1
- Add `GET /ping` returning healthy only once the worker is ready.
- `GET /ping` returns 200 only after MossFormer2 session init completes — this is RunPod's readiness gate, and it's the "init hook."
- Initialize ORT session once at startup, not per-request.
- Ensure ONNX Runtime shared libraries are available in the image and on the runtime library path.

## Endpoint configuration

- **Endpoint type**: load-balancing Serverless
- **Execution timeout**: raised above default 600s — podcast jobs run 5–90 min (default would silently kill long jobs)
- **Idle timeout**: modest bump if traffic is bursty enough that warm reuse matters
- **FlashBoot**: on (default)
- **Env vars**: app secrets and storage settings only; no model-volume dependency for v1
- **Network volume**: not required for the first version

## Model delivery

For v1, keep model delivery simple:

- bake `model.onnx` and `mel_fb.bin` into the image
- rebuild the image when the model changes

Reasoning:

- the model is modest by RunPod standards
- the CUDA/cuDNN base is already several gigabytes
- the extra model weight does not buy enough simplification to justify a separate model-delivery path yet
- one image is easier to reproduce from scratch than image + volume + model sync

## Deployment workflow

### Phase 1 — Bootstrap (one-time, Pod)

Rent a GPU Pod, get Rust + Axum + `ort` + CUDA + MossFormer2 loading end-to-end. Codify the working setup into the repo as `Dockerfile`, entrypoint, and env-var declarations. Tear down the Pod. **Any fix that exists only inside the Pod and not in Git is not real** — it must flow back to the repo before the Pod dies.

This phase already yielded two important repo changes:

- `ort` CUDA enabled in the Rust dependency graph
- explicit CUDA execution provider selection so the app does not silently fall back to CPU

### Phase 2 — Endpoint creation (one-time)

- Build image once, push to GHCR (`ghcr.io/malgamoe/poddyclip-api:<sha>`)
- Create load-balancing Serverless endpoint pointing at that image
- Set env vars (storage, auth, webhook secrets, CORS, etc.)

### Phase 3 — Ongoing (no Pods)

```
local dev → git push → GH Actions builds ghcr.io/…:<sha>
         → CI calls RunPod endpoint update API → rolling release
```

Immutable tags only (commit SHA, never `latest`) — rollback is just pointing the endpoint at the previous SHA.

## Cold-start expectations

Realistic first-request latency: **several seconds**, not milliseconds. Don't plan around the "sub-250ms" FlashBoot figure — that's image revival, not ORT + 500MB model init. Levers, in order of impact:

1. Init session at worker startup, not per-request (covered by `/ping` gate)
2. Keep the image lean relative to the workload:
   use NVIDIA CUDA/cuDNN runtime, not a heavy RunPod/PyTorch image
3. Use a multi-stage build so the final image does not carry Rust toolchains and build deps
4. Raise idle timeout to favor warm reuse

## Cost posture

Break-even between always-on Pod and Serverless for A4000-class sits at roughly **43–63% GPU utilization**. For sporadic podcast jobs, Serverless is fine and scales to zero. If a steady queue of long jobs emerges most of the day, revisit — a Pod gets cheaper fast. Don't over-optimize on day one; revisit once real traffic numbers exist.

## Operational rules

1. **Pinned image tags, always** — commit SHA, not `latest`
2. **One image for v1** — code + runtime libs + model assets ship together
3. **Pods are disposable dev machines**, never production
4. **Endpoint env vars are declared explicitly**, not inherited from local `.env`
5. **Don't lock GPU SKU**, set min CUDA filter and let RunPod schedule across the pool

## Changes to the current repo

- Add `Dockerfile` with:
  - build stage
  - final stage based on NVIDIA `cudnn-runtime`
  - baked-in model assets
  - ONNX Runtime provider libs available at runtime
- Keep `Cargo.toml` and AI-clean session setup aligned with CUDA EP usage
- Add `GET /ping` handler to Axum with the readiness gate
- Add GH Actions workflow: build image, push to GHCR, call RunPod endpoint update

Everything else in the stack (Phoenix, SolidJS, webhook contract, billing, auth) is unaffected.

## Open items to validate

- [ ] Pin the exact NVIDIA `cudnn-runtime` tag we want to ship
- [ ] Decide whether the final image should also keep `poddyclip-cli` for emergency debugging
- [ ] Verify the exact ONNX Runtime shared-library set needed in the final image
- [ ] Add `GET /ping` and startup readiness behavior
- [ ] Measure actual cold-start time with FlashBoot for the final image
- [ ] Decide on GH Actions → endpoint-update auth (API key in GH secret; rotate policy)
- [ ] Confirm Phoenix webhook delivery works from the RunPod worker network (no egress restrictions)
