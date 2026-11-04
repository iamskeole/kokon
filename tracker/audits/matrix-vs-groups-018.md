# matrix vs groups — the box-wide routing comparison (task-018)

**Outcome: switched.** `config.yaml.stock-015` (the matrix router) has been
live since 2026-11-04 06:38; the groups config remains one copy back as
`config.yaml.stock-014`. This file is the written comparison the task
asked for: every routing behavior of the old groups config expressed in
matrix terms, the differences called out, and the evidence.

Provenance note: task-018 is the original parking of this question (the
task-013 two-door fix, 2026-11-03); it was worked to completion as
task-015 (the "N-models-per-job" generalization — same subject). The
per-behavior replay that follows is task-015's 4,608-case harness
(`/tmp/q15-replay.txt`; harness source `llama-swap-deploy/refs/
m015-solver-replay/main.go`): a faithful port of v269's `groupSwapper.
EvictionFor` and the real `matrixdsl.Compile` + `Program.Solve` run over
all 9 models × all 512 running subsets each.

## The two configurations

**Groups (stock-014, the baseline):**

| group | members | flags |
|---|---|---|
| `kokon` (klein) | kokon-klein, kokon-pup | swap: false, exclusive: false |
| `qwen-t` | kokon-turbo, kokon-pup-qwen | swap: false, **exclusive: true** |
| `qwen-f` | kokon-full, kokon-pup-qwen-f | swap: false, **exclusive: true** |
| *(none)* | qwen3.8-27b, creative-fast, creative-slow | ungrouped → default group |

**Matrix (stock-015, what went live):**

```yaml
routing:
  router: {use: matrix}
  settings:
    matrix:
      vars: {k: kokon-klein, p: kokon-pup, qt: kokon-turbo, pq: kokon-pup-qwen,
             qf: kokon-full, pqf: kokon-pup-qwen-f, v: qwen3.8-27b,
             cf: creative-fast, cs: creative-slow}
      sets:
        klein:  "k & p & v"
        qwen-t: "qt & pq"
        qwen-f: "qf & pqf"
        small:  "(cf | cs) & v"
```

All nine models are leaves of at least one set (none is an orphan — a
model in *no* set would evict everything on request, the one trap).
`evict_costs` left at the default 1; ties break by definition order.
vLLM is a leaf of **two** sets (klein, small) — shared membership is the
expressibility the groups router lacks (a member of two groups is a
config error there).

## Behavior, per requested model

Rules of the group router (v269 `group.go`): a request evicts a running
model when (same group AND the target group swaps) OR (different group
AND the target group is exclusive AND the running group isn't
persistent). Rules of the matrix: the request keeps every running model
in some set that contains it, evicts the rest, cheapest first; a
request for an already-running model evicts nothing (fast path).

| requested | groups: evicts | matrix: evicts | same? |
|---|---|---|---|
| kokon-turbo / kokon-pup-qwen | everything outside qwen-t (vLLM, creatives, klein pair, the other flavor) | everything outside {turbo, pup-qwen} | **identical** (zero deltas in the replay) |
| kokon-full / kokon-pup-qwen-f | everything outside qwen-f | everything outside {full, pup-qwen-f} | **identical** |
| kokon-klein / kokon-pup | **nothing** (non-exclusive) — a resident heavy tenant is handled by the supervisor's policy gate, not the router | everything outside {klein, pup, vLLM} — i.e. any qwen pair, any creative | **differs (delta class 2)** |
| qwen3.8-27b (vLLM) | **nothing** (ungrouped targets never evict) — a boot next to a ~20 GB pair is the OOM/404 landmine | everything outside {klein, pup, creatives, vLLM} — i.e. any resident qwen pair | **differs (delta class 1 — the headline)** |
| creative-fast / creative-slow | **nothing** | everything outside {creatives, vLLM} — i.e. any resident image pair (26B+31B don't fit next to a 20 GB pair) | **differs (delta class 3)** |

## The differences, called out (not hand-waved)

1. **vLLM reclaims a resident qwen pair** (delta class 1; the nudge-404
   failure class). Under groups, a vLLM request with a qwen pair loaded
   evicted nothing; the 23 GB boot then collided with the ~20 GB pair —
   the exact 404 storm that cost the overnight nudge cycles. The matrix
   evicts the pair first (cheapest set containing vLLM), vLLM boots
   clean. **Live-proven**: the m015 window's final phase fired a vLLM
   request at a resident qwen pair; journal
   `matrix: model=qwen3.8-27b set=klein … evict=[kokon-pup-qwen kokon-turbo] … cost=2`,
   ports freed, 1-token 200 through the gateway.
2. **klein evicts a resident qwen/creative set** (delta class 2). Under
   groups the klein group was non-exclusive: a klein request next to a
   qwen pair evicted nothing and OOMed (the gate only knows vLLM — it
   reads the policy file and the two heavy-tenant ports, and
   protect-vllm refuses; a qwen tenant is *not* in its refusal list, so
   it would have tried to load into a full GPU). The matrix evicts;
   klein and qwen still never coexist (neither set contains both), just
   cleanly.
3. **creative evicts a resident image pair** (delta class 3). Same
   shape as 2, for the 26B/31B creative tenants.
4. **The one behavior dropped, deliberately**: groups was *asymmetric*
   — klein never evicted, but qwen evicted klein. The matrix has no
   per-direction eviction (a set is a set); the klein request now evicts
   like any other. Recorded in task-015 as an accepted improvement
   (clean swap in both directions instead of an OOM landmine in one);
   it was the only non-expressible fragment of the old table, and the
   vLLM-protection that the asymmetry carried is preserved *by set
   membership instead of group flags* (vLLM sits in the klein and small
   sets, so no klein/creative request evicts it — the replay verifies
   this on every case).

## Evidence

- Replay: **4,608** (target × running-subset) cases through both
  solvers. **0 invariant violations** (protect-vllm, qwen-evicts-
  everything, pair co-residence, fast path — per case). **1,262**
  eviction-set deltas collapsing to **254 unique running-set families**,
  **100% strict supersets** of the group eviction set, 0 subsets, 0
  crossovers — every difference is "the matrix evicts more of a set
  that cannot fit", never less. Raw lines: `/tmp/q15-replay.txt`.
- Live: the m015 verification window (255 s, all six phases — reload,
  vLLM revival, the protect-vllm refusal under the matrix, a production
  turbo job with the 9B door's quant proven from `/proc`, the vLLM
  reclaim of the resident pair, clean teardown) — `llama-swap-deploy/
  jobs/m015.log` + `/tmp/m015/digest.txt`. The journal line quoted in
  delta 1 is the router's own decision log.
- Rollback: one atomic config copy back to `config.yaml.stock-014`
  (the groups version, still validated).
