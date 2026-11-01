# task-017 — upstream PR / merge / philosophy (the last task)

## goal
Re-baseline the parked `pr-external-engine` branch (10 DCO commits,
never pushed) against upstream + the v2 work. The owner then decides
what we contribute upstream (the unbundling, external-engine mode, the
picker, possibly the Qwen3.5 engine support) vs what stays ours
(`tracker/`, the AGENTS fork preamble, the host layout). The
philosophy conversation (fork vs upstream PR — why it is shaped the way
it is) happens here, at the end, with the whole picture in hand.

## acceptance criteria
- The PR branch rebuilt: upstream + contributed files only (no
  `tracker/`, no fork preamble); proposal/discussion text drafted; the
  branch pushed or parked per the owner's call.
- If anything merges upstream: this repo re-baselines onto it, and the
  fork shrinks to whatever diff remains (the mission is to keep getting
  smaller).
- The decision recorded here and in `STATUS.md` (open item G8 closes).

## blockers
- task-016 (the final state first — you describe what works before you
  propose it).
- user approval required, never autonomous.

## decisions
- (deferred to the owner — their explicit "when we're done" item)
