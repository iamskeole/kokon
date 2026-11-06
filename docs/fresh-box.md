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
llama-swap gateway (stock, pinned) on 0.0.0.0:8081
        │  the box's ONLY LAN door for model traffic — keyed (one 48-char
        │  box key; /health is the only open path); starts / evicts on
        │  demand; TTL unloads; the only thing that ever stops a model
        │  process (its default process-group signal)
        ▼
engines, all on daemon-assigned 127.0.0.1 ports (${PORT}, from 9100 —
  assigned by sorted model ID, so there is no port table to memorize):
  sd-server (stable-diffusion.cpp, pinned)     the qwen engines + the klein engine
  llama-server (llama.cpp, pinned)             the shared PUP door (all three
                                               image families) + the two creative writers
  vLLM (owner's chat model; two variants, one at a time —
                                               set vllm: "v | vv")  text: qwen3.8-27b · vision: qwen3.8-27b-vision
```

The service speaks only to the **gateway**, never to engines directly: each
pipeline in its config names a gateway alias for its engine and another for
its VLM sidecar (all three image families point their VLM at the same shared
PUP door — task-019). The gateway is the only component that ever starts a
process, and since the 2026-11-05 re-land the only one that ever stops one.
Since the 2026-11-06 one-door cutover the gateway is **keyed** with one
48-char box key: the service presents it on every call
(`externalEngine.apiKey`; a VLM entrypoint that names no key of its own
inherits it), and so does every other client of the box (the owner's phone,
MCP) — a call without the key is a 401, except `/health`. There are no fixed
engine ports anywhere: the daemon assigns every engine a loopback number at
config load and proxies to it (the contract is in the gateway section).
## Host layout

Everything deployment-side lives under one root (reference host: `/home/p/ml`),
outside the repository (non-git):

```
ml/backends/     llama.cpp/             pinned, pure upstream — one shared
                                        build (build/bin/llama-server,
                                        shared-linked: the libggml-*.so sit
                                        beside the binaries)
                 stable-diffusion.cpp/  pinned, pure upstream (290148d);
                                        one sd-server build serves both
                                        image families
                 llama-swap/            pinned, pure upstream (v269)
                 hyperqwen-032/         pinned, pure upstream — the vLLM
                                        runtime (a project venv inside)
                 _scripts/              deployment space: the pure boot
                                        scripts (one per gateway model)
                 _configs/              deployment space: llama-swap.yaml
                                        (the config home) + llama-swap.yaml.last-good +
                                        gateway.key (the box key, mode 600 —
                                        never in git, never echoed) +
                                        adding-a-model.md
                 _jobs/                 the window tooling (verification
                                        windows, the daemon-restart job)
ml/frontends/    kokon/                 the service deployment: compose.yaml (+README)
                 openwebui/             the LAN door (accounts)
ml/models/       gguf/   {org}/{Model}/model-<QUANT>.gguf · mmproj-<PRECISION>.gguf
                 sd-cpp/ {org}/{Model}/  engine DiTs + VAEs; upscalers/ stays flat
                 safetensors/  the vLLM weights
ml/__new_mess/   parked cold history: the old wrapper generation (the former
                 llama-swap-deploy/ and sdcpp deploy/ trees, the old OPS
                 notes) — never loaded, never referenced by the live system
