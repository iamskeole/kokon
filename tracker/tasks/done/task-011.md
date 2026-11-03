# task-011 — klein cutover to stock + 3-box i2i acid test

## goal
Production klein = **stock** engine (the task-002 named build, no PUP
flags) + **sidecar** PUP (the task-010 winner) + the service (task-008
image). The forked engine deploy retires to `__mess/` with its rollback
dir kept and the one-step rollback documented. Then run the acid test: a
3-box edit job (the layout-box contract under the new wiring).

## acceptance criteria
- The production serve path references only the stock deploy (no fork
  binary anywhere on the live path — grep the serve scripts).
- Klein t2i stable on the new stack (several jobs, owner validates).
- **3-box i2i acid test passes end-to-end**: boxes drawn → table contract
  → PUP (sidecar) → stock engine → the owner sees the three edits applied
  in the right places. This is the critical path: the DiT/conditioner
  weights are byte-identical to today, so any regression here is in the
  caption path (the only variable this cutover introduces).
- Rollback drill: once, for real — back to the fork deploy, one klein job,
  then forward again.

## blockers
- task-002, task-004, task-008, task-009, task-010.

## decisions
- None expected; if the acid test fails, this task becomes the debugging
  task (record the failure mode here before changing anything).
- 2026-11-03 (owner): run window G now, on this stack — not retire the
  rollback to rebuild-from-source (his visual verdict was already in hand).
  Attempt 1 ran it and died on the forward leg (see above); attempt 2 ran
  the forward leg only — forward-only by design (the fork leg is already
  proven, and the configs are back on stock).
- 2026-11-03: the handoff protocol was made durable (owner: "we're going in
  circles around this every task"). The blocking wait is now the canonical
  script `/home/p/ml/wait-marker.sh` (tested: DONE/FAIL/cap-hit paths) and
  MUST be the last tool call of the turn — bash tool timeout = cap + 120 s,
  where the cap exceeds the job's worst case (watcher cap + revival
  margin), never its expected runtime (a timeout firing while vllm is down
  kills the session for good; a turn with no pending wait never resumes —
  both observed 2026-11-03). vLLM revival is canonical `serve.sh start
  --no-wait` (the on-demand gateway curl drops — the trigger-loss modes
  above), and the marker is the last write, after the revival. A window
  that must keep a registry model down swaps it out of the registry or
  keeps a veto loop (the daemon brings dead registry models back — see the
  corrected mid-window attribution). Full version: `/home/p/ml/OPS.md`
  (autonomy protocol items 1, 3–7 + Gateway caveats); summary: this repo's
  AGENTS.md. OPS.md is not git — this entry is the in-repo pointer.

## File-side cutover (done 2026-11-03, pre-window)

- **Sidecar default = PUP Q8_0** (task-010 winner): flipped in both places —
  `deploy/klein/sd-cpp-serve-klein-stock.sh` (the pair supervisor's `PUP_MODEL`
  default) and `pup-serve.sh` (the standalone `kokon-pup` door). The A/B
  override (`pupab-override.env`) still wins when present.
- **Fork klein deploy retired to `__mess/`**: `sd-cpp-serve-klein.sh` +
  `sd-cpp-stop-klein.sh` moved (the archive copy is the current task-003-layout
  version, which supersedes the stale archived one — it points at the intact
  `__mess/sd.cpp/klein-engine/sd-server`, 2.7 MB, built 11-01, and the two
  rollback dirs `engine-1814814-rollback/` / `engine-NEW-0a06712-rollback/`
  stay). The QWEN fork scripts (`sd-cpp-serve.sh`/`sd-cpp-stop.sh`) stay at
  `backends/` — still the live qwen path until WP3 (task-012/013).
- **Rollback documented**: `llama-swap-deploy/KLEIN-ROLLBACK.md` — swap the
  alias's cmd/cmdStop to the `__mess/` scripts + gateway restart (the scripts
  are absolute-path and run from the archive); known consequence: the fork's
  in-engine 8B PUP does not fit alongside the Q8 sidecar, so a full
  pre-sidecar rollback also drops the service's `externalEngine.vlm` and
  restarts the container. `frontends/kokon/README.md` updated (image line was
  stale at 58b3588; Q8 note + rollback pointer added); `config.yaml.orig`
  resynced (the description-only edit bounced vLLM once, warm revival ~70 s —
  the documented behavior).
- **Grep-verified**: no fork reference (`__mess`, `--pup-llm`, `0a06712`,
  `sdcpp-fork`) in the klein live path (gateway config, stock serve/stop,
  pup scripts, vllm-serve, evict-kokon).
- **Production config confirmed maker-spec**: `externalEngine.sample` =
  `{cfg:1, sampler:euler, steps:16}` — `apply_sample_overrides` maps exactly
  these keys onto the wire `SampleParams` (the "wrong field names" bug found
  in task-010 was only my standalone case driver, which speaks the engine API
  directly); `vlm.entrypoint` is gateway-routed (TTL parity), `promptBundle`
  = `pup`.

