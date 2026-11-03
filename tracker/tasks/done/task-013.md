# task-013 — qwen on the same stock build (the fork retires)

## goal
The qwen engine becomes the **same** stock stable-diffusion.cpp build as
klein (the named slot / pin from task-002 — `0a06712` was verified to sit
on that upstream line, so both pipelines share one build, one deploy
lineage). Deploy it, verify parity with current qwen production, and
retire the 21-commit fork to `__mess/` (rollback dir kept, like task-011).

## acceptance criteria
- Qwen engine served from the stock build on `:39160` (same serve-script
  shape as klein's); a qwen t2i matches current production quality (owner
  validates — the DiT/VAE/conditioner weights are unchanged, so this is a
  no-regression check, not a quality test).
- The fork tree is archived in `__mess/` with its deploy dir + rollback
  notes; no fork binary on the live path.
- The task-012 exception list (if any) resolved or explicitly parked with
  an upstream-PR note.

## blockers
- task-002 (the build), task-012 (the audit says it's safe).

## decisions
- **One build, two deploy dirs** (the lean option, taken): the stock
  `sd-server` stays in `deploy/klein/`; `deploy/qwen/sd-server` is a
  symlink to it. Two serve scripts, one binary, one build lineage.
- **PUP: Option A (owner's call, "go on a")**. The stock build has no
  in-engine LLM, so the qwen pipelines' prompt upsampler moves to a
  llama.cpp **sidecar** (`kokon-pup-qwen`, :39176) running the *same*
  Qwen3-VL-8B Instruct weights (Q4_K_M + mmproj-F16) the engine
  conditions with. Service change: per-pipeline `models[].vlm` is now
  checked first (commit `5d6ce08`, 323/323 tests), then the shared BFL
  `externalEngine.vlm`, then the in-engine LLM. Q4_K_M matches the
  in-engine PUP's quant; sampling is temp 0 greedy, same max_tokens
  1024, non-BFL pipelines send the built-in Qwen system prompts, and
  Edit-mode jobs attach the reference image. Klein keeps the shared BFL
  sidecar (`kokon-pup`, :39175) untouched.
- **The sidecar is a co-tenant, never an eviction target**: 39176 is not
  in `evict-kokon.sh` (which kills 9100/9101/39160/39170/39171/39175);
  its `qwen-pup-serve.sh` does no evict at all and only checks the
  kokon-policy.txt gate against out-of-band triggers. Turbo engine
  (~10.5 GB) + sidecar (~6-7 GB) fits the 24 GB card; the quality flavor
  is tighter (measured in the window's digest).
- **The config swap happens inside the GPU window**, not at prep time:
  `-watch-config` (2 s poll) reloads on any edit and cmdStops loaded
  models — including vLLM, which serves this session.
- **Window design** (protocol: OPS.md autonomy protocol; scripts in
  `llama-swap-deploy/jobs/kleenq-{test,restore,watch}.sh`, workdir
  `/tmp/kleenq`; final order per the attempt-3 design below): swap to the
  window config (vLLM out of the registry) → canonical vLLM stop → **leg
  A**: fork engine on :39160 (the gateway's new stock supervisor *adopts*
  it — protocol-level port check) with the service on the pre-013 config
  (no vlm entries → in-engine PUP = today's production) → 3 t2i jobs,
  fixed seeds → **leg B**: stock engine (fresh start) + sidecar, service
  on the patched config (per-pipeline vlm → sidecar) → same 3 t2i (same
  prompts+seeds) + 1 edit/i2i job (reference image from the gallery —
  proves the stock vision path and the sidecar's image parts) →
  teardown → swap to `config.yaml.stock-013` → vLLM revived canonically
  **last** → DONE marker.
  Internal deadline 110 min; watcher 150-min cap (pid-aware);
  `wait-marker.sh` blocking wait as the turn's last call, cap 180 min.
  The owner's pi (qwen-direct) turns fail while vLLM is down — expected,
  the marker revives it.
- **AC3 resolved as empty**: the task-012 audit found zero keep-ported
  commits (17 retire / 3 moot), so there is no exception list to resolve
  or park. (Addendum: the audit's "in-engine PUP is unused by the
  service" claim was wrong for qwen — qwen used it; the sidecar above is
  its replacement, recorded in the audit addendum.)
- **Window attempt 1 (14:40–14:55) — infra failure, no GPU damage.** The
  job was launched with `setsid sh` but written as a bash script (arrays);
  dash parses incrementally, so it ran the POSIX-clean prefix (preflight
  ok, vLLM stopped canonically, config swapped to stock-013) and then died
  with a bare parse error at the first array (`PROMPTS=(`, line 147) — no
  marker, no die(). The watcher's 15-min silence fallback fired at 14:55
  and did exactly what it is for: restore (fork config back, service
  healthy, active model klein, vLLM revived canonically) + digest + FAIL
  marker; box safe in ~15 min. Evidence: `/tmp/kleenq-fail1/`,
  `jobs/kleenq.log` ("147: Syntax error: "(" unexpected"). Fix: job
  converted to pure POSIX dash (case-getter instead of arrays; all four
  scripts now `sh -n` clean); the dash-dialect rule + this failure mode
  recorded in the autonomy protocol (OPS.md item 1/7, AGENTS.md).
  Service `/config/config.json` returned to the pre-013 config meanwhile
  (the patched vlm entries are only correct once the stock gateway config
  is live — the window job does the swap itself on each leg).
- **Window attempt 2 (15:40–15:59) — infra failure, five compounding faults,**
  all diagnosed from the logs. The job died at 15:59:13 (its own no-pid-on-
  39160 check, two seconds after the daemon's health check passed on that
  port) and the watcher's silence fallback fired at 15:55 (a 900 s blocking
  trigger curl heartbeats nothing). (a) The config swap was a non-atomic
  `cp`: the daemon's 2 s poll read the file mid-write (15:40:37) and kept
  the FORK config for the whole window — so the daemon's turbo cmd was the
  fork's `sd-cpp-serve.sh`, never the stock supervisor (no `run:` line ever
  appeared). (b) vLLM was left in the registry, so the daemon resurrected it
  4x inside the window (15:55:37/15:56:04/15:59:11/16:04:59 per
  /tmp/vllm_boot.log); the 15:56:30 boot OOM-collided with the 13 GB fork
  engine on the 24 GB card ("memory mapping failed with OOM, free: 4 MB"),
  leaving a zombie holding :18020 (health never 200 — invisible to the
  health-based veto) that refused the canonical revival ("refusing to start
  a second instance"). (c) the watcher silence false-positive (above).
  (d) the health-based veto was blind to the zombie. (e) `ss -ltnp`
  reported no pid for the daemon-owned :39160 socket (unexplained; the same
  tool works on the daemon's own :8081 socket). The restore recovered the
  box: fork config, service pre-013, active klein, vLLM alive (the 16:04:59
  boot eventually won the race). Evidence: `/tmp/kleenq-fail2/`,
  `jobs/kleenq.log` 15:40:34–15:59:13, `/tmp/vllm_boot.log`.
- **Attempt-3 design (bakes all five faults in):** window config =
  stock-013 MINUS the vLLM registry entry (the daemon cannot resurrect what
  is not in the registry — proven clean by the kleinrb2 attempt-1 window),
  swapped ATOMICALLY (cp+mv) and BEFORE the canonical vLLM stop (no
  dead-but-registered gap; the daemon's removal kills the vllm-serve.sh
  watcher, not the vLLM scope, which the job then stops canonically);
  the job writes its own pid (`$W/job.pid`) and the watcher is pid-aware
  (dead/missing pid → immediate fallback; live pid → 150-min hard cap, the
  silence check demoted to a 45-min secondary); every wait heartbeats and
  vetoes, and the veto is PROCESS-based (health-200 OR a :18020 listener OR
  a vllm process among nvidia-smi compute apps → canonical `serve.sh stop`,
  3 strikes = die); the trigger is a non-blocking 420 s curl polled every
  30 s (a fresh supervisor `run: variant=` line confirms the daemon ran the
  stock cmd; a non-200 answer re-triggers; ≤3 attempts); adopt is verified
  by the supervisor's pidfiles (no `ss -p` dependency); teardown ends with
  the atomic swap to `config.yaml.stock-013` (vLLM back in the registry) →
  stop-any-holder → canonical revival (veto disabled against the revival
  itself) → 1-token gateway adoption check → digest → DONE marker last.
- **Attempt-3 run (16:32:11–16:36:54) — DONE, 4 min, vetoes=0, 7/7 jobs.**
  Page-cached loads dominated: fork engine 30 s, each stock engine 2.4–3.8 s,
  sidecar 1.4–2.4 s, vLLM warm boot ~60 s. All legs passed; the cutover is
  LIVE (config.yaml = stock-013, service on the patched config, active
  klein). Evidence: `/tmp/kleenq/` (DONE, digest, run.log, 7 job jsons, 7
  renders, caps, vllm logs), `jobs/kleenq.log`, watcher exit "DONE — all
  good". Owner's no-regression visual verdict on the 7 renders is still the
  outstanding gate before the fork is archived.
- **Churn found in the stock leg (the regression that triggered the two-door
  fix).** In leg B the daemon ran cmdStop on kokon-turbo 5x and on
  kokon-pup-qwen ~6x within ~2 min (engine pids cycled 10–35 s apart), vs
  zero churn in leg A (fork engine, in-engine PUP). Root cause, proven in
  the llama-swap source (`/home/p/ml/backends/llama-swap/`, v269):
  `internal/config/config.go AddDefaultGroupToConfig` puts every ungrouped
  model into the default group with **Swap=true, Exclusive=true** — "out of
  the box llama-swap runs one model at a time" — so the turbo door and the
  engine (both ungrouped) evicted each other on every request
  (`internal/router/group.go groupSwapper.EvictionFor`). Not TTL: the TTL
  goroutine unloads only on `StateReady && inflight==0 && now-lastUse>ttl`
  (120 s) and there is no periodic health poll — a 10–25 s cycle cannot be
  either. The pre-fix production journal (4 days) shows turbo loads spaced
  2–3 min+ apart, i.e. the fork era did not churn the engine — the churn is
  specific to the new ungrouped door/engine pair. Ring-buffer proof of every
  cycle: `deploy/qwen/supervisor.log` + `/api/events/logs?stream=upstream`
  ("stopped engine (pid X)" after each enhance). Impact: ~5–10 s extra per
  job component per request (measured 20–31 s/job stock leg vs 10.5 s fork
  leg). The BFL `kokon-pup` shows similar per-request bursts in production
  journal history (Nov 3 10:14–10:16, 3 loads in 54 s) — established, but
  the qwen door churned harder and was the visible one.
- **Two-door fix (owner GO: "go on the two door fix its the common sense
  aporoach").** The group router allows one group per model, and turbo+full
  cannot share a group (a group load raises ALL members — both DiTs + a
  sidecar ≈ 30 GB > 24 GB). So each taste gets its own sidecar door and
  group: `qwen-t` {kokon-turbo, kokon-pup-qwen} and `qwen-f` {kokon-full,
  kokon-pup-qwen-f}, both `swap:false`, **`exclusive:true` stated
  explicitly** (the YAML zero value for bool is false — the klein group
  omits exclusive by design so the policy gate, not the daemon, arbitrates;
  the qwen groups must be exclusive so the daemon evicts vLLM/creative/klein
  before a qwen load, the fork-era production behavior). The doors run the
  same weights (Qwen3-VL-8B Q4_K_M + mmproj-F16), same alias `qwen3vl-8b`,
  `qwen-pup-serve.sh [t|f]` parameterized (t→:39176, f→:39177, separate
  pidfiles); they never load simultaneously (the groups are exclusive of
  each other); 39176/39177 stay out of `evict-kokon.sh`. Service config:
  Turbo → door-t, Full → door-f (per-pipeline `models[].vlm`, commit
  `5d6ce08`). Files: `config.yaml.stock-013` (updated), `deploy/qwen/*` (4
  scripts), `QWEN-ROLLBACK.md` (two-door version).
- **The exclusive qwen groups exposed a new OOM hole; both supervisor gates
  extended to close it.** The klein group is non-exclusive, so a klein
  request arriving while a qwen pair is resident would NOT daemon-evict the
  qwen pair — both ~20 GB pairs would start side by side on the 24 GB card.
  Symmetric: the qwen groups evict klein in the normal flow, but an
  out-of-band klein start (human script, reload race) has no daemon help.
  Fix: both supervisor policy gates (`deploy/klein/sd-cpp-serve-klein-
  stock.sh`, `deploy/qwen/sd-cpp-serve-qwen-stock.sh`) generalized from
  vLLM-only to **heavy-tenant**: vLLM (:18020/health) OR the other image
  engine (klein :39170 / qwen :39160, by their `/sdcpp/v1/capabilities`
  200). `protect-vllm` (default, `kokon-policy.txt`) = refuse cleanly after
  the 30 s grace (exit 0, GPU untouched — the request fails upstream, same
  as the established klein-while-vLLM behavior); `take-gpu` = stop the
  heavy tenant first (vLLM: canonical serve.sh; the other pair: its stop
  script — for qwen that includes both sidecar doors) with a 120 s wait and
  abort if it won't die. In the normal daemon flow the gate is inert (the
  exclusive groups already evict; the non-exclusive klein group is the only
  path where a co-tenant can be resident). `run:` lines now report both
  tenants.
- **Matrix router parked, not smuggled.** The matrix router (one sidecar
  shared by both flavors, set expressions, evict_costs) could express the
  desired co-tenancy with a single sidecar, but it rewrites the box-wide
  routing (klein/creative/vLLM semantics) — bigger blast radius than the
  fix it would enable. Parked as its own task (task-018) per the owner's
  "common sense approach"; the two-door layout is the interim, boring,
  low-blast-radius design.
- **Two-door verification window (scripts `jobs/qwenp-{test,restore,watch,
  digest}.sh`, W=/tmp/qwenp).** The cutover is already live on the PRE-FIX
  layout (single door, no groups), so the fix goes live inside a window:
  preflight (live config must be the pre-fix version — guards against a
  re-run; all five image ports free; vLLM up; service image 5d6ce08; switch
  bodies BUILT FROM the live service config, never hardcoded) → atomic swap
  to `config.yaml.window-013` (= stock-013 with the fix, MINUS the vLLM
  registry entry) → canonical vLLM stop → **leg 1 klein canary** (the
  reloaded config still runs the untouched klein pair: trigger, engine+
PUP up, one t2i job seed 54, stop) → **leg 2 turbo pair ×2 jobs (CHURN
  CHECK**: engine pid AND turbo-door pid identical across the jobs, ≤1 new
  supervisor `run:` line — pids differ = die) → **leg 3 quality pair
  (FLAVOR CHECK**: turbo pair pre-killed directly for determinism (the new
  supervisor's adopt-if-healthy must never catch a dying turbo engine),
  trigger kokon-full, quality engine is a fresh pid, quality door up while
  the turbo door stays down, one full job, pids stable across it — any
  violation = die) → teardown → atomic swap to the FINAL
  `config.yaml.stock-013` (two doors, vLLM back in the registry) →
  stop-any-holder → canonical vLLM revival (veto disabled) → 1-token
  gateway adoption check → digest → DONE marker last. die() →
  `qwenp-restore.sh` (stop all image processes; keep the final stock-013 if
  the job got that far, else restore the pre-window backup; service healthy
  + active klein; vLLM stop-before-start) → digest → FAIL marker. Job
  self-deadline 55 min; watcher pid-aware 70-min hard cap; blocking
  wait-marker 95-min cap (worst case = watcher hard cap + a 20-min vLLM
  cold-tail in the restore) as the turn's final call. The owner's
  qwen-direct turns fail while vLLM is down — expected.
- **Verification window run (18:08:00–18:12:34) — DONE, 4 min 34 s,
  vetoes=0, all three checks PASS.** Leg 1 klein canary ok (794 KB render,
  enhance 1103 chars; klein pair 19.4 GB). Leg 2 CHURN CHECK PASS: engine
  pid 583869 and turbo-door pid 584275 identical across 2 consecutive jobs,
  exactly 1 new supervisor `run:` line — the per-request ping-pong is gone
  (turbo jobs back to the ~10–11 s class). Leg 3 FLAVOR CHECK PASS: quality
  engine a fresh pid (584987, replacing the turbo's), quality door 585390
  stable across its job, turbo door stayed down; quality pair 21.0 GB of
  24 GB (the tightest config, fits with ~3 GB headroom). vLLM revived in
  60 s, gateway adoption 200. The new `run:` line formats prove the extended
  heavy-tenant gates are live (klein lines report `qwen=`, qwen lines report
  `klein=`). Evidence: `/tmp/qwenp/` (DONE, digest, run.log, 4 job jsons,
  4 renders), `jobs/qwenp.log`, watcher exit on DONE. (Digest cosmetic bug
  fixed same day: its config-identity section used relative refs — the job's
  own absolute cmp checks are authoritative and passed.)
- **Owner verdict (2026-11-03): "not worse, not different indeed. park the
  fork, success achieved we go stock."** The 7 A/B renders (fork engine +
  in-engine PUP vs stock engine + sidecar PUP, identical prompts and seeds
  11/22/33 plus the i2i edit) are the AC-1 gate — passed.
- **Fork archived (AC-2, 2026-11-03).** Verified the live gateway config
  references only stock deploy scripts (turbo/full → `deploy/qwen/sd-cpp-
  serve-qwen-stock.sh`, the two doors → `qwen-pup-serve.sh [t|f]`, the three
  groups as designed, vLLM in the registry; zero references to the fork
  tree). The fork cmd scripts `/home/p/ml/backends/sd-cpp-serve.sh` and
  `sd-cpp-stop.sh` are marked RETIRED in their headers — kept functional
  (`bash -n` clean) as the one-step rollback path per
  `deploy/qwen/QWEN-ROLLBACK.md` (fork config `config.yaml.fork-pre-013.bak`
  + pre-013 service config + restart; first suspect for any drift is the
  ggml pin, stock `d49c49e1` vs fork `ceacb2d3`). The fork build tree
  `/home/p/ml/__mess/sd.cpp` and source `/home/p/ml/__mess/sdcpp-fork` stay
  untouched as the rollback dir, exactly like the klein rollback dir from
  task-011. AC-3 was already empty (task-012 audit: 0 keep-ported).
  Task closed; WP3 continues with task-014 (the PUP A/B).
