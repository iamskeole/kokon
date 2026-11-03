# task-009 — gateway: one job stands up {klein engine, PUP sidecar} with one lifetime

## goal
A klein generation job needs **two** models resident (the DiT engine and
the PUP sidecar) with one shared lifetime (on-demand load, one TTL,
clean teardown). Make the gateway express that — natively if v269's
config schema can, otherwise by the documented operational workaround.

## acceptance criteria
- **Path A (native):** v269 `config-schema.json` supports a set/group of
  models per upstream alias → configured so one on-demand load raises both
  and the TTL tears both down. Acceptance: one gateway request → both
  processes up; job completes; both gone after TTL.
- **Path B (workaround):** the klein serve script also starts/stops the
  sidecar (documented in the deploy dir, tested). Same acceptance, minus
  the native-config claim.
- The choice is recorded in this file's decisions (it is the input to
  task-015, which generalizes whichever path wins).

## blockers
- task-001 (v269), task-002 (engine), task-004 (sidecar) — all done.
- **Live proof: DONE** (2026-11-03 00:45, window run 6, DONE marker). All
  criteria in the section above verified live — see "Results" below.
  Residual (accepted, noted): a production kokon job submitted while the
  window holds the GPU hard-fails its sidecar call (no job was running
  during the overnight windows; the window is announced before it starts
  and an owner nudge at any point takes the GPU back — veto).

## decisions
- **Path A + B fused (A is the container, B is the executor).** v269's
  config schema has `groups` (swap/exclusive/persistent/members) and
  `matrix` (solver-based alternative; a config uses one or the other).
  `groups` is the right native mechanism, but it expresses co-residence
  and eviction behaviour only — it cannot make one request *start* two
  processes. So: the `kokon` group keeps {kokon-klein, kokon-pup}
  together at gateway level (swap:false = co-resident; otherwise a normal
  exclusive one-GPU group), and the klein alias's cmd is a joint
  supervisor that raises/adopts BOTH the stock klein engine (:39170) and
  the PUP sidecar (:39175) and keeps them alive together; both aliases'
  cmdStops stop both. That is the answer to record for task-015.
