# task-009 — gateway: one job stands up {klein engine, PUP sidecar} with one lifetime

## goal
A klein generation job needs **two** models resident (the DiT engine and
the PUP sidecar) with one shared lifetime (on-demand load, one TTL,
clean teardown). Make the gateway express that — natively if v269's
config schema can, otherwise by the documented operational workaround.

## acceptance criteria
- **Path A (native):** v269 `config-schema.json` supports a set/group of
  models per upstream alias → configured so one on-demand load raises both
  and the TTL tears both down. Acceptance: one gateway request → both
  processes up; job completes; both gone after TTL.
- **Path B (workaround):** the klein serve script also starts/stops the
  sidecar (documented in the deploy dir, tested). Same acceptance, minus
  the native-config claim.
- The choice is recorded in this file's decisions (it is the input to
  task-015, which generalizes whichever path wins).

## blockers
- task-001 (v269), task-002 (engine), task-004 (sidecar).

## decisions
- (to be recorded at execution: native vs workaround, and why)