ml/__backups/    parked backups (the old bespoke vLLM runtime, out of service)
```

**The stock-tree purity rule:** every non-underscore directory under
`backends/` is a pristine upstream checkout — no deployment scripts, no
configs, no logs, no pid files inside. Everything deployment-side lives in
the underscore directories. (The old layout had a `llama-swap-deploy/`
sibling and per-engine `deploy/` trees *inside* the checkouts; the 2026-11-05
re-land moved all of that out and parked the old generation cold.)

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

The gateway's vision entry, **`qwen3.8-27b-vision`**, adds **no new weights**:
it is the owner's chat-model checkpoint served with its vision tower enabled
(same directory, same quant, same download — the provenance is the one the
chat-model runtime's `provenance.md` already records; only the launch differs:
no `--language-model-only`, a shorter context, and the second served name).
It has its own assigned port, but the two share the card: the set
`vllm: "v | vv"` (§3) keeps them mutually exclusive — one process at a
time, a request for either evicting the other.

The PUP fine-tune runs as **one shared sidecar door** (task-019) at Q4_K_M —
the only quant that fits all three image pairs in 24 GB (the Q8 full door's
OOM, measured 2026-11-03, is the constraint) — serving all three families
under two names (the klein entry's name and the `qwen3vl-8b` alias the qwen
entries use), so a family or flavor switch reloads only the engine, never the
sidecar.

Downloads: follow each directory's `provenance.md` on the reference host
(source, converter, hash). Verify sha256 after every download; a model is not
"downloaded" until its hash checks.

## 2. Engines (stock upstream, named slots)

Four pinned upstream checkouts under `backends/`; **none is modified** (the
purity rule above). The boot scripts in `backends/_scripts/` are the only
thing between the gateway's models and these trees; **every one of them
takes its port as `$1`** — the daemon hands it the number `${PORT}` assigned
at config load (the gateway section), so no script and no config line ever
names a port.

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

The exact engine invocations (verified; the `serve-kokon-*.sh` scripts
preflight their model files and `exec` this binary — the daemon's tracked
process *is* the engine):

- **klein** (port from `$1`): `sd-server --listen-ip 127.0.0.1 --listen-port "$1"
  --diffusion-model models/sd-cpp/BFL/flux-3-image-klein/model-Q8_0.gguf
  --llm models/gguf/Qwen/Qwen3-VL-4B-Instruct/model-Q4_K_M.gguf
  --llm_vision models/gguf/Qwen/Qwen3-VL-4B-Instruct/mmproj-Q8_0.gguf
  --vae models/sd-cpp/BFL/flux-3-ae/ae.safetensors
  --hires-upscalers-dir models/sd-cpp/upscalers
  --flash-attn on --sage-attn --eager-load --max-vram 0 --rng cuda`
- **qwen** (port from `$1`; one process; the variant picks the DiT): turbo →
  `qwen-image-2.1-turbo/model-Q4_K_M.gguf`, full → `qwen-image-2.1/model-Q8_0.gguf`;
  conditioner `Qwen3-VL-8B-Instruct` (Q4_K_M + mmproj-F16); VAE the
  texture-fix one. Same flag set as klein.

**llama.cpp** — one shared build serving the PUP sidecar and the creative
writers (`build/bin/llama-server`). The reference host's build is
**shared-linked** (the `libggml-*.so` live in the build's bin directory,
beside the binaries), so every boot script exports
`LD_LIBRARY_PATH` at that directory — the binary's baked RUNPATH predates
the tree's location in `backends/`:

```sh
# verified: reference host (Oct-14 tree, commit 529ac893e). cmake Release,
# GGML_CUDA=ON, CUDA arch 86, shared libs; the PUP needs vision (mmproj)
# and --jinja.
git clone https://github.com/ggml-org/llama.cpp backends/llama.cpp
cd backends/llama.cpp && git checkout 529ac893e
cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=ON \
      -DBUILD_SHARED_LIBS=ON -DCMAKE_CUDA_ARCHITECTURES=86
cmake --build build --config Release -j12 --target llama-server
```

The PUP sidecar is a plain `llama-server` (OpenAI-compatible, with vision).
Since task-019 it is **one shared door** (`serve-kokon-pup.sh`): the BFL PUP
fine-tune at Q4_K_M + mmproj-F16 on its `$1` port (daemon-assigned), `--ctx-size 8192`, thinking
off (`--jinja --chat-template-kwargs '{"enable_thinking":false}'`), image
clamps `--image-min-tokens 1024 --image-max-tokens 4096` (the processor is
patch-16/merge-2 — one visual token per 32×32 px; the fine-tune's mmproj
metadata carries no pixel limits, so these flags set the window), default
sampling = the base model's official non-thinking general row, and two
`--alias` names so the single process answers both client families. The
creative writers (`serve-g4-meromero-v2-26b.sh` / `-31b.sh`) are the same
binary, each taking its daemon-assigned port as `$1` (9100/9101 on this box
— the alphabetical contract in the gateway section).

**hyperqwen-032** — the owner's chat-model runtime: a stock upstream
vLLM-based tree with a project venv (reference host: python 3.14). The two
vLLM boot scripts (`serve-qwen3.8-27b-text.sh` / `-vision.sh`) `exec`
`venv/bin/vllm serve` with **the owner's final calibration** — the scripts
are the record of those parameters; this doc deliberately does not restate
them, and nothing in this stack re-tunes them.
## 3. Gateway (llama-swap, pinned)

**llama-swap v269** (tag `9fbc47c`), pristine upstream, built from its
Makefile:

```sh
# verified: reference host (task-001)
git clone https://github.com/mostlygeek/llama-swap backends/llama-swap
cd backends/llama-swap && git checkout v269 && make   # -> build/llama-swap-<goos>-<goarch>
```

The deployment lives in the two underscore directories (purity rule):
`_configs/` — `llama-swap.yaml` (the config home),
`llama-swap.yaml.last-good`, `gateway.key`, `adding-a-model.md` — and
`_scripts/` (the boot scripts). The daemon runs as
a **systemd user unit** (`ml-llama-swap`):

```
ExecStart=…/backends/llama-swap/build/llama-swap-linux-amd64 \
          -config …/backends/_configs/llama-swap.yaml -listen 0.0.0.0:8081 -watch-config