- **One authority for the GPU = the gateway (owner's design, adopted
  after the 21:15 incident).** No script kills model processes directly:
  requesting the pair makes the gateway unload vLLM via vLLM's clean
  cmdStop (and vice versa); the owner's nudge is a VETO on the GPU (the
  gateway unloads the pair, loads vLLM; the window's collision loop
  re-requests the pair). The `kokon-policy.txt` gate (default
  protect-vllm, 30 s grace) in both serve scripts is a last-resort safety
  net for OUT-OF-BAND runs only; on the gateway path it is a dead letter
  (the group's exclusive swap already unloaded vLLM before the cmd runs).
- **Incident 2026-11-02 21:15 (the one that needed a reboot) — root
  cause.** Window v1 (a) killed vLLM directly (kill_vllm) and (b) claimed
  in its header that it had set the policy to take-gpu — but never wrote
  it. The owner's nudge 10 s after the window's trigger raised vLLM; the
  pair supervisor (still protect-vllm) correctly yielded the GPU and
  exited — gateway log: "group: starting kokon-klein failed: upstream
  command exited prematurely" (journal 21:15:37). The GPU churn (23 GB
  context torn down + immediate reload under load) left vLLM unstable:
  every subsequent LLM stream died (gateway journal 21:21-21:37, ~2-min
  harness retries); the direct boot died too (driver poisoned by the
  SIGKILL storm); the owner rebooted at 21:46 — the clean fix. Deeper
  flaw: three competing authorities over the GPU (script kills, policy
  file, the gateway's loaded table).
- **Two further self-inflicted vLLM stops that same night, both
  self-healed without the owner (evidence the clean-stop design holds):**
  1. the 21:5x groups-config edit triggered the gateway's 2 s-poll
     reload; the reload cmdStops loaded models (no rehydrate) → vLLM
     stopped clean and came back on its own;
  2. the 22:26 PUP gate test: the first gate version had a broken vLLM
     probe (`-o /dev/null` discards the body, so `grep -q 200` never
     matched → the gate was a no-op) → the old evict-gpu ran, stopped
     vLLM, and the PUP sidecar began loading; the test was aborted (my
     timeout) and the sidecar died with its parent. vLLM revived in ~2
     min — **CORRECTED mechanism** (later source-verified): NOT a restart
     loop (vllm-serve.sh is attach-and-watch and exits when vLLM dies; the
     stable deploy's start script `exec`s, no loop). The revival was
     v269's REQUEST-DRIVEN load: an API "unload" is Stop-only (the model
     stays in the table), so any request to the qwen alias re-runs its cmd
     (EnsureReady). My own agent harness was that client (it kept
     requesting vLLM through the debug proxy). Same for the 21:38-era
     revival the reboot ended. See "v269 semantics" below.
  Rule born of #2: **unit-test guard probes in isolation before running
  any script with a kill path** (the fixed probe was verified to detect
  vLLM up first; the live gate test then refused cleanly after the 30 s
  grace and left vLLM's pid unchanged).
- **Config edits that change routing bounce loaded models** (reload =
  cmdStop, no rehydrate). Hence: edit routing config with vLLM down, or
  expect one clean bounce + auto-revival (both observed working tonight).
- **v269 semantics (source-verified 2026-11-02/03 — corrects assumptions
  carried from task-008):** (a) there is NO autonomous monitor or timer
  respawn anywhere (checked root main, internal/*, cmd/kubeswap — the only
  tickers are the config dir-watcher, the kubeswap K8s poll, and the wol
  proxy); a dead or stopped model is re-run ONLY on demand — any request
  hitting its alias (or a doSwap) calls EnsureReady. This is the entire
  "vLLM automation" the owner expected: request-driven, not timer-driven;
  (b) `POST /api/models/unload/{model...}` = Stop only — the model STAYS
  in the table (there is no removal route at all; pause/resume only), so
  it revives on the next request; (c) cmd start/stop/clean-exit log lines
  are Debugf (process_command.go 541/553/558) — invisible at the live
  `logLevel: info`; only TTL-unload (Info) and unexpected-exit (Warn) show
  in the journal, so journal silence about cmd runs proves nothing.
- **The nudge problem (new, solved):** my own agent harness is the client
  that keeps resurrecting vLLM — its completions go through pi-debug-proxy
  (:18021) → the gateway (:8081), and the completion that issues the
  blocking wait arrives ~3 s after a window's vLLM kill, re-loading it in
  ~65 s (proven from the debug-proxy's SQLite request records: 00:00:15→
  00:00:24 and 00:13:12→00:13:18; the same explains the 22:47 and 23:31
  revivals). Fix: the window STOPS pi-debug-proxy after taking vLLM down
  (my LLM errors for the window's duration — the accepted autonomy-protocol
  failure mode; the job is a setsid host process and the watcher is the
  backstop) and RESTARTS it at teardown and on every fallback path.
- **Window failure post-mortems (all four self-healed; owner asleep):**
  run 2 (23:2x): supervisor's first health poll at t+5 s killed still-
  LOADING children (loading ≠ death) → 240 s startup grace added to the
  supervision loop (grace expiry = fatal exit 1; post-ready death = reap
  the pair); run 3 (23:30): scratch service via docker failed (KOKON_IMAGE
  unset; the image's kokon-service binary takes no --listen/--engine-url
  flags — it is configured via QLS_CONFIG_DIR/QLS_DATA_DIR + token +
  /api/v1) → replaced with the task-008 host-debug-binary pattern; run 4
  (00:00): debug binary needs LD_LIBRARY_PATH (openssl) at runtime + the
  first hard evidence of the harness nudge; run 5 (00:12): API unload is
  Stop-only (assumed table removal — it isn't) so vLLM came back on the
  harness's next request; job-B submit parse gave a false negative
  (multi-line python in shell) → file-based parse + grep fallback; the
  fallback's gateway vLLM trigger didn't come back within 10 min (undivined
  — likely the pair still holding the GPU while vLLM loaded; the direct-
  start last resort resolved it). Run 6 proved the trigger path works when
  the GPU is actually free.
- **Results (run 6, DONE 00:45 — all acceptance criteria met):** one
  gateway request raised BOTH pair members (endpoints healthy in 40 s —
  page-cached weights; the supervisor's 240 s grace covers the cold case);
  job A t2i done in 16 s; job B box-edit done in 20 s with the locked bbox
  table SURVIVING the sidecar verbatim (rows a1/b2 byte-identical; the
  sidecar ADDED three of its own src==tgt locked rows — extension, not
  corruption — the window's exact-2-rows checker was over-strict, verdict
  recorded manually); the 120 s TTL took both down together; a second
  request revived the pair in 20 s (request-driven, as v269 does it);
  vLLM back through the gateway's own trigger (40 s) with the vllm-serve
  wrapper watching it again; pi-debug-proxy restarted; policy
  protect-vllm; all ports clean; watcher stood down on the DONE marker.
  End state: vLLM up AND watched, pair out of the way, no loops.
- **Artifacts on the host (final, all in place):** config.yaml (+ .orig)
  carries `groups.kokon = {swap:false, members:[kokon-klein, kokon-pup]}`
  (one GPU authority: loading the group evicts vLLM and vice versa;
  swap:false keeps the two members co-resident); deploy/klein/
  sd-cpp-serve-klein-stock.sh = the joint supervisor (policy gate with 30 s
  grace → adopt-or-raise BOTH → 240 s startup grace → co-supervise; file
  log supervisor.log); deploy/klein/sd-cpp-stop-klein-stock.sh = cmdStop
  stopping both; the live pup-serve.sh (/home/p/ml/backends/) has the
  policy gate before its old evict (out-of-band safety net only); evict-
  kokon.sh = SD-tenant-ports-only evict (never touches :18020); jobs/
  klein-pair-test.sh v4 (the DONE version: unload vLLM + stop proxy → one
  trigger → collision loop with per-poll logging → scratch service via
  host debug binary + schema-13 config → job A/B → TTL check → revival
  check → teardown: unload both, restart proxy, gateway-trigger vLLM) and
  jobs/klein-pair-watch.sh (1 h backstop; fired twice tonight on the
  failed runs and restored vLLM both times — the design working as
  intended).
- **VERDICT (the recorded answer for task-015): Path A + B fused.** v269
  native `groups` provides the gateway authority, eviction policy, and
  co-residence (swap:false); the joint supervisor cmd provides the shared
  lifetime (one request raises both, one stop tears both down, one TTL).
  v269 alone cannot start two processes from one request, so the script
  layer is mandatory, not a workaround.
