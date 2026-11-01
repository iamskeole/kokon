# task-006 — service: VLM client replaces the in-engine PUP call (hard-fail)

## goal
A generic OpenAI-compatible chat client in the service
(`/v1/chat/completions`, image parts, non-streaming v1). `enhancer.rs`
calls the **sidecar** instead of the engine's `/sdcpp/v1/text_gen`. Per
**D-FB**: if the VLM is unreachable, the job **hard-fails** with a clear
diagnostic — no raw-prompt fallback (making the VLM work is our job).
Everything else in the enhancer orchestration — the layout-box contract
(detect/lock/reconcile the prompt-embedded table), the two-layer remove
backstop, the one-retry on degenerate answers — is **unchanged**.

## acceptance criteria
- Klein t2i through the service: PUP tokens come from the sidecar
  (provable from the service log / job metadata).
- A box-edit job works end-to-end (the box table contract intact — this
  guards the "don't start from scratch" rule: only the token source moved).
- With the sidecar down: the job hard-fails with a clear, actionable
  message (test covers it).
- `cargo test -p qls-core` green; DCO commit.

## blockers
- task-004 (a live sidecar to test against), task-005 (its entrypoint in
  config).

## decisions
- None — D-FB is resolved (hard-fail). The one open knob: exact error
  string/shape for the UI status pill (decide at execution, record here).
