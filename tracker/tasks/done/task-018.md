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

## done — 2026-11-04 (as task-015 — the two files were the same work)

This file was the original parking of the matrix-router question (from
the task-013 era); task-015 ("generalize the klein pair to N-models-
per-job config") is the active version of the same subject and was
worked to completion on 11-04. Resolved here so the queue drains.
AC mapping:

- *Written comparison, every behavior, differences called out* →
  `tracker/audits/matrix-vs-groups-018.md`: per-requested-model table
  (groups vs matrix), the three delta classes (vLLM reclaims a qwen
  pair — the nudge-404 class; klein evicts a qwen/creative set; creative
  evicts a resident image pair) and the one deliberately dropped
  behavior (the klein/qwen eviction asymmetry — matrix swaps are
  symmetric; vLLM protection preserved by set membership instead of
  group flags).
- *matrix-candidate drafted & validated, or decision recorded* →
  exceeded: `config.yaml.stock-015` passed `-validate`, is **live since
  11-04 06:38**, and the groups config remains one copy back
  (`stock-014`).
- *Owner's call on the verification window* → exercised via the
  full-autonomy directive: the m015 window ran (255 s; reload, vLLM
  revival, klein protect-vllm refusal, a production turbo job with the
  9B door's quant proven from `/proc`, the vLLM reclaim of a resident
  qwen pair, clean teardown; `jobs/m015.log` + `/tmp/m015/digest.txt`).
- *STATUS updated* → G6 + WP4 lines.

Replay evidence: 4,608 (target × running-subset) cases through a
faithful port of v269's `groupSwapper` and the real matrix solver —
0 invariant violations, 1,262 eviction deltas / 254 unique families,
100% strict supersets (`/tmp/q15-replay.txt`; harness source
`llama-swap-deploy/refs/m015-solver-replay/main.go`).
