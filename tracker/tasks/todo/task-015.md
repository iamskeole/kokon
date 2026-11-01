# task-015 — gateway: generic N-models-per-job (the "matrix")

## goal
Generalize the klein pair (task-009) into a first-class configuration for
arbitrary **N models per job** in llama-swap — one lifetime, one TTL,
on-demand — the "matrix" the owner flagged (DiT + VLM now; more later).
Both the klein and the qwen jobs consume it.

## acceptance criteria
- The pinned llama-swap config (v269 or its upstream-PR'd successor)
  expresses an arbitrary model set per job; klein and qwen both use it;
  on-demand semantics (TTL, eviction, reload) unchanged.
- If v269 cannot express a set: an **upstream PR** to llama-swap
  (stock-first — never a fork). The PR branch lives on the host
  (`backends/llama-swap/` at the pinned tag + one PR branch — it is not
  part of the public repo) and its status is recorded here.
- Deploy-dir doc: how to define a new job's model set (the thing the
  fresh-box doc, task-016, will point at).

## blockers
- task-009 (the working pair first — this generalizes whichever path it
  took: native config or script workaround), task-001 (v269 base).

## decisions
- (open: native v269 vs upstream PR — decided by the schema check,
  recorded here with the reason)
