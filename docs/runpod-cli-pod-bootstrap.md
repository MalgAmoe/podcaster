# RunPod Pod Bootstrap For `poddyclip-cli`

This is the exact pragmatic bootstrap flow we used to get a fresh RunPod GPU pod working for the Rust CLI with MossFormer2 on CUDA.

This is not the final reproducible deployment story. It is the manual bring-up recipe that got the pod into a working state for experimentation.

## Scope

Goal:

- SSH into a fresh RunPod pod
- install the missing system and Rust tooling
- copy the Rust workspace and model assets from the local machine
- build `poddyclip-cli`
- run `--ai-denoise` with CUDA working

Assumptions:

- GPU pod is already created
- SSH access is configured
- model files are not in git and are copied manually from the local machine

## 1. SSH Into The Pod

Example:

```bash
ssh root@213.173.107.140 -p 26777 -i ~/.ssh/id_ed25519
```

## 2. Install Base Packages

Fresh pods did not have all Rust build dependencies installed.

Install:

```bash
apt update
apt install -y \
  pkg-config \
  libssl-dev \
  build-essential \
  cmake \
  clang \
  git
```

## 3. Install Rust

One fresh pod did not have Rust installed.

Install `rustup`:

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

## 4. Copy The Workspace From The Local Machine

Use `rsync`, not repeated `scp`.

Minimum useful set:

- `Cargo.toml`
- `Cargo.lock`
- `crates/`
- MossFormer2 model files
- test audio files if needed

Example from the local machine:

```bash
rsync -avz \
  -e 'ssh -p 26777 -i ~/.ssh/id_ed25519' \
  Cargo.toml Cargo.lock crates/ short.mp3 \
  root@213.173.107.140:/root/podcaster/
```

If the models are not already included in that tree, sync them explicitly too.

## 5. Keep The Project Under `/workspace`

RunPod preserves `/workspace` across pod stop/start. Container-disk paths outside `/workspace` are not the right place for anything you want to keep.

Move or sync the project to:

```bash
/workspace/podcaster
```

Notes:

- `mv` into `/workspace` can print ownership-preservation warnings
- those warnings are about metadata, not necessarily data loss
- verify the files are really there after the move

Sanity check:

```bash
cd /workspace/podcaster
ls
find crates -maxdepth 2 -name Cargo.toml
```

## 6. Important Model-Path Gotcha

Current MossFormer2 model lookup in:

- [crates/poddyclip/src/ai_clean/mod.rs](/Users/malg/biz/podcaster/crates/poddyclip/src/ai_clean/mod.rs)

uses:

```rust
Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("models")
    .join("mossformer2")
```

That means the binary expects the model relative to the path it was **compiled from**.

So:

- if you build under `/root/podcaster`, the binary looks under `/root/podcaster/...`
- if you want the binary to use `/workspace/podcaster/...`, build it from `/workspace/podcaster`

Do not build in `/root` and then expect the binary to pick up the model from `/workspace`.

## 7. Build The CLI

Build from the workspace you actually want the binary to use:

```bash
cd /workspace/podcaster
cargo build --release -p poddyclip-cli
```

## 8. CUDA / ONNX Runtime State

The repo now contains the two changes that were needed to stop silent CPU fallback:

1. `ort` has CUDA enabled in:

- [crates/poddyclip/Cargo.toml](/Users/malg/biz/podcaster/crates/poddyclip/Cargo.toml)

2. MossFormer2 requests the CUDA execution provider explicitly in:

- [crates/poddyclip/src/ai_clean/mod.rs](/Users/malg/biz/podcaster/crates/poddyclip/src/ai_clean/mod.rs)

So if CUDA initialization fails now, it should fail explicitly instead of quietly using CPU.

## 9. Running The Built Binary Directly

Running with `cargo run` worked because Cargo handled some runtime setup.

Running the built binary directly required `LD_LIBRARY_PATH` so ONNX Runtime provider libraries could be found.

Use:

```bash
export LD_LIBRARY_PATH=/workspace/podcaster/target/release:$LD_LIBRARY_PATH
```

Then run:

```bash
cd /workspace/podcaster
target/release/poddyclip short.mp3 -o out.mp3 --ai-denoise
```

Without that, the binary can fail with errors like:

- `libonnxruntime_providers_shared.so: cannot open shared object file`

## 10. Sanity Checks

### GPU visible

```bash
nvidia-smi
```

### CLI run

```bash
cd /workspace/podcaster
export LD_LIBRARY_PATH=/workspace/podcaster/target/release:$LD_LIBRARY_PATH
target/release/poddyclip short.mp3 -o out.mp3 --ai-denoise
```

### Release performance

Do not judge performance from debug builds.

Use release:

```bash
time target/release/poddyclip short.mp3 -o out.mp3 --ai-denoise
```

## 11. What Survives A Pod Stop

RunPod docs say:

- container disk is cleared on stop
- `/workspace` on the volume disk is preserved on stop/start

So before stopping the pod, make sure the useful things are under `/workspace`, especially:

- `/workspace/podcaster`
- model files
- built binaries
- any bootstrap scripts

## 12. Minimal Resume Flow After Restart

After the pod restarts:

```bash
source "$HOME/.cargo/env"
cd /workspace/podcaster
export LD_LIBRARY_PATH=/workspace/podcaster/target/release:$LD_LIBRARY_PATH
target/release/poddyclip short.mp3 -o out.mp3 --ai-denoise
```

If the binary or target dir is missing, rebuild:

```bash
cd /workspace/podcaster
cargo build --release -p poddyclip-cli
```

## 13. Next Step After This Manual Flow

Once this is stable, the next step is not more pod hand-tuning.

The next step is:

- turn these system installs and runtime assumptions into a Docker image
- make the pod launch from that image
- stop depending on manual SSH bootstrap