```

The wildcard bind is deliberate: after the one-door cutover (2026-11-06)
this port is the box's **only** LAN door for model traffic (the phone and
any other device reach the models through it; the kokon UI on `:39152` and
the owner's phone proxy on `:18021` are the only other exposed listeners,
and they route onward to the gateway). Every model-side listener stays
`127.0.0.1`-only.

**Keyed: one admission key.** The config carries a top-level `apiKeys:`
list — one 48-char box key on this box; the key's home is
`_configs/gateway.key` (mode 600, never in git, never echoed). Every
model-routed path demands it — `/v1/models`, `/upstream/*`, `/running`,
`/unload`, the UI and the `/logs` rings (`Authorization: Bearer <key>`,
Basic or `x-api-key` also accepted) — while `/health` (and `/wol-health`)
stay open. The key is a **door ticket, not an engine secret**: the
reverse proxy forwards the client's `Authorization` header verbatim to
engines that have no auth of their own (llama-server, sd-server, and this
box's keyless vLLM), so the key is inert past the gateway. The daemon
builds its auth middleware from the config on every load, so a key change
takes effect on the next hot reload — but a reload is a whole-box engine
reset (below), so a key change is a window like any other config change.
Every client of the box holds a copy of the key: the service
(`externalEngine.apiKey` in its volume config; a VLM entrypoint with no
key of its own inherits it), the owner's phone, MCP.

**Pure boots, no `cmdStop`.** Every `cmd` is a small POSIX script that
preflights (model files present, its port free) and `exec`s the engine: the
daemon's *tracked* process **is** the engine, nothing else. v269 starts
every cmd in its own process group (`Setpgid`), so the daemon's **default
stop** — SIGTERM to the group, SIGKILL after the model's `unloadTimeout` —
reaches the engine's children (vLLM's `EngineCore` included) and is the
**only** unload path on this box. Measured in the 2026-11-05 re-land window:
vLLM 4 s fired→exit, llama.cpp 2 s, VRAM back to ~1 MiB in both; the
`unloadTimeout`s (240 s vLLM, 120 s the rest) are backstops, not the observed
behavior. No script on this box kills or evicts anything the router tracks —
**the router is the sole eviction authority**.

`llama-swap.yaml`: `startPort: 9100` (the base of the **port assignment for
every** model — the contract below), `healthCheckTimeout: 1200` (a cold
20-GB-plus load can take that long — vLLM once took ~16 min),
`globalTTL: 0` (per-model TTLs decide — 600 s the kokon models, 300 s the
writers), `logLevel: info`, `logToStdout: both`, the `apiKeys:` block
above, and one entry per model (`proxy`, `cmd`, `checkEndpoint`, `ttl`,
`unloadTimeout`, capabilities) — **every `cmd` and every `proxy` uses the
`${PORT}` macro, so the config names no port at all**:

| model | engine | port | in a set |
|---|---|---|---|
| `creative-fast` | llama.cpp (G4 26B, MTP) | 9100 | `small` |
| `creative-slow` | llama.cpp (G4 31B, MTP) | 9101 | `small` |
| `kokon-full` | stock sd-server (Qwen-Image 2.1 full) | 9102 | `qwen-f` |
| `kokon-klein` | stock sd-server (FLUX.3 Klein) | 9103 | `klein` |
| `kokon-pup` | llama-server (the shared PUP door, two names) | 9104 | **all three image sets** |
| `kokon-turbo` | stock sd-server (Qwen-Image 2.1 Turbo) | 9105 | `qwen-t` |
| `qwen3.8-27b` | vLLM (text) | 9106 | `vllm` |
| `qwen3.8-27b-vision` | vLLM (vision tower on; own port, second served name) | 9107 | `vllm` |

**The port contract (a v269 property — nothing to memorize).** The daemon
assigns the next number from `startPort` to the models **with their IDs
sorted alphabetically** at config parse (`macros.go` collects the map keys
and `sort.Strings` them) — so the table above is the whole story, it is
deterministic per config version, and neither the config nor any script
carries a port. Adding a model whose ID sorts *before* an existing one
shifts that existing model's number — harmless: only the gateway knows the
map, a reload re-boots on the new number, and the journal prints the number
actually in use on every boot (`… Health check passed on
http://127.0.0.1:9103/…`).

and the **routing block** (the matrix router, v269):

```yaml
routing:
  router:
    use: matrix
    settings:
      matrix:
        vars:
          k: kokon-klein
          p: kokon-pup          # the ONE shared PUP door — member of all
          qt: kokon-turbo       # three image sets (task-019): a family or
          qf: kokon-full        # flavor switch reloads only the engine,
          v: qwen3.8-27b        # never the sidecar
          vv: qwen3.8-27b-vision
          cf: creative-fast
          cs: creative-slow
        sets:
          klein:  "k & p"
          qwen-t: "qt & p"
          qwen-f: "qf & p"
          small:  "cf | cs"
          vllm:   "v | vv"      # the two vLLM variants never coexist
```

Three rules: a **set** declares which models may co-reside; it never starts
anything (a request starts *only* the requested model — the klein pair forms
from the kokon job's two gateway calls, VLM request then image request, and
stays co-resident because both are in `klein`). A request for a model **keeps
every running model that sits in some set containing it and evicts the rest**
(cheapest first, `evict_costs`, default 1). A model that is a **leaf of no
set at all** is an *orphan*: a request for it evicts *everything* running —
and since the one-door cutover **no model on this box is an orphan**: the
vLLM variants, which used to be (the pre-cutover shared-port orphan
convention), now sit in their own set, **`vllm: "v | vv"`**. That set is a
disjunction the matrix solver can never satisfy with both sides (a state
carries one value per variable), so the two variants — 24 GB each, one card
— **never coexist even though they now have distinct ports**: a request for
either first stops the resident other with the daemon's group signal and
reclaims the whole card, and (the part that matters) the daemon's **FIFO
scheduler** (in stock since v225; the default, no config needed) **defers
that request while any evict target is still handling one** — journal (debug
level): `queuing request for model … (would evict in-flight process)` — and
re-evaluates on every completion. An in-flight request is never interrupted;
the evictor waits. (A disjunction like this is the general tool for any
never-coexist pair; the `small` set is the other instance of it.) The sets:

- `klein: k & p` — the klein engine plus the shared PUP door. A klein
  request evicts everything else, vLLM included: the card holds one heavy
  job, the job runs, and the chat model comes back when a request for it
  reclaims the card (warm boot ~60–90 s).
- `qwen-t` / `qwen-f` — each qwen flavor is its engine plus the *same*
  shared door. The two flavors never mix (their engine variants share one
  port anyway), and each evicts everything else **except the door** when
  requested — the flavor switch is an engine-only reload.
- `small: cf | cs` — one creative writer; the two writers never share the
  card (26B + 31B > 24 GB), and a creative request evicts vLLM and any image
  pair too.
- `vllm: v | vv` — the two vLLM variants, one process per card. The text
  variant is what the box chat runs on; the vision variant is available for
  vision work on demand, at the cost of a full chat-model restart
  (~60–90 s warm). Their identity is the `--served-model-name` in the
  process argv, never the port (the boot scripts are model-aware).

**Hot reload and the last-good copy.** `-watch-config` polls the file every 2 s.
A **successful** edit is a hot reload: the daemon atomically swaps in a new
server built from the new config and shuts the old one down — **that stops
every loaded model** (vLLM included), and nothing auto-revives until the
next request. Two footguns, both observed on this box (task-022): the reset
is **whole-box even when the edited model isn't loaded** — an in-flight
stream gets one 200-with-empty-body and the client retries — and the daemon
**never re-seeds `.last-good` itself** on a reload; that file is manually
maintained. So: batch config changes into one edit; a window that changes
the config owns the whole sequence (swap atomically → wait for the reload →
re-seed `.last-good` → re-boot what the box needs → **marker last**). A
**failed** parse is ignored and the old config stays live.
`llama-swap.yaml.last-good` is a byte copy of the last
validated-and-deployed config: after every deliberate change
`cp llama-swap.yaml llama-swap.yaml.last-good`, and a window asserts the two
md5s match at start and end. Rollback = restore the last-good copy (the
2-s poll reloads it; loaded models stop, then re-boot on demand).

**Logs in the journal.** `logToStdout: both` copies every log stream (proxy,
upstream — every engine's own stdout/stderr — and http) to the daemon's
stdout, which is the systemd user journal:

```sh
journalctl --user -u ml-llama-swap -f     # follow everything
```

The `:8081/logs` ring buffers (100 KB per stream) keep working regardless
(behind the key, like the rest of the API). A daemon restart (the
`_jobs/restart-daemon.sh` shape: explicit UNLOAD of whatever is loaded,
`systemctl --user restart`, then a **handover gate** — old process gone +
the new pid owns `:8081` + `/health` 200 — before any request is sent: a
request sent during the bind handover can land on the dying old listener
and come back empty) is needed only when the daemon binary itself changes;
config *and key* changes take effect on the plain hot reload.

**Manual boots (the one sanctioned out-of-band path).** The daemon can see
and stop only the processes it started. Anything booted outside it is
invisible to the router — and since the one-door cutover there is no other
door: a manual boot holds the card, gateway requests for another model queue
behind it (FIFO deferral) or fail, and the gateway **never** kills a manual
boot. That is by design: a manual boot is a **human reclaiming the card**,
and only the human decides when it stops (the pre-cutover convention — "go
talk to the manual port directly" — died with the fixed ports; the gateway
is now the only front). The boot scripts' **adopt** branch still matters in
one place: after a daemon restart (or a config reload that re-resolved
ports) a healthy engine the daemon no longer tracks may be sitting on its
*same assigned port* (the number is deterministic per config version); a
request for it re-attaches to the surviving process instead of
double-launching.

Before any live use: `…/llama-swap-linux-amd64 -config …/llama-swap.yaml
-validate` (validates and exits: "N model(s), 0 peer(s)").
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
(`http://127.0.0.1:8081/upstream` — alias, not port) and **presents the box
key** (`externalEngine.apiKey`; a VLM entrypoint that names no key of its own
inherits it — the service's gateway client and all its VLM entrypoints sit
behind the same keyed door), and each pipeline names its engine alias, its
VLM entrypoint and the prompt bundle the VLM applies:

- engine aliases: `kokon-klein`, `kokon-turbo`, `kokon-full`
- VLM: the single shared door `kokon-pup` (one llama-server process,
  Q4_K_M, two `--alias` names): the klein entry speaks the alias `q35`, the
  qwen entries `qwen3vl-8b` — a family or flavor switch reloads only the
  engine, never the door (task-019)
- prompt bundles: the committed asset bundles under
  `crates/qls-core/assets/` (`pup`, `pup-qwen`); the service copies them into
  the container's `/data/assets/` and reads them per job, so a dropped-in
  replacement applies to the next job

A down sidecar **hard-fails the job** (no silent raw-prompt fallback): the
VLM is part of the quality contract, not an optional garnish.

## 5. Verification (fresh box)

1. **Gateway**: `curl 127.0.0.1:8081/health` is 200 (the one open path);
   **the keying**: `curl 127.0.0.1:8081/v1/models` is 401 with no key and
   200 with `Authorization: Bearer <the box key>`; `journalctl --user -u
   ml-llama-swap` shows the config load; `GET /running` (keyed) lists
   in-state models (state `"ready"` once the model's health check passes).
2. **One-token probes** — the readiness check everywhere in this stack
   (`/health` lies while models load; a 1-token completion does not):
   `GET /upstream/<model>/<path>` **pins** the model and **holds** through
   its load (the request blocks until the health check passes — the canary
   pattern; the job scripts use long timeouts for this reason); a 1-token
   `POST /v1/completions` per llama.cpp sidecar — the BFL klein hybrid is
   the exception: it has **no** `/v1/completions` (its LLM loads only as
   the prompt conditioner; a clean 404 is the healthy answer), and its
   readiness proof is a real image job: `POST /upstream/kokon-klein/sdcpp/
   v1/img_gen` → poll `GET …/sdcpp/v1/jobs/<id>` → a PNG back; `GET
   /upstream/<model>/sdcpp/v1/capabilities` on each sd-server after a
   request has started it; `POST /api/models/unload/<model>` is the explicit
   stop (it blocks until the process actually exits — the number of seconds
   it took *is* the stop-propagation measurement).
3. **One job per family** through the service UI (39152): a klein edit and a
   qwen text-to-image. Both must land in the gallery, and the journal must
   show the matrix decisions at info level, e.g.
   `matrix: model=kokon-turbo set=qwen-t dsl="qt & p" evict=[kokon-klein]
   target=[kokon-pup kokon-turbo] cost=2` — that line is the proof the
   matrix router (not the groups fallback) is what decided, and `… Health
   check passed on http://127.0.0.1:<port>/…` marks each boot. The
   scheduler's own deferral lines are Debug-level: `logLevel: debug` in a
   *candidate* config (never the live one) shows `queuing request for model
   … (would evict in-flight process)`.
4. **Idle unload**: after 600 s (kokon models) / 300 s (creative writers)
   without requests the assigned ports go quiet (TTL); the next request
   re-boots on the same number (warm, page-cached).
5. **The shared door** (task-019): `GET /upstream/kokon-pup/v1/models` lists
   both alias names; a klein job and a qwen job back-to-back keep the
   **same** sidecar pid resident — the journal shows `evict=[kokon-klein]`
   and `evict=[kokon-turbo]` (the engines only), never the door.
## 6. Operations

- **A config swap stops every loaded model** (a successful reload shuts the
  old server down; nothing auto-revives anything — the daemon is pure
  on-demand). The reset is **whole-box even when the edited model isn't
  loaded** (every loaded engine stops; an in-flight stream gets one
  200-with-empty-body and the client retries), and the daemon **never
  re-seeds `llama-swap.yaml.last-good` itself** — which is why a window
  does. Plan live config changes as a *window*: batch the changes into one
  edit, swap atomically (`cp … .new && mv -f` — the 2-second poll can read a
  half-written file), re-seed `llama-swap.yaml.last-good`, wait for the
  `configuration reloaded` journal line, re-boot what the box needs, and
  write the DONE/FAIL marker **last** (the marker means "the runtime is
  back"). The box's windows live in `backends/_jobs/`: a setsid'd job
  script (self-deadline), a second setsid'd backstop watcher (hard cap;
  fallback digest + best-effort restore + forced FAIL), and `wait-marker.sh`
  as the blocking wait.
- **The vLLM boot scripts are model-aware** (the owner's final calibration —
  the scripts are the record of the parameters): "is this process *my*
  model?" is answered by the served name in the process argv, never by the
  port (the two variants have distinct assigned ports now, but the check is
  the same — the number moves with config versions, the served name
doesn't). Each has an **adopt** branch (a healthy instance of *this* served
  name is attached to as the tracked process, not relaunched) and a **wait**
  branch (a *foreign* holder, or a port held by an in-flight boot that isn't
  healthy yet, is waited on, never killed out of band — the script never
double-launches an in-flight boot). They `exec` vllm; they kill nothing.
- **No out-of-band kills anywhere.** No script on this box stops a process
  the router tracks; there are no zombie nets. An out-of-state process (a
  manual boot) is a human reclaiming the card — the manual-boot note in the
  gateway section is the policy.
- **Rollback**: restore `llama-swap.yaml.last-good` over `llama-swap.yaml`
  (atomic mv; the 2-s poll reloads it). The pre-re-land wrapper generation is parked cold in
  `__new_mess/` if an old-layout state is ever needed — restoring it means
  restoring the whole old layout, not a one-file swap.
- **The journal is the ops log**: `journalctl --user -u ml-llama-swap -f`
  (everything, because `logToStdout: both`); grep `matrix:` for routing
  decisions, `Health check passed` for readiness, `reloading configuration`
  / `configuration reloaded` for reloads, and the engine's own lines for
  load/exit. The `:8081` UI and its `/logs` rings show the same at 100 KB
  depth.
- **A window is a script, not a habit**: self-deadline, a separate backstop
  watcher, and the marker written only after the box is back to its
  known-good state.

