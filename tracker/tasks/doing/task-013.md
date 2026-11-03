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
  `/tmp/kleenq`): canonical vLLM stop → swap to `config.yaml.stock-013`
  (vLLM stays in the registry; the veto loop keeps any daemon-resurrected
  boot down, as proven in the kleinrb2 attempt-2 window) → **leg A**: fork
  engine on :39160 (the gateway's new stock supervisor *adopts* it —
  protocol-level port check) with the service on the pre-013 config
  (no vlm entries → in-engine PUP = today's production) → 3 t2i jobs,
  fixed seeds → **leg B**: stock engine (fresh start) + sidecar, service
  on the patched config (per-pipeline vlm → sidecar) → same 3 t2i (same
  prompts+seeds) + 1 edit/i2i job (reference image from the gallery —
  proves the stock vision path and the sidecar's image parts) →
  teardown → vLLM revived canonically **last** → DONE marker.
  Internal deadline 110 min; watcher 150 min cap / 15 min silence;
  `wait-marker.sh` blocking wait as the turn's last call, cap 180 min.
  The owner's pi (qwen-direct) turns fail while vLLM is down — expected,
  the marker revives it.
- **AC3 resolved as empty**: the task-012 audit found zero keep-ported
  commits (17 retire / 3 moot), so there is no exception list to resolve
  or park. (Addendum: the audit's "in-engine PUP is unused by the
  service" claim was wrong for qwen — qwen used it; the sidecar above is
  its replacement, recorded in the audit addendum.)
