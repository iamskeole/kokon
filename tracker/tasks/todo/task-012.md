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
