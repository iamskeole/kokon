# Fresh box: the whole stack

How to stand up the entire Kokon inference stack on a new machine with a single
NVIDIA GPU: the service (this repository), the llama-swap gateway (pinned,
stock upstream), the image engines (stock upstream, named slots), and the
models (a host-level directory with a provenance convention).

Scope: **one machine, one GPU** (reference host: an RTX 3090, 24 GB).
Multi-GPU, multi-tenant and multi-box layouts are out of scope.

The fork's division of labour: **this repository is the only custom layer.**
Everything else is stock upstream at pinned commits; nothing in this stack is
a fork of an engine.

## The stack

```
Kokon service (this repo: docker image)          browser + MCP
        │  loopback HTTP + token
        ▼
llama-swap gateway (stock, pinned) on 127.0.0.1:8081
        │  starts / evicts on demand; TTL unloads
        ▼
engines, all on 127.0.0.1:
  sd-server (stable-diffusion.cpp, pinned)     :39160 qwen  :39170 klein
  llama-server (llama.cpp, pinned)             :39175 klein sidecar
                                               :39176/:39177 qwen sidecars (PUP)
  vLLM (owner's chat model)                    :18020
```

The service speaks only to the **gateway**, never to engines directly: each
pipeline in its config names a gateway alias for its engine and another for its
VLM sidecar. The gateway is the only component that ever starts a process.

## Host layout

Three host-level directories, all outside the repository (non-git):

```
<host>/backends/    llama-swap/        pinned, pristine upstream (v269)
                    llama-swap-deploy/ the live deployment: config.yaml,
                                       serve/stop scripts, systemd unit docs
                                       (non-git; its own README.local.md)
                    stable-diffusion.cpp/  pinned, one build, deploy/{klein,qwen}/
                    llama.cpp/     the sidecars' named build (build/bin/llama-server)
                    <owner's chat-model runtime>   e.g. a vLLM checkout
<host>/frontends/kokon/  the service deployment: compose.yaml (+README)
<host>/models/      gguf/   {org}/{Model}/model-<QUANT>.gguf · mmproj-<PRECISION>.gguf
                    sd-cpp/ {org}/{Model}/  engine DiTs + VAEs; upscalers/ stays flat
                    huggingface/  (owner-owned; not part of this stack)
```

Each model directory carries a `provenance.md`: source org/repo/URL, quant,
license, download date, sha256, converter/imatrix notes. **Provenance is the
record of where a weight came from and what it is; every new model lands with
one.** The directory layout and file naming shown above *are* the convention.

## Prerequisites

Verified on the reference host (Ubuntu 26.04.1 LTS, RTX 3090 24 GB):

| Need | Reference host | Notes |
|---|---|---|
| NVIDIA GPU | RTX 3090, 24 GB | everything on one GPU; the budgets below are measured on it |
| NVIDIA driver + Container Toolkit | driver current; `nvidia-smi` in a container | needed by both the service container and the engines |
| CUDA toolkit | 13.2 (`/usr/local/cuda-13.2`) | for building the engines (sm_86 for the 3090) |
| Go | any recent | builds llama-swap |
| rustup / cargo | rustc 1.98.1 | builds the service (also inside the docker build) |
| Node | 22 | builds the UI (also inside the docker build) |
| Docker + compose | 29.9.1 | runs the service |
| git, cmake, g++, make | — | engine builds |

Disk budget for the models, roughly: the image pipeline (klein + qwen + both
PUP quants + conditioners + VAEs) is about 40 GB; the optional creative models
add another ~35 GB; the owner's chat model is on top (owner-managed). The
per-directory `provenance.md` files carry the exact sizes and hashes — they
are the source of truth, not this table.

## 1. Models

Place everything under `models/` following the convention above. The final
state of this deployment uses:

