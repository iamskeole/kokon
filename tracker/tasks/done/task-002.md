# task-002 — named checkout backends/stable-diffusion.cpp @ pinned commit, CUDA build, klein engine verified

WP0 step 2. The engine slot becomes a named directory (a "stable" checkout
of the upstream repo) with a clean, reproducible build.

## context

- PLAN WP0; decisions in [PLAN.md](../../PLAN.md)
- Fork build being replaced: `/home/p/ml/__mess/sd.cpp/` (branch 1814814,
  +PUP) — stays in place until WP2/WP3 finish.
- The live kokon-service config still points at the fork build
  (`/home/p/ml/__mess/sd.cpp/klein-engine/sd-server`); this task builds and
  verifies the stock engine, it does not yet flip the live alias.

## goal

- `backends/stable-diffusion.cpp/` = clean checkout of
  `https://github.com/leejet/stable-diffusion.cpp`, pinned to a tagged
  commit (tag line = the "stable" pin, exactly like `backends/stable/`).
- Build CUDA (sm_86) per the fork's build recipe (minus PUP, plus
  `--flash-attn` / `--sage-attn` defaults) → `deploy/klein/` artifact.
- One real klein t2i through the raw engine API proves the engine works.

## build recipe (from the working fork build; re-apply here)

- cmake: `-DGGML_SYCL=OFF -DBUILD_SHARED_LIBS=OFF -DBUILD_KLEIN=ON
  -DCUTLASS_DIR=<repo>/thirdparty/cutlass`
  (cutlass submodule = the one the fork used: `thirdparty/cutlass` latest/
  release — same commit as the fork's, from its `submodules/` copy)
  plus `-DCMAKE_CUDA_ARCHITECTURES=86`
- nvcc at `/usr/local/cuda-13.2/bin/nvcc` (not on default PATH; export
  CUDACXX or set CMAKE_CUDA_COMPILER)
- `make -j12 sd-server`
- engine flags for klein (the live klein recipe, minus all `--pup-*`):
  `-m flux3-image-klein-Q8_0.gguf --llm Qwen3VL-4B-Instruct-Q4_K_M.gguf
  --llm_vision mmproj-Qwen3VL-4B-Instruct-Q8_0.gguf --vae ae.safetensors
  --flash-attn on --sage-attn --eager-load --max-vram 0 --rng cuda`
  (models from `/home/p/ml/models/sd-cpp/flux3/` — task-003 moves the dir)

## acceptance

- [x] checkout exists at `backends/stable-diffusion.cpp/` at the pinned
      commit, `git status` clean (no dirty files)
- [x] `make sd-server` succeeds with the flags above
- [x] the klein engine binary is deployable to `deploy/klein/`
- [x] `sd-server --help` shows no `--pup-*` flags (stock is PUP-less)
- [x] one klein t2i request through the raw engine API succeeds
      (GPU window; vllm healthy after)

## decisions

- **Named slot `backends/stable-diffusion.cpp`** (owner, 2026-11-01, chat):
  a named checkout in the `backends/` slot convention. It is a git
  checkout (pinned), not a copy — the only non-code artifact is
  `deploy/` (untracked, like `backends/stable/`'s local scripts).
- Pin = upstream tag/commit; the pin is written into this file (see done
  notes) and stays until a later task moves it.
- One checkout, one build, serves both pipelines (klein now; qwen in
  WP3) — no second tree.
- The GabrieleLozupone `1814814` branch is no longer needed for the
  engine; its only surviving role (kokon's `sdcpp.rs` speaks its
  request format) becomes the WP2/WP3 adapter work in our repo.

## done (2026-11-02)

- **Pinned commit: `290148d`** (upstream master as of 2026-11-02; tag
  line `master-1039-290148d`), cloned from `https://github.com/leejet/
  stable-diffusion.cpp`. Submodules (ggml `d49c49e` — leejet's patched
  ggml, cutlass `ad7b2f5`, libwebm, libwebp, server frontend) populated
  by copy from the sibling tree at the identical parent commit (the box
  had no direct network access to the submodule remotes); `.git/modules/`
  copied too so git status works in the named tree.
- Build: `cmake -B build -DBUILD_SHARED_LIBS=OFF -DBUILD_KLEIN=ON
  -DGGML_SYCL=OFF -DCUTLASS_DIR=<tree>/thirdparty/cutlass
  -DCMAKE_CUDA_ARCHITECTURES=86 -DCMAKE_CUDA_COMPILER=/usr/local/
  cuda-13.2/bin/nvcc` + `make -j12 sd-server` → `build/bin/sd-server`
  (137MB static; also `libstable-diffusion.a` + 4 `libggml*.a` copied
  into `deploy/klein/` for future linking). Build time ~6 min.
- `deploy/klein/`: `sd-server` + libs + `sd-cpp-serve-klein-stock.sh`
  (evict-gpu 900 then stock engine on :39170, klein recipe) +
  `sd-cpp-stop-klein-stock.sh` + README. **Not wired into
  llama-swap config yet** — the live alias still runs the fork build;
  the flip is task-011 (WP2), once the Kokon service speaks the stock
  API (see findings below).
- `--help` on the stock binary: no `--pup-*` flags, no PUP in the
  binary strings; klein model loads with the plain conditioner recipe.
- **GPU window (2026-11-02 09:38, third run of the night):** vllm
  unloaded through the gateway, stock engine on scratch port :39171,
  ready in 10s (8680 MiB: DiT 5866 + 4B conditioner 2376 + VAE 160),
  `POST /sdcpp/v1/img_gen` → 202 job → **completed in 20s** → decoded
  768×768 JPEG (`/tmp/stock-klein-img.jpg`), engine killed (TERM, GPU
  free in 1s), vllm reloaded through the gateway, healthy 51s later.
  vllm healthy at end — verified by the agent itself (it runs on vllm).

### Findings (input for task-010 / task-011 — the Kokon adapter work)

- **Upstream renamed the t2i API.** At `290148d` the route is
  `POST /sdcpp/v1/img_gen` — async: 202 + `{id, poll_url:
  /sdcpp/v1/jobs/<id>}`, then `GET /sdcpp/v1/jobs/<id>` (statuses
  queued/generating/completed/failed; image in `images[].b64_json`;
  `POST .../jobs/<id>/cancel` exists). The fork's sync
  `POST /sdcpp/v1/diffusion` returns 404 on the stock build. Kokon's
  `crates/qls-core/src/sdcpp.rs` targets the fork API → WP2/WP3 must
  adapt it (img_gen + job polling, or a thin sync wrapper over the job
  API). Check `/sdcpp/v1/text_gen` (used by the klein conditioner path)
  before task-011.
- **`output_format` validation is stricter:** stock accepts only
  `png|jpeg|webp` (the fork also took `jpg`). One-token smoke is
  unaffected; t2i callers must use `jpeg`.
- **Verified stock klein numbers** (3090, warm page cache): load 10s,
  8403 MiB resident, t2i 768²/16 steps/cfg1 ≈ 20s. The fork klein
  (with 8B PUP) was 17.4GB / 15s — stock is half the footprint, marginally
  slower on this box; the fork's PUP is gone entirely, not configurable.

### Autonomy-protocol hardening (born from tonight's failed first run)

- The first run died at the t2i step (404 route) and its engine-kill
  never executed → engine left on the GPU → vllm could not fit (8.7GB
  held of a 24GB GPU; vllm needs ~23GB) → the box was down ~40min until
  the owner pkilled by hand. **The owner's standing requirement: any
  test that touches the GPU must leave vllm healthy without owner
  action.** Now enforced by construction:
  1. job scripts kill their engine on **every** exit path (pidfile-based
     `free_gpu()` before the vllm reload, success *and* failure);
  2. `llama-swap-deploy/evict-gpu.sh` kill-list now includes the scratch
     port **39171** (a stray test engine can never block a vllm/creative
     load, whichever side does the evicting);
  3. every GPU-window job gets a setsid **watcher** (1h cap) that on
     stall/death runs evict-gpu + gateway vllm reload + FAIL marker;
  4. the agent's turn ends in a **blocking wait on the marker** (a
     launched-and-forgotten job is the failure mode that cost this
     incident).
- Pitfall to remember: never launch a long-lived child with bare
  `setsid CMD &` and record `$!` — setsid forks, so `$!` is a dead
  wrapper pid; the real process becomes unkillable by the pidfile.
  The job script is already setsid'd; plain `CMD &` children get true
  pids and outlive the script (no tty, stdin /dev/null).

## blockers

(none)
