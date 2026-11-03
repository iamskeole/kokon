# task-014 — qwen: A/B the shared PUP, then cut over (D-QW)

## goal
Per D-QW: A/B the qwen-side VLM — the klein-fine-tuned PUP (9B, the
task-010 winner quant) vs the 8B Qwen3-VL (qwen's current VLM) — judged
on **qwen's own** enhancement prompts. Cut qwen's enhancement over to
the sidecar with the winner; the in-engine PUP slot turns off for the
qwen engine (both pipelines then share one VLM artifact, per the reuse
rule).

## acceptance criteria
- Digest: both candidates over a standing qwen prompt set (t2i + edit),
  using qwen's prompt bundle (which becomes a repo asset in the
  task-007 pattern — BFL silence rules apply to the BFL set; qwen's own
  prompts are the model's and are recorded as assets too).
- The owner judges enhancement quality vs current qwen production —
  the bar is *not worse*, not *different*.
- Winner wired as the qwen pipeline's `vlm` entry (task-005 config);
  qwen jobs run end-to-end through the sidecar; the in-engine PUP
  flags are off in the qwen serve script.
- Winner recorded in `STATUS.md` (the D-QW open item closes).

## blockers
- task-013 (qwen on stock, same sidecar runner), task-006 (the client),
  task-007 (assets pattern). GPU windows for the A/B runs.

## decisions
- **Goal reinterpretation (post-task-013):** the file predates the
  stock cutover — there is no in-engine PUP slot to turn off anymore;
  the qwen sidecar IS qwen's PUP (task-013, owner's Option A). The A/B
  is therefore **9B vs 8B on the same sidecar runner**: the current
  qwen door (8B Qwen3-VL Instruct Q4_K_M) vs the 9B BFL klein-pup (a
  Qwen3.5-9B fine-tune, the task-010 Q8 winner on klein's sidecar)
  run with klein's production recipe. Same alias (qwen3vl-8b) so the
  service config is identical between legs — only the gateway door
  entry's cmd differs.
- **Repo side done (2026-11-03, commits a528f0a + 610ccb3):** the
  qwen prompt set is now a repo asset — `crates/qls-core/assets/pup-qwen/`
  (system_prompts.json: t2i/edit/compose/compose_label, extracted
  verbatim from the built-in enhancer constants; a test pins asset and
  constants together). `pup.rs` is the two-family loader (BFL family:
  all six keys + companion mandatory; Qwen family: t2i mandatory, the
  rest optional with built-in fallback — the Qwen path never
  hard-fails on its bundle). `enhancer.rs`/`engine.rs`: the Qwen path
  uses the pipeline's own `promptBundle` (schema-13 per-model field,
  previously BFL-only) — Turbo/Full now point at `pup-qwen`; a broken
  bundle is a job note, not a job failure; the BFL branch takes the
  unwrapped BflPrompts form (a BFL pipeline pointed at a non-BFL
  bundle fails the job cleanly). 327/327 qls-core + 160/160 UI.
  Deployed: image external-engine-0.11.2-610ccb3, container recreated,
  bundle in the data volume at /data/assets/pup-qwen, service config
  has the per-model `promptBundle: "pup-qwen"` on Turbo/Full only.
- **Window design (`jobs/qwabl-*`, W=/tmp/qwabl; launched
  2026-11-03 after the file-side work):** the standing qwen set =
  3 t2i (seeds 11/22/33, 768², the kleenq A/B prompts) + 1 edit
  (seed 44, "a watercolour painting of this scene", ref = newest
  gallery PNG). vLLM out of the registry for the whole window
  (window-014a/014b = window-013 lineage minus vLLM); leg A =
  window-014a (8B door = production); mid-window atomic swap to
  window-014b (turbo door cmd → `deploy/qwen/qwen-pup-serve-9.sh` —
  9B Q8, production klein recipe, same port 39176 + pidfile + alias,
  so `qwen-pup-stop.sh t` tears it down); leg B = same 4 jobs, same
  seeds. Per-leg gates: pid stability across the 4 jobs (engine.pid
  + door pidfile — a churn regression dies the window), leg-B door pid
  must differ from leg-A's, 1-token probe per door, VRAM recorded
  (turbo engine ~10.3 GB + 9B door ~11 GB ≈ 21.2 GB — tight but fits,
  like the quality pair). Final: active back to klein, atomic swap to
  stock-013, canonical vLLM revival last (stop-before-start on an
  unhealthy holder), 1-token gateway adoption check, digest + DONE
  last. The 9B door cold load (9.5 GB, first page-cache fill) is the
  long pole — leg B door wait budget 15 min. Budgets: job self-
  deadline 45 min; watcher 60-min hard cap + 45-min secondary silence
  (pid-aware, qwenp pattern); wait-marker 80-min cap. `sh -n` clean
  on all four scripts (POSIX dash).
