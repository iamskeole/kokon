# task-012 — audit: the qwen engine fork's 21 commits (keep vs retire vs upstream)

## goal
Enumerate every commit in the qwen-path engine fork (the host's
`__mess/sdcpp-fork`: `0a06712` → HEAD, 21 commits) and classify each:
**retire** (in-engine LLM text machinery — KV cache, on-demand LM head,
BPE decoder, sampler, `/sdcpp/v1/text_gen`, PUP slot, vision conversion —
all replaced by the sidecar architecture), **keep-ported** (a fix the
pipeline genuinely needs that upstream doesn't have → candidate for an
upstream PR, per the running rule), or **moot** (superseded by the named
pin's own code).

## acceptance criteria
- An audit table committed under `tracker/` (one line per commit: hash,
  title, classification, one-line rationale).
- A conclusion section: is the qwen pipeline safe to run on the **same**
  stock build as klein (the named pin / `0a06712` on that line) with zero
  of the 21 commits, or is there an explicit exception list?
- Any `keep-ported` item gets a follow-up note (which upstream PR it
  becomes — that PR is made only if/when the owner says so; nothing is
  forked for this).

## blockers
- None (read-only analysis on the archived fork tree).

## decisions
- The audit table is the deliverable; classification disputes get recorded
  here, not resolved by fiat.
- 2026-11-03: the count is 20, not 21 (`git rev-list --count 0a06712..kokon`
  = 20; the brief's 21 was an off-by-one at write time). The audit table
  covers all 20.
- 2026-11-03: result = **17 retire / 3 moot / 0 keep-ported**; conclusion:
  the qwen pipeline is safe on the stock pin (`290148d` — the same build as
  klein, which is one day *ahead* of the fork's base) with zero of the 20
  commits, and there is no exception list. No upstream PR candidates arise
  (running rule: a PR only if the owner says so). Deliverable:
  `tracker/audits/qwen-fork-audit.md`. The 3 moot commits (01c970b, 301d4e0,
  ba47a5d) are merge/build shims whose end state *is* the stock code — the
  final `execute_segment` matches stock line-for-line and the final ggml pin
  is unpatched upstream. Watch-outs for task-013's cutover (ggml pin drift,
  in-engine PUP / `/sdcpp/v1/text_gen` disappear, sidecar PUP per pipeline)
  are recorded in the audit.
