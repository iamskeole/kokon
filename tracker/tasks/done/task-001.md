# task-001 — llama-swap: pin v269, deploy artifacts to the sibling

## goal
`backends/llama-swap` = 100% clean upstream at a pinned tag (current upstream
head is **v269**; the tree is at v267, 0 ahead / 17 behind). All of our
deploy artifacts — currently untracked files inside the repo tree
(`config.yaml` + `.bak`, `creative-fast.sh`, `creative-slow.sh`,
`evict-gpu.sh`, `vllm-serve.sh`, `serve.sh`, the test scripts, `state/`,
`logs/`, the built `llama-swap` binary) — move to the non-git sibling
`backends/llama-swap-deploy/`. The gateway is rebuilt from the pinned tree
and comes back up with every managed model working.

## acceptance criteria (met 2026-11-02, agent — evidence below)
- [x] `git status` in `backends/llama-swap` is clean at the upstream v269 tag
      (`9fbc47c`; `git describe` = v269; zero untracked files).
- [x] Deploy artifacts live in `backends/llama-swap-deploy/` (non-git) and are
      referenced correctly; a repo-wide + host-wide grep for old
      `backends/llama-swap/` references (excl. `__mess/` archive) finds none.
- [x] Binary rebuilt from the pinned tree (`make` from v269 source; reports
      `version: v269 (9fbc47c)`), installed in the deploy dir, running under the
      updated `ml-llama-swap.service` user unit, serving `:8081`.
- [x] On-demand load verified for every managed alias, in the gateway, on the
      new binary:
      - `creative-fast` — OK; 1-token completion via gateway (round 1)
      - `creative-slow` — OK; 1-token completion via gateway (round 1)
      - `kokon-turbo` — OK; `/sdcpp/v1/capabilities` 200 in 10s (round 2)
      - `kokon-full` — OK; capabilities 200 in 160s incl. turbo→full DIT swap
        (round 2)
      - `kokon-klein` — OK; capabilities 200 in 9–10s, 17.4GB on GPU (round 3
        + standalone replay; the engine is a genuine klein build: auto-fit
        shows DiT 5.87GB + conditioner 12.2GB incl. the 8B PUP)
      - `qwen3.8-27b` — OK; loaded several times (50s warm / 16min cold
        page-cache boot); **gold-standard 1-token `/v1/completions` through the
        gateway with the vLLM API key → 200** (final check, 2026-11-02 01:10)
- [x] vllm healthy at the end (`/running`: `qwen3.8-27b ready`); the agent's
      own runtime (served through this gateway) came back after every window
      it was part of — the autonomy protocol's failure modes (poll loops,
      false-DONE, stranded GPU, premature markers) did not materialize.

## blockers
- One gateway-restart window (vllm down during it; standard protocol).
- Our `config.yaml` was written for v267 — validate it against v269's
  `config-schema.json` / `config.example.yaml` before rebuilding (17
  commits moved; schema drift is the main risk).

## decisions
- Sibling dir name = `backends/llama-swap-deploy/` (confirm at execution).
- Pin = v269 (current upstream head at extraction time).