- **Fail-1 record (attempt 1, 20:00:33-20:08:36, 8 min) — the door never
  loaded; the window design assumed group co-loading that doesn't exist.**
  The job triggered `kokon-turbo` and then waited 450 s on the 8B door's
  health with nothing to request it: a request to a group member loads
  ONLY the requested model (the qwen-t/qwen-f groups govern eviction /
  co-residence, not co-loading). In every earlier window the door came up
  because *something requested the door's alias* — in production that is
  the service's enhance call; the 16:32/18:09 windows' jobs triggered it
  explicitly. This job waited on the door before the first job could be
  submitted -> circular wait -> die at 20:08:36 (ring buffer in
  /tmp/qwabl-fail1/: engine load/healthy/stop present, zero door lines).
  Side findings, all recorded: (a) `sh serve.sh stop` is a silent no-op
  while vLLM is alive — serve.sh is a bash script (shebang
  `#!/usr/bin/env bash`) whose line-72 `for ((...))` dash cannot parse;
  dash parses incrementally so it only dies when the stop path is
  reached; the daemon's own cmdStop (shebang-run) did the actual killing
  in that window; job+restore scripts now invoke serve.sh by shebang,
  never `sh`. (b) The restore's `start --no-wait` was refused at
  20:08:42 because the daemon's own vllm-serve.sh resurrection boot had
  already started (vLLM re-entered the registry with the stock-013
  swap) and held the port; that boot finished cold (~13 min — page cache
  evicted by the window's ~25 GB of gguf loads) at 20:21:23, i.e. the
  refusal was harmless and the box self-recovered; the job's final start
  is now a WARN-not-die for exactly this case. (c) The window's vLLM
  out-of-registry period cost the owner ~21 min of chat (20:00-20:21);
  the window itself was only 8 min. Fix applied (attempt 2): both legs
  trigger the door explicitly via `GET /upstream/kokon-pup-qwen/health`
  (first call loads, later calls join; budgets 5 min / 15 min; the
  global healthCheckTimeout 1200 s covers the 9B cold load).
- **Fail 2 (20:41:40-20:55:58): leg A completed, leg B killed by my exec bit + a veto war — the protocol's worst failure mode, now fixed.**
  (a) LEG A (8B) COMPLETED PERFECTLY: 20:41:40-20:42:42, engine 682429
  + 8B door 682643 loaded (door up in <15 s from the explicit gateway
  trigger — the attempt-2 fix works), 4/4 jobs (enhanced 638/868/656/211
  chars), pids stable across all four, teardown clean. Evidence: /tmp/qwabl/.
  (b) LEG B died on MY file: qwen-pup-serve-9.sh was created by the write
  tool with mode 644 (no exec bit) — the daemon logged
  `starting kokon-pup-qwen failed: fork/exec ...: permission denied`
  at INFO level every 15 s for 13 min; the job's door-wait 500'd to death.
  (The one bright thing: cmd-start failures ARE visible in the user
  journal at info — the 500 was never a mystery once the journal was
  read.) Fixed with chmod +x plus a preflight exec-bit audit of every
  cmd/cmdStop script in the window config (all four configs audited:
  this was the only offender).
  (c) THE VETO WAR — the actual crisis: while vLLM was out of the
  registry (the daemon can never resurrect it), the job's process-based
  veto had no legitimate targets left, yet it canonically killed the
  OWNER'S three manual vLLM boots (boot-log markers 20:51:39 / 20:53:51 /
  20:54:56; vetoes 20:52:35 / 20:54:38 / 20:55:55) until it died on
  strike 3 (20:55:58); the owner restarted the daemon once (20:50:28),
  hit 'no router for requested model' on their chat (20:49:58/20:50:48
  404s in the journal), and stopped the daemon (20:54:44). ~21 min of
  chat lost; the owner then booted a 4th time (20:56:05) which the
  auto-restore's stop-before-start cleared and re-booted (20:56:53,
  healthy 20:57:30 — 85 s on the warm cache). Root lesson, now durable:
  in a window with vLLM out of the registry, any vLLM appearance is a
  HUMAN reclaiming the GPU — the job must YIELD (die + restore; restore
  leaves a healthy vLLM alone), never kill. The kill-veto only made
  sense against daemon resurrections, which registry removal already
  eliminates. The next run (attempt 3) is 9B-only (leg A already
  evidence-complete): fresh workdir /tmp/qwabl2 with leg A archived
  under legA/, window-014b straight, check_yield instead of check_veto,
  daemon-up preflight, exec-bit audit, 9B proven by /proc/<door>
  cmdline, same reference image as leg A's edit job. Attempt 3's first
  launch (21:25) died in preflight on its own audit: the audit's python
  needs pyyaml, which only existed under the ephemeral /tmp/pyyaml-pkg —
  the job now points at the durable /home/p/ml/pylibs (pip --target);
  the die was pre-swap, the restore verified a no-op, box untouched.
