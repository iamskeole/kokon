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
