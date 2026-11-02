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
- **Live proof pending the owner's GO** (night of 2026-11-02, owner going
  to bed): the v2 acceptance window is written and every guard is
  verified in isolation, but it has NOT been run. When run, vLLM (and thus
  the agent's LLM) is down ~10-12 min (clean gateway stops only) and comes
  back through the gateway before the digest. The window is announced in
  chat before it starts; an owner nudge at any point during it takes the
  GPU back (veto) — the window retries, or FAILs on budget.

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
     min via the stable deploy's own supervisor: vllm-serve.sh is a
     systemd-run loop (the new vllm serve's parent is systemd, not the
     gateway, and no gateway re-run was logged — the wrapper restarts its
     child while the gateway's cmd stays alive).
  Rule born of #2: **unit-test guard probes in isolation before running
  any script with a kill path** (the fixed probe was verified to detect
  vLLM up first; the live gate test then refused cleanly after the 30 s
  grace and left vLLM's pid unchanged).
- **Config edits that change routing bounce loaded models** (reload =
  cmdStop, no rehydrate). Hence: edit routing config with vLLM down, or
  expect one clean bounce + auto-revival (both observed working tonight).
- **Artifacts on the host (all syntax-checked; only the groups config is
  live, and it is harmless):** config.yaml (+ .orig) carries
  `groups.kokon = {swap:false, members:[kokon-klein, kokon-pup]}`;
  deploy/klein/sd-cpp-serve-klein-stock.sh is the joint supervisor (gate →
  adopt-or-raise both → co-supervise; file log supervisor.log for
  forensics); deploy/klein/sd-cpp-stop-klein-stock.sh stops both (the
  cmdStop); the live pup-serve.sh (/home/p/ml/backends/) has the policy
  gate before its old evict; jobs/klein-pair-test.sh v2 does NO direct
  kills — one request raises the pair, the nudge-collision loop re-requests
  it, job A t2i + job B box-edit run, the 120 s TTL check expects BOTH
  down together, a respawn watch follows, and teardown unloads both
  aliases OUT OF THE TABLE and revives vLLM via the gateway trigger;
  jobs/klein-pair-watch.sh is the 1 h backstop (pair kill by pid/port,
  unload out of the table, gateway vLLM trigger, FAIL marker).
- **Pending live proof (when the owner says GO):** one request → both up;
  job A done; job B done with the bbox table intact; the TTL takes both
  down together; the respawn watch; vLLM back through the gateway before
  the digest; ports clean; policy still protect-vllm.