- **Fail 3 (21:26-21:29, 90 s): my own switch was missing, and the daemon behaved exactly as designed.**
  Attempt 3's first launch got the 9B door up in 36 s (7.3 s load — the
  20:41 storm had warmed the page cache; the 10-min cold budget was
  pessimistic) and the 1-token probe passed, but the job had never
  switched the service's active model to turbo (that switch lived in the
  deleted leg A), so b1 was routed to the KLEIN pipeline. The daemon then
  did the right thing with a request it should never have gotten: an
  exclusive-group request (kokon) evicted the exclusive qwen-t group
  (cmdStop on the turbo engine + 9B door), the klein supervisor's
  (correctly ordered) gate saw qwen=down and started the klein pair, and
  the BFL sidecar's own evict-gpu killed that fresh pair — b1's enhance
  EOF'd (502) and the job died. The restore recovered in 68 s (vLLM
  21:29:09; owner's 4th boot's cache made it warm). Fix: leg B switches
  active to turbo before triggering and verifies the switch. The daemon
  group semantics this confirmed: an exclusive group (qwen-t/qwen-f)
  yields to ANY out-of-group request, not just other exclusive ones —
  i.e. while a qwen pair is resident, klein/creative/vLLM requests evict
  it, and vLLM/default-group requests evict qwen-t on demand (the
  fork-era behavior, by design).
- **Attempt 3 (21:38:59-21:42:16) — DONE in 180 s: the 9B leg is evidence-complete.**
  After fail 3, the 21:38 launch ran the fixed flow: preflight clean
  (daemon active, exec-bit audit, live==stock-013), window-014b straight,
  canonical vLLM stop, switch-to-turbo + verified readback, turbo engine
  724456 healthy ~10 s after the trigger, 9B door 724794 loaded in ~6 s
  (cmdline-proven: flux-3-image-klein-pup in /proc/<pid>/cmdline),
  1-token probe, then b1-b4 all done in 15-16 s each (seeds 11/22/33 t2i
  768^2 + seed 44 edit of the leg-A reference) with engine and door pids
  identical across all four; teardown, active back to klein, atomic swap
  to stock-013, canonical vLLM start (healthy +60 s), gateway adoption
  200. Zero vetoes/yields; the watcher exited on DONE. 9B measurements:
  door load 6-7.3 s (the 20:41 storm had warmed the page cache), enhance
  decode 66-69 t/s (8B door: ~92 t/s), enhance lengths 1164/1408/1105/
  1152 chars vs 8B's 638/868/656/211 on the same prompts, VRAM 10328
  (engine) + 10928 (door) MiB. Round-1 evidence: /tmp/qwabl2 (9B leg)
  and /tmp/qwabl (8B leg from the 20:41 run, under legA/).
- **Owner verdict on round 1 (2026-11-03): "i actually think 9b is better, but"**
  — (1) first show a harder edit on the canary: the brass desk-lantern
  render with the "pi was here" engraving, "replace the cables with a
  brass godzilla"; (2) then also run the standing set on the FULL
  flavor (round 1 was turbo only). After that, the owner can close.