## Windows E+F (done 2026-11-03 11:43 — the 4th run; three false-positive FAILs before it)

All five jobs ran through the PRODUCTION service (container `kokon`, :39151,
its own token) against the pair raised via the gateway — Q8 sidecar confirmed
in the supervisor's run line (`deploy/klein/supervisor.log`:
`pup=/home/p/ml/models/gguf/BFL/flux-3-image-klein-pup/model-Q8_0.gguf`), and
every job record shows the maker recipe (`model=model-Q8_0.gguf,
steps=16, cfg=1.0, sampler=euler`):

- e1 diver helmet: done 9.9 s, caption 1451 chars
- e2 lantern stall: done 10 s, caption 1812
- e3 clockmaker: done 9 s, caption 1379
- ref workshop workbench (fresh scene for the acid test): done 10 s, 1728
- acid 3-box edit on the ref (delete books / move radio / add plant): done 22 s, 2746

No caption had any repeated shingle (120-char, k=3). The acid caption cites
all three locked rows VERBATIM (obj_1 books src[360,330,600,630]→null; obj_2
radio src[80,30,400,330]→tgt[620,30,940,300]; obj_3 plant tgt[660,340,950,600])
and adds five scene rows (its own book_1 duplicate-delete at the books' real
coordinates, plus stool/workbench/compass/soldering-iron keeps) — none of
them contradicts the locked rows, the same benign pattern as window D's C2.
Renders: `/tmp/kleinprod/img-{e1,e2,e3,ref,acid}.png` and the production
gallery (`/gallery/2026-11-03/`, six files including one duplicate e1 from an
aborted run). **Owner's visual inspection of the five images is the open
verdict** (the agent's model is text-only; the images are the owner's to see).
Pair warm-raise 24 s; whole window 2 min 44 s including the full self-heal.

The three earlier FAILs were all my own harness bugs, not the stack — and
each one correctly exercised the full fallback (pair stopped, proxy up,
vLLM revived, marker written):

1. the Q8 proof read the wrong log file (`llama-swap-deploy/supervisor.log`,
   never written by this supervisor) and the wrong pattern (`run=` instead
   of `run: ... pup=...` in `deploy/klein/supervisor.log`);
2. `submit_job`'s python unpacked six names for five args;
3. `judge()` read the required-id list from `sys.argv[2]` (the job name)
   instead of `argv[3]`.

## Window G — attempt 1 (2026-11-03 12:00–12:20, `jobs/kleinrb-test.sh`)

The owner ran it (his call, per the question above). The backward half
PASSED, the forward half FAILED, and the drill's own restore returned the
stock state in seconds (`/tmp/kleinrb/`, digest `jobs/kleinrb-digest.txt`):

- 12:00 fork leg: gateway `cmd`/`cmdStop` swapped to the `__mess/` fork
  scripts, service `vlm` block dropped, container restarted. The first
  trigger did not become ready inside the 480 s budget — the engine was
  ready ~12:09:00, one second after the retrigger fired: an ~8.5 min
  cold start (klein DiT + 4B conditioner + 8B in-engine PUP). After the
  retrigger it was ready in 20 s; identity confirmed
  (`__mess/sd.cpp/klein-engine/sd-server` on :39170); `job_3b0d05ce1914`
  (t2i on the in-engine 8B PUP) came back OK — caption 1486 chars,
  `model-Q8_0.gguf`.