| Path | Role |
|---|---|
| `sd-cpp/BFL/flux-3-image-klein/model-Q8_0.gguf` | the klein DiT (engine, `kokon-klein`) |
| `sd-cpp/BFL/flux-3-ae/ae.safetensors` | klein's VAE |
| `sd-cpp/Qwen/qwen-image-2.1/model-Q8_0.gguf` | qwen "full" DiT |
| `sd-cpp/Qwen/qwen-image-2.1-turbo/model-Q4_K_M.gguf` | qwen "turbo" DiT |
| `sd-cpp/madebyollin/texture-fix-vae-for-qwen-image-2.1/` | qwen's VAE |
| `sd-cpp/upscalers/` | hires upscalers (flat) |
| `gguf/BFL/flux-3-image-klein-pup/` | the PUP fine-tune: `model-Q8_0.gguf`, `model-Q4_K_M.gguf`, `mmproj-F16.gguf` |
| `gguf/Qwen/Qwen3-VL-8B-Instruct/` | in-engine conditioner for the qwen engine (`model-Q4_K_M.gguf`, `mmproj-F16.gguf`) |
| `gguf/Qwen/Qwen3-VL-4B-Instruct/` | in-engine conditioner for the klein engine (`model-Q4_K_M.gguf`, `mmproj-Q8_0.gguf`) |
| `gguf/zerofata/G4-MeroMero-v2-26B`, `…-31B` | the creative models (optional for the image stack) |

The PUP serves two jobs: the klein sidecar runs it at Q8_0 (9.5 GB), and the
qwen doors run it at Q8_0 for turbo and Q4_K_M for full — the full door takes
the lighter quant because its engine (13.6 GB) plus a Q8 sidecar exceeded the
24 GB card (measured OOM; the Q4 pair fits at ~19 GB).

Downloads: follow each directory's `provenance.md` on the reference host
(source, converter, hash). Verify sha256 after every download; a model is not
"downloaded" until its hash checks.

## 2. Engines (stock upstream, named slots)

Two pinned upstream checkouts under `backends/`; neither is modified.

**stable-diffusion.cpp** at a pinned master commit (one `sd-server` build
serves both image families):

```sh
# verified: reference host (task-002). Submodules populated by copying from a
# sibling checkout when git-lfs/network is awkward; a plain
# git clone --recurse-submodules works too.
git clone https://github.com/leejet/stable-diffusion.cpp backends/stable-diffusion.cpp
cd backends/stable-diffusion.cpp && git checkout 290148d
cmake -B build -DBUILD_SHARED_LIBS=OFF -DBUILD_KLEIN=ON -DGGML_SYCL=OFF \
      -DCUTLASS_DIR="$PWD/thirdparty/cutlass" \
      -DCMAKE_CUDA_ARCHITECTURES=86 -DCMAKE_CUDA_COMPILER=/usr/local/cuda-13.2/bin/nvcc
make -C build -j12 sd-server    # ~6 min on the reference host
```

Its `deploy/` tree (in the checkout, host-local) holds the serve/stop scripts
per family; the live deployments are symlinked from the gateway's deploy
directory. The exact engine invocations (verified):

- **klein** (`:39170`): `sd-server --listen-ip 127.0.0.1 --listen-port 39170
  --diffusion-model models/sd-cpp/BFL/flux-3-image-klein/model-Q8_0.gguf
  --llm models/gguf/Qwen/Qwen3-VL-4B-Instruct/model-Q4_K_M.gguf
  --llm_vision models/gguf/Qwen/Qwen3-VL-4B-Instruct/mmproj-Q8_0.gguf
  --vae models/sd-cpp/BFL/flux-3-ae/ae.safetensors
  --hires-upscalers-dir models/sd-cpp/upscalers
  --flash-attn on --sage-attn --eager-load --max-vram 0 --rng cuda`
- **qwen** (`:39160`, one process; the variant picks the DiT): turbo →
  `qwen-image-2.1-turbo/model-Q4_K_M.gguf`, full → `qwen-image-2.1/model-Q8_0.gguf`;
  conditioner `Qwen3-VL-8B-Instruct` (Q4_K_M + mmproj-F16); VAE the
  texture-fix one. Same flag set as klein.

The **klein serve script is a joint supervisor**: one process lifetime runs
the engine *and* its PUP sidecar together, so the pair loads and dies as one
(18–20 GB together) — the one thing the gateway cannot do natively (start
two processes from one request). It is a thin launcher: it adopts a healthy
pair, and its only GPU action is a last-resort zombie clear (it never stops
anything the router knows about — the router is the sole eviction authority,
see the gateway section).

**llama.cpp** (the sidecars' named build — `build/bin/llama-server` with CUDA
and vision):

```sh
# verified: reference host (May-21 tree, task-004). cmake Release,
# GGML_CUDA=ON, CUDA arch 86; the sidecars need vision (mmproj) and --jinja.
git clone https://github.com/ggml-org/llama.cpp backends/llama.cpp
cd backends/llama.cpp
cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=ON \
      -DCMAKE_CUDA_ARCHITECTURES=86
cmake --build build --config Release -j12 --target llama-server
```

