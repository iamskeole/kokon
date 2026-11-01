# task-011 — klein cutover to stock + 3-box i2i acid test

## goal
Production klein = **stock** engine (the task-002 named build, no PUP
flags) + **sidecar** PUP (the task-010 winner) + the service (task-008
image). The forked engine deploy retires to `__mess/` with its rollback
dir kept and the one-step rollback documented. Then run the acid test: a
3-box edit job (the layout-box contract under the new wiring).

## acceptance criteria
- The production serve path references only the stock deploy (no fork
  binary anywhere on the live path — grep the serve scripts).
- Klein t2i stable on the new stack (several jobs, owner validates).
- **3-box i2i acid test passes end-to-end**: boxes drawn → table contract
  → PUP (sidecar) → stock engine → the owner sees the three edits applied
  in the right places. This is the critical path: the DiT/conditioner
  weights are byte-identical to today, so any regression here is in the
  caption path (the only variable this cutover introduces).
- Rollback drill: once, for real — back to the fork deploy, one klein job,
  then forward again.

## blockers
- task-002, task-004, task-008, task-009, task-010.

## decisions
- None expected; if the acid test fails, this task becomes the debugging
  task (record the failure mode here before changing anything).
