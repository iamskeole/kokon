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
