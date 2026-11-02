# task-008 — service image build + `frontends/kokon` deploy + e2e smoke

## goal
Build the service image (quiet window — a container restart kills web
sessions) and deploy it into the host's `frontends/kokon/` (compose + env
pointing at the gateway; the dir is currently empty). End-to-end smoke of
the new wiring: UI → service → **sidecar PUP** → **stock engine**.

## acceptance criteria
- Image built + tagged (tag convention confirmed at execution), container
  running from `frontends/kokon/`.
- One klein t2i end-to-end: the caption path provably went through the
  sidecar (service log / job metadata), the render path through the engine;
  the owner validates the image visually.
- Status pill + metadata bar show the real values (the earlier fix
  `47c87a9` stays in force).
- The service token is read at runtime from the container (`/data/
  service-token`) — never written to any file or log.

## blockers
- task-005, task-006, task-007 (the wiring this image ships).
- A quiet window (no active owner web sessions).

## decisions
- Image tag convention (e.g. `0.12.0-dev.N`) confirmed at execution.

## done notes (2026-11-02)

Executed as one long window plus a follow-up gallery smoke; all on the
rebuilt service.

- **Image + deploy.** Built `ghcr.io/iamskeole/kokon:external-engine-0.11.2-4193733`
  from `docker/Dockerfile` (tag = branch short sha, per the convention in
  `frontends/kokon/README.md`), deployed from `/home/p/ml/frontends/kokon/`
  (compose + README written in this task; the dir was empty). The container
  carries the production config (schema 13, `externalEngine.model=kokon-klein`
  through the gateway, VLM endpoint = the gateway's `kokon-pup` alias) and
  the PUP prompt bundle in its data-dir assets (`/data/assets/pup`). The
  service reads its token from `/data/service-token` at runtime; the
  README/compose reference it by path only — nothing new writes the token
  to a file or log.
- **End-to-end proof (live window, vLLM out for the GPU).**
  - t2i job: service → PUP sidecar (enhanced prompt, seed 42, 184 tokens in
    2.6 s — service log) → stock klein engine (`generate_image 768x768`,
    8-step sampling 3.55 s, decode 0.41 s — engine log).
  - box-edit job: EDIT mode with the reference image; the sidecar's
    enhanced prompt **preserved the locked bbox table** (`src_bbox` JSON)
    verbatim.
  - D-FB: with the sidecar killed, the job hard-failed with code
    `upsampler` ("The prompt upsampler at http://127.0.0.1:39175 is not
    answering") — no raw-prompt fallback.
  - vLLM self-healed at window end via the gateway load; no owner
    intervention in the last two windows.
  - The visual-check image for the owner:
    `llama-swap-deploy/jobs/klein-smoke-0.11.2-t2i.png` (also copied into
    the production gallery volume) — a 768x768 PNG, the mug prompt, seed 42.
- **Status pill / metadata bar:** the production service's authed
  `/api/v1/config` returns the real schema-13 values (external wiring,
  model name, VLM endpoint); the `47c87a9` fix is inside this image.

## decisions

- **The gateway cutover was absorbed from task-011.** The live
  `llama-swap-deploy/config.yaml` (and its `.orig`) now point
  `kokon-klein` at the stock `deploy/klein/sd-cpp-serve-klein-stock.sh`
  (+ stop script), with an adopt-path in the serve script (a healthy
  :39170 is watched, not evicted). Task-011 is now mostly
  verification/retirement: the stock engine's HTTP API already matches the
  in-tree `sdcpp.rs` client (async `img_gen`/`jobs`/cancel) — no adapter
  code was needed. `healthCheckTimeout` raised 900→1200 (a cold vLLM load
  once took ~16 min).
- **v269 monitor semantics (measured, important for 009/015):** a model the
  gateway has *loaded* stays in its table and is watched; when its
  endpoint dies the gateway re-runs its `cmd` after a ~7-10 min cooldown
  (process-parentage proven: the gateway spawned `vllm-serve.sh` at 20:17
  after the engine's death). `evict-gpu.sh` stops vLLM (via the stable
  `serve.sh stop`), not just the SD ports. Two watched one-GPU models
  therefore enter an eviction loop (each respawn's evict kills the other).
  **Window rule adopted:** the window's cleanup unloads the `kokon-klein`
  alias from the table (kills the watch) and leaves vLLM loaded+watched —
  vLLM then self-heals by itself if it ever dies, and no loop is possible.
  The watcher fallback does the same unload.
- **The `starting_backend` freeze (17:40 / 18:55 / 20:02 runs, old code) —
  diagnosed as far as this box allows, fixed, and absorbed here.** Pattern:
  job stuck in `starting_backend` for 8+ min, zero HTTP anywhere (gateway
  journal empty, sidecar idle), service otherwise alive until shutdown.
  The old `Engine::gpus()` held a std mutex around an *unbounded*
  `nvidia-smi`; a driver stall during the 23 GB vLLM teardown + two model
  loads pinned the holding worker, and every other caller (job path, API
  handlers) queued behind the mutex on all four tokio workers — tokio
  request-timeouts cannot fire because they are futures on those same
  threads. Fix (commit `4193733`): `run_cmd_bounded` (child killed at the
  deadline; `run_server_probe` on top of it), `nvidia-smi` bounded to 5 s,
  and a `BoundedCache` whose last-known value readers never wait on
  (refresh lock held only during the query; concurrent refreshers collapse
  to one query). After the fix, two consecutive windows with the same
  churn profile completed in ~105 s and ~90 s. dmesg is not readable on
  this host (no root), so the driver-side trigger cannot be fully proven;
  the behaviour-level evidence above is the record.
- **0.11 result-file lifecycle (learned the hard way):** job output files
  are transient. `private: true` → files in the data dir, cleared on
  service shutdown ("private mode cleared (N results, M files)");
  `private: false` → files under `outputDir/<YYYY-MM-DD>/`, persisted.
  The job JSON's `image` field stays `null` in both cases (the `results[]`
  array carries the record); evidence scripts must find files on disk, and
  must search the date subdirectory.
- **MCP protocol tests need `LD_LIBRARY_PATH`** (brew openssl) in the
  environment or the on-demand-spawned service binary fails to load
  (exit 127) and the two service-dependent protocol tests time out. Two
  "failures" on 2026-11-02 were exactly this, not a code regression.
