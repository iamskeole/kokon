# task-015 — gateway: generic N-models-per-job (the "matrix")

## goal
Generalize the klein pair (task-009) into a first-class configuration for
arbitrary **N models per job** in llama-swap — one lifetime, one TTL,
on-demand — the "matrix" the owner flagged (DiT + VLM now; more later).
Both the klein and the qwen jobs consume it.

## acceptance criteria
- The pinned llama-swap config (v269 or its upstream-PR'd successor)
  expresses an arbitrary model set per job; klein and qwen both use it;
  on-demand semantics (TTL, eviction, reload) unchanged.
- If v269 cannot express a set: an **upstream PR** to llama-swap
  (stock-first — never a fork). The PR branch lives on the host
  (`backends/llama-swap/` at the pinned tag + one PR branch — it is not
  part of the public repo) and its status is recorded here.
- Deploy-dir doc: how to define a new job's model set (the thing the
  fresh-box doc, task-016, will point at).

## blockers
- task-009 (the working pair first — this generalizes whichever path it
  took: native config or script workaround), task-001 (v269 base).

## decisions

- **Native v269 matrix router — no upstream PR needed for the core.**
  v269 (pinned, 9fbc47c) ships a first-class `matrix` router that
  expresses exactly what the task asks: a job declares a SET of models
  that may co-reside, and the router evicts the minimal-cost subset of
  the rest. Source-verified: `internal/matrix/doc.go` (DSL, subset
  semantics, solver), `internal/matrix/program.go:323` (`Program.Solve`),
  `internal/router/matrix.go` (+ `matrix_solver.go`),
  `internal/config/matrix.go` (`vars`/`evict_costs`/`sets`),
  `internal/config/load.go:195-280` (the `routing:` block; `groups` and
  `matrix` are mutually exclusive, XOR with the legacy top-level keys).
  A capability the group router LACKS and the matrix gives us for free:
  **a model may belong to more than one set** (vLLM can be in both the
  klein and qwen sets — the D-QW shared-VLM idea, previously impossible
  because groups forbid double membership). The one group behaviour the
  matrix CANNOT express: the klein asymmetry (klein never evicts, but a
  qwen request evicts klein). stock-015 deliberately drops that
  asymmetry — a klein request now evicts a resident qwen set instead of
  OOMing next to it (the 2026-11-03 landmine) — and vLLM is protected by
  set membership (the `klein` set includes vLLM) instead of group flags.
  If a PR were ever wanted, the remaining gap is exactly this: per-set
  eviction direction. Recorded here so 017 doesn't rediscover it.