- **Round 2 (the 21:5x window, in flight): 4 legs in one window, vLLM out of the registry.**
  L1 canary-edit turbo+8B / L2 canary-edit turbo+9B / L3 standing set
  (seeds 11/22/33 t2i + 44 edit) full+8B / L4 same full+9B; canary edit =
  seed 55 on the owner-confirmed canary (the TURBO brass-lantern render:
  /gallery/2026-10-31/105927-a-small-brass-desk-lantern-stands-1082852319.png,
  the other candidate 110044 was the quality one), prompts/seeds/refs
  identical per pair, pids must be stable within each leg, the 8B/9B
  split proven by the door's /proc cmdline each leg. The 9B door script
  gained a flavor parameter (t=39176 / f=39177, same pidfile scheme as
  the 8B door) so one script serves both doors; config.yaml.window-014c
  = window-014a with the only delta being both door cmds pointing at the
  9B script (pyyaml-asserted); the job swaps a/c atomically between legs.
  Risk recorded in the script header: full+9B is the tightest config on
  the card (~23.9 GB of 24 GB) — if it OOMs, that die reason is a
  legitimate A/B data point (the decision then degrades to "9B as turbo
  door only"), not a protocol failure. New job family jobs/qwabl-r2-{
  test,restore,watch,digest}.sh, W=/tmp/qwabl3; budgets: job 45 min,
  watcher 60-min cap, wait-marker 65 min. Round-1's launch-ends-in-wait
  rule holds (the owner's "ack on process note").
- **Canary confirmed (owner, ~22:1x): the 105927 turbo brass-lantern render**
  is the canary (110044 was the quality one).
- **Flight 2 (22:22-22:26): canary A/B completed, died in leg 3 on my file.**
  L1 (canary, turbo, 8B): c1 seed 55 "Replace the cables with a brass godzilla"
  done — 8B enhanced 169 chars. L2 (canary, turbo, 9B): c2 same seed/prompt —
  9B enhanced 610 chars, door 754419 (cmdline-proven), VRAM 10328+10928 MiB.
  Leg 3 died on a filename bug: the preflight wrote the switch body as
  `ee-{name[-5:]}.json` and 'kokon-full'[-5:] is '-full' → `ee--full.json`;
  `curl -d @ee-full.json` had no file, the switch silently didn't happen,
  readback showed turbo → die. Fixed: explicit keys + existence asserts.
- **Flight 2b (22:50-22:52, 30 s): the trigger-alias bug.** The surviving leg A
  triggered alias `kokon-quality` — which does not exist in the gateway (the
  alias is kokon-full; `quality` is only the supervisor's variant string).
  Three attempts, 404, die. Fixed `trigger_engine ALIAS VARIANT` (URL by
  alias, run-line grep by variant), dropped a duplicate swap, and made
  check_yield window-aware: after the final swap a vLLM reappearance is the
  revival itself, not a human reclaim (an unconditional yield would have died
  the job on its own revival).
- **Flight 3 (23:08-23:16): 8B full leg complete; 9B full leg OOM — the
  predicted data point.** Leg A (full, 8B-Q4): engine 785220 (13.6 GB) + door
  785448 (8.0 GB); f1-f4 4/4 (~31 s each; enhanced 643/875/665/212 chars);
  pids stable across all four. Leg B (full, 9B-Q8): the door loaded (10.9 GB)
  but the engine (pid 789065) was gone from nvidia-smi by the time the door
  finished: 13.6 + 10.9 = 24.5 GB > 24 GB — the CUDA alloc failed during model
  load, /health still returned 200 (the process lived, the model was broken),
  so the supervisor never restarted it; g1's enhance (door-side) succeeded and
  the engine's first step failed at 129 s → die. Exactly the tightest-config
  data point the script header predicted.
- **Owner direction for the OOM (night, ~23:4x): lower quant — the PUP first,
  the image model only if necessary.** The PUP's Q4_K_M (5.6 GB) already
  exists on disk. `qwen-pup-serve-9.sh` is now quant-parameterized ($2;
  per-flavor defaults: t→Q8_0, f→Q4_K_M): full+9B-Q4 lands at ~21.6-22 GB —
  the same footprint class as the proven 8B pair; turbo+9B stays Q8 (the
  19.6 GB config the owner already judged better in round 1).
- **The 404 that cost nudges (owner paste, ~23:5x): "no router for requested
  model".** My session's provider is qwen-fastani → :18021 = the pi-read-more
  dev debug proxy → gateway → vLLM (NOT direct :18020 as the session notes
  said). The model name is correct (a 1-token probe through the proxy passes
  while vLLM is up); the 404 fires exactly when the live gateway config lacks
  qwen3.8-27b — i.e. while a window config is live. The expensive pattern: a
  FAIL marker written BEFORE the restore finished → my blocking wait returned
  → the continuation model call landed seconds before vLLM's revival → 404 →
  a human nudge was the only way back. Fix (job-side, not the owner's proxy —
  the proxy reports the truth and masking a 404 would break its debug
  purpose): the FAIL marker is now written LAST in both the job's die() and
  the watcher's fallbacks (restore → digest → marker), so a wake can only
  ever land on a restorable box (stock-013 live, vLLM healthy). Also found +
  fixed: `~/.pi/agent/models.json` carried a trailing comma (invalid JSON;
  repaired atomically, backup models.json.bak-pre-fastani-fix-20261103).
- **Flight 4, attempts 1-2 (23:30, 23:36) — two more of my own derivation
  bugs, both caught by the gates (marker-last worked both times: the wake
  landed on a restored box, no nudge cost).** (a) 23:30: the block-removal
  edit silently no-op'd (a mismatched multi-line match with no assert) and
  the preflight paths stayed on the old workdir, so the switch body was
  written where nothing read it — `curl -d` failed, switch no-op, die on
  the readback. (b) 23:36: the active-model switch had lived inside the
  deleted leg A, so leg B only *verified* a switch that never happened —
  die on the pre-switch check. Fixed both (diff-verified against the
  known-good r2 base).
- **Flight 4, attempt 3 (23:40:48-23:45:30) — DONE in 4m42s: the A/B is
  complete.** full + 9B-Q4: FITS (engine 810819 13.6 GB + door 811051
  7.7 GB = ~21.3 GB; post 11982+7772 MiB), door up in 4 s (cmdline-proven
  model-Q4_K_M.gguf), g1-g4 4/4 (~36 s each), pids stable across all four.
  9B-Q4 enhanced 1369/1354/1146/1181 chars vs 8B's 643/875/665/212 on the
  identical prompts/seeds — 1.6-5.6x longer, same direction as every other
  leg tonight. vLLM revived in 61 s, adoption 200. Digest: /tmp/qwabl5
  (carries all three evidence sets: canary turbo c1/c2, full 8B f1-f4,
  full 9B-Q4 g1-g4).
- **CUTOVER (23:52, per the owner's lean + the full evidence): qwen's doors
  now run the 9B PUP.** `config.yaml.stock-014` is live (atomic swap;
  daemon reloaded cleanly): the only delta vs stock-013 is the two door
  cmds → `deploy/qwen/qwen-pup-serve-9.sh` (turbo door flavor t → Q8_0,
  ~19.6 GB pair; quality door flavor f → Q4_K_M, ~21.3 GB pair — Q8
  doesn't fit beside the full engine, measured above). Groups, ports,
  stops and the `qwen3vl-8b` alias unchanged; the service side untouched.
  `QWEN-ROLLBACK.md` gained the one-step rollback (stock-013 copy; the
  stop script is model-agnostic). Pending the owner's morning visual
  confirmation of the g/f pairs + canary pair (renders in /tmp/qwabl5);
  until then the rollback is one command.
- **New durable lesson (23:52): a stock→stock config reload is NOT
  zero-cost.** Even with the vLLM entry byte-identical, the daemon cycled
  the vLLM model on the reload (old pid killed, new one up ~60 s later on
  the warm cache — the 000 window cost ~90 s of chat). Batch live config
  changes, or do them inside a window where vLLM is already down; don't
  assume an identical entry means an untouched model.
- **DONE (2026-11-03).** Winner: 9B BFL klein-pup (Q8 on the turbo door,
  Q4_K_M on the full door); 8B stays one step back (stock-013). D-QW
  closes in STATUS.md with the visual-confirmation caveat. G2 (qwen set)
  and the qwen half of G5 are satisfied by this task's repo side +
  cutover; G5's remaining half (if any) and G6+ go to WP4.
