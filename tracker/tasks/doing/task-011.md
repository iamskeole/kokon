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

## Remaining

- **Owner visual inspection** of the five E+F renders (paths above).
- **Window G attempt 2 — the forward leg alone** (`jobs/kleinrb2-*.sh`,
  written 2026-11-03): the fork leg is proven above, so v2 changes no
  config and executes only what attempt 1 never reached — canonical vLLM
  stop (`serve.sh stop`: kill by pid whatever started it, wait until
  nvidia-smi reports the GPU free), stock pair up through the gateway,
  one t2i job on the Q8 sidecar (maker recipe), pair down, gateway
  restart (clean group state; attempt 1's first trigger was lost in the
  post-restart config-load race — /health is ready before the first
  -watch-config poll loads the model registry), vLLM revived canonically,
  proxy up, gateway re-adopting it (the adoption probe retries, so the
  same race cannot swallow it). Watcher backstop (1 h cap, 15 min silence); every
  failure path ends in `kleinrb2-restore.sh` (stock state + vLLM
  guaranteed). Owner impact: the qwen backend — and with it the pi
  sessions on it and OWUI chat — is down for the ~15–30 min window;
  the launching agent ends its turn in a blocking wait on the marker
  (autonomy protocol), the job revives vLLM as its last step.
- After v2 passes + the visual verdict: close this task (move to
  `done/`), update STATUS.md, and note in `KLEIN-ROLLBACK.md` that a
  fork rollback should expect ~10 min to engine readiness (measured
  8.5, incl. the 8B in-engine PUP load).