The sidecars are plain `llama-server` processes (OpenAI-compatible, with
vision): klein's on `:39175` (PUP Q8_0 + mmproj-F16, `--jinja`,
`--ctx-size 8192`, `--n-gpu-layers 999`, thinking disabled via
`--chat-template-kwargs`), the qwen doors on `:39176` (turbo, Q8_0) and
`:39177` (full, Q4_K_M). The serve scripts check the model file exists, adopt
a healthy listener on their port instead of double-starting, and write pid
files.

## 3. Gateway (llama-swap, pinned)

**llama-swap v269** (tag `9fbc47c`), pristine upstream, built from its Makefile:

```sh
# verified: reference host (task-001)
git clone https://github.com/mostlygeek/llama-swap backends/llama-swap
cd backends/llama-swap && git checkout v269 && make   # -> build/llama-swap-<goos>-<goarch>
```

The deployment lives in a **non-git** directory (`backends/llama-swap-deploy/`):

```
config.yaml            the live config (below); versions: config.yaml.<name>
llama-swap             the built binary
vllm-serve.sh …        one serve/stop script per model, referenced by cmd/cmdStop
<engines'/ deploy dirs> symlinked serve/stop scripts
```

Run it as a **systemd user unit** (`ml-llama-swap`) with
`-config config.yaml -watch-config`: the 2-second config poll makes live
config swaps work without a restart — with the sharp edge that **every reload
kills running model processes** (see Operations).

`config.yaml`: one entry per model — `proxy` (the real port), `cmd`/`cmdStop`
(the serve/stop scripts), `ttl` (idle unload; 0 = never; the kokon models use
120 s, the creative ones 300 s) — and the **routing block** that decides what
may co-reside and what a new request evicts. This deployment runs the **matrix
router** (v269), a superset of the older `groups` router:

```yaml
routing:
  router:
    use: matrix
    settings:
      matrix:
        vars:
          k: kokon-klein        p: kokon-pup
          qt: kokon-turbo       pq: kokon-pup-qwen
          qf: kokon-full        pqf: kokon-pup-qwen-f
          v: qwen3.8-27b
          cf: creative-fast     cs: creative-slow
        sets:
          klein:  "k & p"
          qwen-t: "qt & pq"
          qwen-f: "qf & pqf"
          small:  "cf | cs"
```

Three rules: a **set** declares which models may co-reside; it never starts
anything. A request for a model **keeps every running model that sits in some
set containing it and evicts the rest** (cheapest first, `evict_costs`, default
1). A model that is a **leaf of no set at all** is an *orphan*: a request for
it evicts *everything* running. Here **vLLM is deliberately the only orphan** —
a request for it reclaims the whole card, and (the part that matters) the
daemon's **FIFO scheduler** (in stock since v225; the default, no config
needed) **defers that request while any of them is still handling one** —
journal: `queuing request for model … (would evict in-flight process)` — and
retries on every completion. An in-flight request is never interrupted; the
evictor waits. The sets:

- `klein: k & p` — the klein engine plus its PUP sidecar. A klein request
  evicts everything else, vLLM included: the card holds one heavy job, the
  job runs, and the chat model comes back when a request for it reclaims the
  card (warm boot ~60–90 s).
- `qwen-t` / `qwen-f` — each qwen flavor is its engine plus its PUP door. The
  two flavors never mix (different engine variants share one port anyway),
  and each evicts everything else when requested.
- `small: cf | cs` — one creative writer; the two writers never share the
  card (26B + 31B > 24 GB), and a creative request evicts vLLM and any image
  pair too.

The router is the **sole eviction authority**: it stops what it evicts via
each model's `cmdStop`, and the serve scripts no longer kill anything on
their own — their only GPU action is a last-resort clear for out-of-state
zombies that never touches vLLM. The deploy directory keeps the full
write-up (router + scheduler + the wrapper contract + the live-change
protocol); the upstream docs
(`docs/kb/guides/routing/groups-and-matrix.md`) describe the semantics
generically.

Before any live use: `./llama-swap -config config.yaml -validate` (validates
and exits).

## 4. The service (this repository)

Build the image (the two-stage `docker/Dockerfile` builds the UI, then
`kokon-service` release; the engine and models are **not** in the image):

