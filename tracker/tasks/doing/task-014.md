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
  cmdline, same reference image as leg A's edit job.
- (open until the A/B lands: winner model + quant for qwen)