- 12:09 forward leg: stock config back, gateway restarted, pair
  triggered. At 12:17 the owner started vLLM by hand (`serve.sh`
  systemd scope — the box's documented manual revival; the gateway's
  on-demand load could not fire while the daemon was stuck in the pair's
  failing load loop, and the in-band reviver pi-debug-proxy had been
  down by drill design since 12:00:20). The job's veto evicted via a
  GATEWAY UNLOAD — a no-op against a process the gateway had never
  tracked — so the supervisor's `protect-vllm` gate refused every pair
  start (`deploy/klein/supervisor.log` 12:18:37–12:20:02: "refused
  cleanly after 30 s grace"), the 3-veto budget ran out, and
  `kleinrb-restore.sh` ran: stock config, service `vlm` back, proxy up,
  gateway restarted at 12:20:33 and adopted the hand-started vLLM
  (journal: "<qwen3.8-27b> Health check passed on http://127.0.0.1:18020/health").
- The gateway was never broken. The "unresponsive" window for pi users
  was (a) pi-debug-proxy down 12:00:20→12:20:38 by drill design, and
  (b) the daemon mid-kokon-load-loop, which blocks a qwen on-demand
  load (one GPU, one load pipeline; qwen is pure on-demand, no
  supervision). Post-restore both the :8081 and :18021 paths serve real
  1-token completions (verified 12:31 UTC, key-authenticated, both
  returned a chat completion from `vllm-0.27.1`).

**Failure mode (what attempt 2 fixes):** the job assumed the only vLLM
reviver was pi-debug-proxy (which it stopped) and evicted via the
gateway — but a human on a terminal is a reviver too, and the gateway
cannot stop what it never started. The supervisor's `protect-vllm` gate
did exactly what it was built for; the drill's premise was wrong, not
the stack.

## Window G — attempt 2 (2026-11-03 12:48–13:03, `jobs/kleinrb2-*.sh`) — DONE

The forward leg, exactly as planned (the fork leg was proven in attempt 1,
so v2 changed no config): canonical vLLM stop → pair through the gateway →
one t2i job on the Q8 sidecar → teardown → vLLM revived → gateway adopted.
Digest: `jobs/kleinrb2-digest.txt`.

- 12:48:59 vLLM evicted canonically (3 s to a 1 MiB GPU); pair triggered.
- **Trigger-loss mode 2 discovered:** the 12:48:59 trigger was lost again —
  but this time not the post-restart config race (the gateway had been up
  14 min): the daemon was still in the qwen UNLOAD caused by the job's own
  canonical vLLM stop (journal 12:48:56 "upstream process exited
  unexpectedly"). A group trigger fired while another model's unload is in
  flight is dropped, not queued. The 12:58:00 retrigger (the 540 s no-pair
  timer) is what finally raised the pair — supervisor launches 12:59:16
  (294 s to healthy, cold: DiT + 4B conditioner + 9.5 GB Q8 PUP) and
  13:02:00 (10 s, page-cached), ready 13:02:10. Lesson for any future
  swap/trigger: poll for a fresh supervisor `run:` line after the trigger;
  never trust one fire-and-forget GET (both loss modes: right after a
  restart, and during an in-flight unload of another model).
- Mid-window, vLLM came up three times (health 200 at 12:58:36 / 13:00:15 /
  13:01:56; first scope boot 12:57:44) and the job's veto canonically
  stopped each within 20–25 s (veto 1/3 12:58:45, 2/3 13:00:21, 3/3
  13:01:56) — the "vLLM keeps bouncing between alive and dead" episode from
  the owner's side. Corrected attribution (verified later): the boots were
  the **gateway daemon's own vllm-serve.sh** re-running the dead registry
  model's cmd (the 12:57:44 `run-p381271` systemd scope is its signature;
  the daemon's journal was otherwise silent at info — load-trigger lines
  are Debugf, so the exact internal trigger is unproven, but the daemon is
  the initiator). It was NOT the owner's or this session's traffic:
  qwen-direct was verified truly direct (models.json baseUrl 18020/v1) and
  the pi-debug-proxy was down for the window's duration; an earlier session
  had misattributed it to gateway-routed owner messages. The procedural
  advice stands for that reason anyway: a gateway-routed session is a load
  trigger per message. The veto was doing exactly its job (the pair cannot
  share the card with a 23 GB vLLM; a vLLM boot alongside the ~20 GB pair
  OOMs — the failed manual boot the owner saw at ~13:00 was that). The
  contrast is the proof: attempt 1's window had qwen removed from the
  registry by the config swap and bounced nothing. A window that must keep
  a registry model down: swap it out of the registry (clean) or run this
  veto loop (works, but each cycle burns a 50–90 s boot).
- 13:02:25 pair ready (260 s from the retrigger); Q8 sidecar confirmed in
  the run line; **stock job `job_cd7fe63c378d` OK** — done in 9 s, maker
  recipe (seed=62, steps=16, cfg=1.0, euler, model-Q8_0.gguf), caption
  1220 chars, no repeated shingle; render in the production gallery
  (`/gallery/2026-11-03/130234-a-single-white-chess-knight-on-62.png`) and
  `/tmp/kleinrb2/img-stock.png`.
- 13:02:37–47 pair down, gateway restarted; 13:03:42 vLLM revived
  canonically (pid 387046; warm boot ~55 s — the compile cache makes the
  2026-11-02 16-min cold tail the exception, not the rule); 13:03:43 the
  gateway adopted it (health passed) and the proxy came up. DONE marker;
  the watcher never had to intervene; journal quiet since.
- Process note (mine, not the stack's): the launching agent ended its
  turn without the blocking wait on the marker — the protocol's form is a
  suspended wait, not an idle turn. It survived because no inference was
  needed mid-window (its model calls ride on the very vLLM the job was
  cycling — its turns only completed during the up-windows), but it was
  blind for the whole window. Fixed durably — see decisions above.

## Closed (2026-11-03)

All acceptance criteria met: the production serve path references only the
stock deploy (grep-clean); klein t2i is stable on the new stack (windows
E+F — incl. the owner's visual verdict: fine — plus the forward leg);
the 3-box i2i acid test passed end-to-end (the three locked rows applied
in the right places, verdict confirmed by the owner); and the rollback
drill ran for real in both directions — fork in + one fork job (attempt
1, `job_3b0d05ce1914`), forward + one stock job (attempt 2,
`job_cd7fe63c378d`). `KLEIN-ROLLBACK.md` updated with the measured
readiness times (~10 min fork cold, ~5 min stock pair cold) and the
trigger-loss caveats. Next: task-012 (the qwen fork audit).
