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

## Remaining

- **Owner visual inspection** of the five renders (paths above).
- **Window G — rollback drill** (per `llama-swap-deploy/KLEIN-ROLLBACK.md`):
  fork scripts in, gateway restart, service `vlm` dropped (config backup
  taken), one klein job on the fork, forward again, one job on stock.
  The owner questioned the point of it (2026-11-03, post E+F) — the answer
  is in the session: the drill proves the *exit* works while it is cheap,
  including the forward direction, before it is needed at 3 a.m.; the
  alternative is to retire the rollback to "rebuild from the fork's git
  branch" and delete the one-step promise. His call decides which.
