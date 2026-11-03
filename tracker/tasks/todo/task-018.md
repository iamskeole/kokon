# task-018 — llama-swap: evaluate the matrix router as the box-wide routing

## goal
The box's gateway config currently expresses routing with the legacy
`groups:` block (klein pair: non-exclusive, policy-gate arbitrated; qwen
tastes: two exclusive flavor groups with one sidecar door each; vLLM,
creative: ungrouped → default group, one-at-a-time). The two-door qwen
layout (task-013 fix) is the boring low-blast-radius answer to the
per-request churn; the **matrix router** (`routing.router: matrix` in
llama-swap v269) could express richer co-tenancy — e.g. one qwen sidecar
serving both taste groups, or explicit evict costs across all tenants —
but it replaces the whole routing layer of the daemon (klein, creative,
vLLM semantics all re-declared as set expressions with `&`/`|`/`+name`,
subsets permitted, `evict_costs` tie-breaking, "only the requested model
starts").

Goal: read the matrix router semantics end-to-end (`internal/router/
matrix.go`, `docs/kb/guides/routing/groups-and-matrix.md`), write a
side-by-side of the current group config vs an equivalent-or-better
matrix config for THIS box's five model families, and make the call with
the owner: switch (its own config-swap window, klein canary + qwen both
tastes + a vLLM revive check) or keep the two-door groups (the matrix
idea stays documented as parked).

## acceptance criteria
- A written comparison (in this file or `tracker/audits/`): every routing
  behavior of the current config (what evicts what, when, per request
  pattern) expressed in matrix terms, including the klein group's
  deliberate NON-exclusivity (policy-gate arbitration vs vLLM) and the
  vLLM/creative one-at-a-time default-group behavior — with the
  differences called out, not hand-waved.
- A `config.yaml.matrix-candidate` drafted and pyyaml-validated (NOT
  swapped live) if the matrix route looks viable, or the decision
  recorded here if it does not.
- Owner's call on whether to take a verification window for the switch.
  If switched: the window proves klein canary, both qwen tastes (churn
  check as in task-013's qwenp window), vLLM revival; rollback = one
  config copy back.
- STATUS.md updated with the outcome either way.

## blockers
- task-013 (the two-door groups are the baseline being evaluated; the
  box must be in its post-window state), a spare GPU window if the switch
  is chosen.

## decisions
- (open: switch vs keep — data first, no vibes)
- Origin: task-013 two-door fix (2026-11-03). The owner's "common sense
  approach" ruling kept the box on the boring design; the matrix rework
  is parked here rather than smuggled into task-013.