- **stock-015 design** (`llama-swap-deploy/config.yaml.stock-015`,
  `-validate` clean, 9 models): models section byte-identical to
  stock-014; the `groups:` block replaced by
  `routing: {router: {use: matrix, settings: {matrix: {vars, sets}}}}`:
  vars k=kokon-klein, p=kokon-pup, qt=kokon-turbo, pq=kokon-pup-qwen,
  qf=kokon-full, pqf=kokon-pup-qwen-f, v=qwen3.8-27b, cf=creative-fast,
  cs=creative-slow; sets: `klein: "k & p & v"`, `qwen-t: "qt & pq"`,
  `qwen-f: "qf & pqf"`, `small: "(cf | cs) & v"`. No `evict_costs`
  (uniform 1 = the group router's flat behaviour); all 9 models are set
  leaves so no `+auto:orphans` is synthesized. The `small` set is
  forward-looking (creatives are never currently resident on this box):
  it fixes the creative-vs-vLLM OOM in the direction the owner's
  preference implies (vLLM evicts a creative, never the other way) and
  lets a creative co-reside with vLLM. vLLM is a leaf of two sets — the
  first use of shared membership.
- **Solver replay (offline proof before touching the live box):**
  faithful port of `groupSwapper.EvictionFor` (incl. the router fast
  path) vs the real `matrixdsl.Compile` + `Program.Solve` on the exact
  stock-015 definitions, over all 4,608 (target × running-subset)
  cases: **0 invariant violations** (vLLM never evicted by
  klein/creative; qwen evicts everything; pair co-residence preserved;
  fast path intact) and **1,262 eviction-set deltas in 254 unique
  families — every one a STRICT SUPERSET of the old eviction set**
  (0 subset, 0 crossover): more clean-swaps, no weakened protections.
  Full output: `/tmp/q15-replay.txt`; harness preserved at
  `llama-swap-deploy/refs/m015-solver-replay/main.go` (the pinned
  llama-swap tree stays clean). The three intended deltas: (1) a vLLM
  request evicts a resident image/creative set instead of cold-booting
  OOM next to it — the nudge-404 class; (2) a klein request evicts a
  resident qwen set instead of OOMing; (3) a creative request evicts a
  resident image job (26B+31B can't share) instead of OOMing.
- **The live window `jobs/m015-{test,restore,watch,digest}.sh`
  (W=/tmp/m015) runs with vLLM IN the registry the whole time** (unlike
  the qwabl A/B windows): stock-015 differs from stock-014 only in
  routing, so the daemon's config reload cycles the vLLM process once
  (documented daemon behaviour — the 23:52 cutover did exactly this and
  it self-revived). The yield rule is trivially satisfied: the window
  never stops a healthy :18020 out of band; `vllm-serve.sh` adopts a
  healthy listener; every eviction goes through the router's own
  cmdStop path (the normal production flow a qwen job always used).
  Steps: P0 preflight (incl. exec-bit audit of stock-015 and the
  live==stock-014 re-run guard) → P1 atomic swap, gated on the daemon's
  `configuration reloaded` journal line → P2 vLLM revival (2-min daemon
  grace, else canonical `serve.sh start --no-wait`) + 1-token
  completion THROUGH the gateway (proves the vLLM route under the
  matrix; this is also this session's chat route) → P3 klein
  protect-vllm regression: a klein job with vLLM resident must FAIL
  cleanly (the supervisor gate's 30 s grace + `upstream command exited
  prematurely`), with vLLM and every port intact — the matrix keeps
  vLLM because it is in the klein set, so the gate sees vllm=up and
  yields → P4 one production turbo job (switch active → turbo; the
  gateway evicts vLLM first — journal proof: `matrix: model=
  kokon-turbo ... evict=[qwen3.8-27b]`; engine + 9B-Q8 door load
  together — door `/proc` cmdline proves the task-014 cutover — one
  image renders) → P5 THE new behaviour: with the qwen pair still
  resident (120 s TTL grace), a vLLM request must make the matrix evict
  BOTH qwen models (journal: `matrix: model=qwen3.8-27b ... evict=[
  kokon-turbo kokon-pup-qwen]`) and boot vLLM cleanly — the nudge-404
  failure class, fixed at the router level → P6 stop the pair, switch
  active back to klein, final state asserts, digest, DONE last.
  `die()` = restore (→ stock-014 + canonical vLLM revival, last) →
  digest → FAIL marker last (the nudge-404 lesson). Budgets: job
  self-deadline 30 min, watcher cap 40 min, wait-marker default 75 min.
- **TTLs carry over untouched** (vLLM none, creative 300 s, kokon
  models 120 s, globalTTL 0): TTL is an inactivity timer in the base
  router, orthogonal to the swapper — the window's P5 120 s grace and
  P6 3-min drain verify it live.

## work done

- 2026-11-04: **Decision: native matrix, no PR for the core.** Full
  source audit of the v269 matrix router (docs + `internal/matrix` +
  `internal/router` + `internal/config`) and of the current
  `groupSwapper` (derived the exact production eviction table from the
  live groups: vLLM/creatives ungrouped → never evict; klein group
  non-exclusive → never evicts; qwen-t/qwen-f exclusive → evict
  everything else). The klein asymmetry is not matrix-expressible and
  is dropped on purpose (see decisions). `config.yaml.stock-015`
  written (groups → `routing.router.use: matrix`, the four sets above,
  models section byte-identical to stock-014) and passes the pinned
  binary's `-validate` (9 models). 4,608-case solver replay: 0 invariant
  violations, 254 unique delta families, 100 % strict supersets (output
  `/tmp/q15-replay.txt`; harness in `llama-swap-deploy/refs/`). The four
  `m015-*.sh` window scripts written and review-hardened (marker-last
  `die()`; P3 dies on a non-terminal klein job, not just a success —
  the 2026-11-02 respawn-race class; P4 asserts the door pid's cmdline
  is the 9B-Q8 PUP; P5 distinguishes matrix-eviction from a TTL race
  and records which happened). Box green at prep time (live==stock-014,
  gateway/vLLM 200, image ports free, service active=klein).
- 2026-11-04 (window attempt 2, 06:27–06:31, FAIL — a one-character bug in my
  own script; the matrix itself kept passing): P1/P2 fine. P3 reproduced
  the clean protect-vllm refusal under the matrix (second time). **P4
  passed the router test for real**: journal 06:31:13 `matrix:
  model=kokon-turbo set=qwen-t dsl="qt & pq" evict=[qwen3.8-27b]
  target=[kokon-pup-qwen kokon-turbo] cost=1` — vLLM evicted for the
  turbo request and (unlike attempt 1) NOT resurrected: the fixed
  `vllm-serve.sh` sat out the swap, the qwen pair loaded, the 9B door
  enhanced (1167 chars) and the engine rendered in 23 s. The window
  then died on my own P4 record extraction: a hand-transcribed multi-line
  python one-liner from the r4 script dropped one closing paren
  (r4-class silent transcription bug; `sh -n` cannot see it — python
  syntax inside shell single quotes), so the job's `file_path` came
  back empty and die() fired 2 s after the job's success. Fixed and
  verified against the real job record (extracts the gallery path);
  added a `[ -s ]` check on the copied render. Evidence preserved at
  `/tmp/m015-attempt1/` and `/tmp/m015-attempt2/` (digests carry the
  matrix journal lines for both attempts). Window re-running (attempt
  3; P5, the vLLM-reclaims-the-resident-qwen-pair test, was the only
  step not yet proven).
- 2026-11-04 (window attempt 3, 06:38–06:43, **DONE in 255 s — task
  complete, stock-015 is now production**): all six phases passed.
  P1 swap + reload in 8 s. P2 vLLM revived through the matrix gateway
  (1-token completion 200; canonical start after the 2-min daemon
  grace). P3 the klein protect-vllm regression under the matrix: vLLM
  KEPT (it is in the klein set), supervisor gate refused cleanly after
  its 30 s grace (`policy=protect-vllm vllm=up ... refused cleanly`),
  job failed fast, GPU and every port untouched — third clean
  reproduction. P4 the production turbo job: journal `matrix:
  model=kokon-turbo set=qwen-t dsl="qt & pq" evict=[qwen3.8-27b]
  target=[kokon-pup-qwen kokon-turbo] cost=1`; vLLM evicted and NOT
  resurrected (the fixed `vllm-serve.sh` sat out), engine + 9B-Q8 door
  loaded together (VRAM 9696 + 10938 MiB; door `/proc` cmdline proves
  the task-014 9B cutover is what production runs), one image rendered
  in 23 s (enhanced 1167 chars, render 1,242,832 bytes copied out).
  **P5 the new behaviour**: with the qwen pair still resident, a
  vLLM request produced journal `matrix: model=qwen3.8-27b set=klein
  dsl="k & p & v" evict=[kokon-pup-qwen kokon-turbo] target=[kokon-
  klein kokon-pup qwen3.8-27b] cost=2` — both qwen models evicted
  (evicted=1, not a TTL race) and vLLM booted clean, completing with
  http 200: the nudge-404 failure class is fixed at the router level.
  P6 end state: live config = **stock-015 (the matrix router) in
  production**, active model kokon-klein, vLLM healthy, all image
  ports free. One-step rollback: `cp config.yaml.stock-014
  config.yaml.new && mv -f ...` (the groups router; scripts are
  router-agnostic). Deploy-dir doc written: **`MATRIX.md`** (how to
  define a new job's model set — vars/sets DSL, membership rules,
  evict_costs, the orphans trap, the live-change window protocol, the
  `vllm-serve.sh` booting-trap and its rehearsal, rollback) with a
  pointer from README.local.md. All ACs met: pinned v269 config
  expresses arbitrary sets per job; klein and qwen both use it; TTL
  (120 s grace observed in P5) / eviction (strict-superset replay +
  live journal) / reload semantics unchanged; no upstream PR needed.
  Evidence: `/tmp/m015/` (final), `/tmp/m015-attempt1/`,
  `/tmp/m015-attempt2/`, `/tmp/q15-replay.txt`,
  `refs/m015-solver-replay/`, gateway journal since 06:38.
- 2026-11-04 (window attempt 1, 06:00–06:08, FAIL — root-caused and
  fixed): P1 (swap, reload in 8 s), P2 (vLLM revival through the
  matrix), and **P3 (the protect-vllm regression) passed exactly as
  designed** — the journal shows `matrix: model=kokon-turbo ...`-style
  decisions with vLLM KEPT for the klein request (it is in the klein
  set), the supervisor gate refused cleanly after its 30 s grace
  (`policy=protect-vllm vllm=up ... refused cleanly`, GPU untouched),
  and the klein job failed fast with vLLM and every port intact. P4
  then failed: the matrix correctly decided `evict=[qwen3.8-27b]
  target=[kokon-pup-qwen kokon-turbo]` for the turbo job, but **vLLM
  was resurrected 2 s after the router killed it** (boot log:
  `vllm.sh start` 06:03:37), so `serve.sh stop`'s 90 s GPU-empty wait
  never completed, the qwen pair never loaded, and the job hard-failed
  on the service's 5-min VLM timeout (D-FB). Root cause — a real latent
  race, not a window artifact: a gateway `vllm-serve.sh` wrapper
  spawned 06:02:40 (my P2 probe while vLLM was still booting) was
  parked inside `evict-gpu.sh`'s "wait for the GPU to empty"; the
  moment P4's evict killed the booting vLLM, that parked wrapper woke
  to an empty GPU and cold-launched vLLM again — reviving the exact
  model the router had just evicted, before its targets loaded. The
  same sequence is reachable in production (a qwen request arriving
  while vLLM is cold-booting). **Fix: `llama-swap-deploy/vllm-serve.sh`
  gained a booting branch** — port held (or a <600 s fresh
  `VLLM::EngineCore`, mirroring serve.sh's BOOT_GRACE) but not healthy
  = a boot in progress → the wrapper waits for it to resolve (watch if
  it becomes healthy; release WITHOUT launching if it dies or crawls
  past 15 min). Only a fully-down vLLM (no port, no fresh core) takes
  the evict+launch path, so the on-demand revival (the 23:52 style)
  is preserved. Rehearsed on a throwaway port with a fake dying boot:
  the wrapper now logs `in-flight boot died ... NOT relaunching` and
  exits 0, with the box's real (stale) vLLM core present — the old
  version's `pgrep` pattern also false-positived on stale cores, which
  the freshness check removes. Post-mortem evidence:
  `/tmp/m015/` (run.log, digest.txt, turbo-job.json, journal lines in
  the digest), `/tmp/vllm_boot.log` (the 06:02:08 / 06:03:37 boot
  markers), the systemd scope `run-p1029862` (the resurrected vLLM —
  now serving the box's chat, healthy).