```sh
# verified: reference host. Tag convention: external-engine-<version>-<commit>.
git clone <this repo> kokon && cd kokon   # or work in the existing clone
docker build -f docker/Dockerfile \
  -t ghcr.io/<org>/kokon:external-engine-0.11.2-<short-commit> .
```

Deploy it with a compose file (the live one is `frontends/kokon/compose.yaml`
on the reference host):

```yaml
services:
  kokon:
    image: ghcr.io/<org>/kokon:external-engine-0.11.2-<short-commit>
    container_name: kokon
    restart: unless-stopped
    network_mode: host          # the service binds 127.0.0.1 and drops
                                # foreign source addresses: host mode is deliberate
    volumes:
      - kokon-data:/data        # service-token, asset bundles, logs
      - kokon-config:/config    # settings (config.json)
      - kokon-gallery:/gallery  # saved images
    deploy:
      resources:
        reservations:
          devices: [{driver: nvidia, count: all, capabilities: [gpu]}]
```

Ports: **39152** web UI (password), **39151** token API (host loopback; the
MCP server reads the token from `/data/service-token` in the container).

The container's `/config/config.json` (schema 13) is the external-engine
configuration: the service's `externalEngine` points at the **gateway**
(alias, not port), and each pipeline names its engine alias and its VLM alias
plus the prompt bundle the VLM applies:

- engine aliases: `kokon-klein`, `kokon-turbo`, `kokon-full`
- VLM aliases: `kokon-pup` (klein) and `kokon-pup-qwen` / `-f` (qwen turbo /
  full) — all PUP doors run the fine-tune; the quants differ per door
- prompt bundles: the committed asset bundles under
  `crates/qls-core/assets/` (`pup`, `pup-qwen`); the service copies them into
  the container's `/data/assets/` and reads them per job, so a dropped-in
  replacement applies to the next job

A down sidecar **hard-fails the job** (no silent raw-prompt fallback): the
VLM is part of the quality contract, not an optional garnish.

## 5. Verification (fresh box)

1. **Gateway**: `curl 127.0.0.1:8081` answers; `journalctl --user -u
   ml-llama-swap` shows the config load.
2. **One-token probes** (the readiness check everywhere in this stack — `/health`
   lies while models load; a 1-token completion does not): the chat model via
   the gateway (`/upstream/<alias>/v1/completions`), each sidecar's port
   directly, `/sdcpp/v1/capabilities` on each engine port after a request has
   started it.
3. **One job per family** through the service UI (39152): a klein edit and a
   qwen text-to-image. Both must land in the gallery, and the journal must
   show the matrix decisions, e.g.
   `matrix: model=kokon-turbo set=qwen-t … evict=[…] … cost=N` — that line is
   the proof the matrix router (not the groups fallback) is what decided.
   The scheduler's own lines are Debug-level: `logLevel: debug` in a
   candidate config shows the deferral behavior
   (`queuing request for model … (would evict in-flight process)`).
4. **Idle unload**: after 120 s without requests the pair's ports go quiet
   (TTL); the next request reloads it.

## 6. Operations

- **A config swap kills every running model** (the watcher reload tears down
  the old server; nothing auto-revives anything — the daemon is pure
  on-demand). Plan live config changes as a *window*: swap, revive what the
  operator runtime needs, write a DONE/FAIL marker **last**, and make the
  write atomic (`cp … .new && mv -f`) because the 2-second poll can read a
  half-written file.
- **The chat model's serve script** (`vllm-serve.sh`) has a branch for
  "port held or a fresh engine core, but not yet healthy": it waits for the
  in-flight boot instead of evicting and relaunching (a 2026-11-04 incident:
  a wrapper parked in an eviction wait relaunched a model the router had just
  evicted, resurrecting it before its targets loaded).
- **The wrappers are thin.** The serve scripts adopt a healthy co-resident
  and otherwise only run the last-resort zombie clear (never vLLM). There is
  no policy file any more: the sets above are the policy, and the scheduler
  makes every conflict a deferrable, in-flight-safe eviction.
- **Rollback ladder** (in the deploy directory): the current config → the
  previous versioned config (`config.yaml.<previous>`) → the engine-rollback
  notes cover stock ↔ the older build. If a rollback crosses the point where
  the wrapper scripts changed generation (a set that contains vLLM needs the
  older, gate-era scripts), restore both the config and the script snapshot
  together — the deploy directory keeps the pre-change scripts.
- **A window is a script, not a habit**: self-deadline, a separate watcher
  process as backstop, and the marker written only after the box is back to
  its known-good state.