### Decisions made at execution (2026-11-01, agent)
- **D-001a** — Confirmed `backends/llama-swap-deploy/` at execution; holds config,
  scripts, the rebuilt binary, `logs/`, `state/`, the flip rollback bundle, the
  test scripts, and `README.local.md`. The gateway tree stays 100% upstream
  (main ff'd to v269; `git status` clean; build output is gitignored).
- **D-001b** — The 11-01 "mess" cleanup had archived the four live engine
  scripts (`sd-cpp-serve.sh`, `sd-cpp-stop.sh`, `sd-cpp-serve-klein.sh`,
  `sd-cpp-stop-klein.sh`) into `__mess/` while `config.yaml` still points at
  `/home/p/ml/backends/` — i.e. every engine alias had a dangling `cmd`. That
  contradicted both plans' "live exceptions: the serve/stop scripts" rule. Fix:
  restore live copies to `/home/p/ml/backends/` (archive copies stay in
  `__mess/`, intact). The live scripts reference the fork build trees where
  they actually live now (`__mess/sd.cpp`: `engine/` = qwen build 0a06712+,
  `klein-engine/` = 1814814+PUP, rebuilt 11-01 03:58) — explicit exception:
  those fork builds are LIVE until the WP2/WP3 stock cutovers, they just no
  longer live under `backends/`.
- **D-001c** — `evict-gpu.sh`'s kokon job-check pointed at the retired
  `backends/kokon/mcp-data/service.json`; now points at the live location
  `__mess/kokon/mcp-data/service.json` (graceful no-op if absent, as before).
- **D-001d** — Creative models keep the `latest/llama.cpp` (Oct-14, shared-lib
  build) llama-server the scripts already use; verified a genuine ELF with
  its `.so`s beside it. The job smoke-tests both creative models (per the
  old plan's "creative gets a smoke test" note). The named-slot May-21 tree
  is untouched.
- **D-001e** — The v267 repo-built binary is kept as `llama-swap.v267.bak`
  in the deploy dir (rollback of the daemon only; a full layout rollback is
  not attempted — config validated against v269's schema first).
- **D-001f** — v267→v269 schema diff audited: only additive optional fields
  (`store.persistCaptures`, `capabilities.embeddings`); none are used by our
  config. v268 adds `/api/models/pause|resume` (unused). No config migration
  needed.

### Findings from the restart window (2026-11-02, agent) — input for task-010
- **v269 `/upstream/<m>/...` does not block on the load.** The router
  (`internal/router/`, FIFO scheduler + `doSwap` background goroutine) answers
  the request with the *scheduling decision* at request time; the model loads
  in the background. A journal line `<m> Health check passed on ...` = a
  background load completing (it is only logged from the load-wait path, so it
  is a load event, not a monitor tick). Verification must therefore poll the
  engine's own port for readiness instead of relying on the request's status.
- **Without `routing.router.groups` in the config, the v269 group router does
  no cross-model eviction** (`groupSwapper.EvictionFor` only evicts
  same-group-with-swap or exclusive-group members; ungrouped models match
  neither case). One-model-per-GPU on this host is enforced by each `cmd`
  running `evict-gpu.sh` first — that kept working (verified in every load).
  Task-010's config redesign should decide this explicitly (define groups,
  or keep the scripts as the eviction mechanism).
- **Verification footguns that produced the round-1/2 FAIL markers (all were
  measurement artifacts, not v269 or engine defects):**
  1. The sdcpp engine has no `/health` route (404) — its readiness endpoint is
     `/sdcpp/v1/capabilities`; use that for engines.
  2. Firing the next alias's request before the previous engine finished
     loading lets the new cmd's `evict-gpu` kill it mid-load → the gateway
     logs `upstream command exited prematurely` and answers 500. Spacing fix:
     request the next alias only after the current one answers capabilities
     200.
  3. Any *other* client traffic (including the agent's own LLM calls to vllm)
     interleaving with an engine window starts a vllm swap whose `evict-gpu`
     kills the loading engine. Clean verification = a job that owns the whole
     window (vllm down at start, vllm reloaded last) while the agent is
     suspended on the marker wait.
  4. vllm's `/v1/*` needs the API key (the gateway forwards the client's
     `Authorization` header as-is; `/health` is open). A direct no-key probe
     401s.
- **vllm cold-boot tail**: with a cold page cache the boot ran ~16 min and
  briefly blew the 15min `healthCheckTimeout` (journal 00:59:55:
  "health check timed out after 15m0s" + three 500s on `/v1/chat/completions`;
  healthy again 80s later). Warm-cache boots are 50–60s. Task-010 should set
  vllm's `healthCheckTimeout` ≥ 20min (or pre-warm).
- **TTL semantics**: `/running` showed `ttl:120` for kokon-klein while the
  config sets global `ttl: 0` (and vllm showed `ttl: 0`) — v269 may treat an
  explicit 0 differently from v267's "never unload" (or apply a per-model
  default). Check when redesigning the config (task-010); the creative/kokon
  models auto-unloading after ~2min of idle is arguably desirable on a shared
  GPU, but it must be a deliberate config statement, not an accident.
