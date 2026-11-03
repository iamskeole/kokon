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

## Remaining (needs GPU windows — owner awake, GO requested)

- **Window E — t2i stability**: 3-4 t2i jobs through the PRODUCTION service
  (`kokon` container, its own token) against the pair now running PUP **Q8**;
  vLLM down for the window, self-healed at the end.
- **Window F — 3-box i2i acid test** (same window, after E): one edit job,
  three locked boxes (a delete, a move, an add) on a stand-in reference —
  boxes → table contract → PUP Q8 (sidecar) → stock engine → owner sees the
  three edits applied in the right places.
- **Window G — rollback drill**: per KLEIN-ROLLBACK.md — fork scripts in,
  gateway restart, service `vlm` dropped (backup of config.json taken first),
  one klein job on the fork, forward again, one job on stock. Result recorded
  in the doc's drill record.

Then: done-notes, move to done/, STATUS, commit.
