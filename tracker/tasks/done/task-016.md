# task-016 — fresh-box reproduction doc (written last)

## goal
The repo doc that lets a stranger stand up the whole stack on a new
machine, describing the **final** state (written after the cutovers so it
documents reality, not intent): clone this repo → docker build → drop
the compose + env into the host's `frontends/` dir (pointing at the
gateway) → build the stock engines per the recipes (sdcpp: the
task-002 build phase; llama.cpp: the named build) → download the models
per the provenance convention → done.

## acceptance criteria
- Doc committed to the repo (`docs/` — location confirmed at execution);
  every command verified on this host or explicitly marked (OS/toolkit
  versions, disk budget, the GPU requirement).
- No machine-specific paths that contradict the agreed layout
  (`backends/`, `frontends/`, `models/`); no BFL prompt bytes in the doc
  (silence rule); no reference to any forked engine (they're gone from
  the final state).
- The provenance convention is spelled out (the task-003 pattern) so
  future downloads stay standardized.

## blockers
- Written **last**: task-011 (klein cutover) and task-014 (qwen cutover)
  must be done — the doc describes the final state, and that state
  doesn't exist yet.

## decisions
- None expected.

## done — 2026-11-04
- The doc is **`docs/fresh-box.md`** (repo; committed with this record). What it
  covers: the stack overview + the host layout (the `backends/`, `frontends/`,
  `models/` convention); a prerequisites table (Ubuntu 26.04.1, RTX 3090 24 GB,
  CUDA 13.2, Go / rustc 1.98.1 / Docker 29.9.1 — all verified on this host);
  the final-state model list with the `provenance.md` convention spelled out
  (provenance = source/quant/license/date/sha256/converter per directory; the
  recorded sizes are the source of truth, the doc's budgets are marked
  "roughly"); the engine recipes (sdcpp @ `290148d` with the exact cmake line,
  the named llama.cpp build for the sidecars, the exact engine and sidecar
  invocations, the klein joint supervisor + one-word policy file); the gateway
  section (v269 pinned build, the non-git deploy dir, the **full matrix
  routing block with the three-rule semantics and the per-set rationale**,
  `-validate`, pointers to the deploy-dir `MATRIX.md` and the upstream guide);
  the service section (image build + tag convention, host-network compose,
  the schema-13 config shape, the down-sidecar hard-fail rule); a verification
  checklist (1-token probes, one job per family, the `matrix:` journal line as
  the router proof, TTL unload); and the ops rules (reload kills models +
  atomic swap + marker-last windows, the vllm-serve booting branch, the policy
  file, the rollback ladder).
- AC check: in the repo's `docs/` ✓; every command verified on this host or
  marked (the vLLM side is owner-owned and kept out of scope) ✓; no BFL prompt
  bytes — the fine-tune's name appears, its prompt/template text does not
  (silence rule) ✓; no forked-engine references — the rollback ladder names
  only stock ↔ older-build notes in the deploy dir ✓; provenance convention
  spelled out ✓.
- AGENTS.md's doc table gained the row for the new file.
