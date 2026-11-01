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
- One shared engine deploy dir with two serve scripts (klein/qwen) vs two
  deploy dirs — decide at execution (lean: one build, one dir, two
  scripts; the weights differ per pipeline).
