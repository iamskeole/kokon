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
- **Error shape for the UI status pill (the open knob):** `code:
  "upsampler"` (snake_case `ErrorCode::Upsampler`, HTTP 503), message
  `"The prompt upsampler at <entrypoint> is not answering (<reason>)"`
  with `<reason>` = `no server is listening` / `it took too long (a
  cold load can take a couple of minutes)` / `it answered <status>:
  <200-char body excerpt>`; one suggestion: wait a minute and retry,
  check the gateway log if it persists. The UI renders it like any
  other job error.
- **Accepted v1 degradations (noted, not defects):** no live streaming
  on the VLM path — the UI gets the finished answer in one update; a
  user cancel during an in-flight VLM call is honored when the call
  ends or hits its 300 s timeout.

## Done (2026-11-02)

### What was built
- `crates/qls-core/src/vlm.rs` (new): `VlmClient` — OpenAI-compatible
  `/v1/chat/completions`, non-streaming v1. One turn = [system, user]
  with user parts = the reference images first (`image_url` with
  `data:<mime>;base64,`) then the text — mirrors the engine API's
  "images shown before the prompt" contract. Sampling maps
  temp/top_p/top_k/presence_penalty; `seed` only when >= 0; the
  `Authorization` header only when `apiKey` is non-empty; f32->f64
  noise removed by rounding to 4 decimals.
- `error.rs`: new `ErrorCode::Upsampler` (HTTP 503) per D-FB.
- `engine.rs`: `enhance_prompt` resolves the VLM from
  `cfg.external_engine.vlm` (the task-005 schema-13 surface). When
  set, the VLM replaces the engine's `text_gen` at every call site —
  the compose-label loop, the main call, the degenerate-answer retry.
  D-FB: an `Upsampler` error hard-fails the job at all of them
  (dedicated match arms placed before the generic soft-fail "your
  prompt was used as written" arm — the first implementation missed
  that tail arm and a down VLM silently fell back to the raw prompt;
  the unit test caught it). Non-VLM pipelines are byte-identical to
  before, including the soft-fail fallback and the
  `caps.text_generation` gate.
- Tests: `a_configured_vlm_replaces_the_engine_llm` (wire format:
  model, system first, image part before text, max_tokens 3584,
  seed, no auth header when keyless), `a_down_vlm_hard_fails_the_job`
  (the D-FB shape end to end), plus the vlm.rs unit tests for the
  unreachable/timeout/500 error mapping.

### Acceptance evidence (live window, 2026-11-02 15:01-15:04)
A scratch service instance (schema-13 config: klein external engine
+ `vlm: {entrypoint: http://127.0.0.1:39175, model: q35}`) with the
PUP sidecar and the no-PUP klein engine co-resident on the one 24 GB
GPU (9012 + 7696 MiB):
- **Job A (t2i, seed 42): `done` in ~10 s.** `enhanced_prompt` is the
  sidecar's 184-token caption; service log: `prompt enhancer: 184
  tokens in 2.9 s (stop, seed 42)`; the sidecar log carries the
  matching chat turn (753 prompt tokens). PUP tokens from the
  sidecar — the acceptance bar.
- **Job B (box edit: generated photo + locked one-row table,
  seed 7): `done` in ~10 s** — the table contract intact (sidecar
  answer: 126 tokens, service log seed 7).
- **Job C (sidecar killed first, seed 99): `failed` with
  `upsampler: The prompt upsampler at http://127.0.0.1:39175 is not
  answering (no server is listening)`** — D-FB proven live, not just
  in the unit test.
- Test battery after the change: cargo 308/308, vitest 160/160, mcp
  35/35 + 3 skipped.
- Window hygiene: gateway config restored and verified, vllm
  reloaded by the job itself (~50 s), zero owner intervention. The
  window-only `sd-cpp-serve-klein-nopup.sh` was deleted after the run.

### Findings (input for task-009/010)
- Co-residency of the PUP sidecar and an image engine fits the 24 GB
  card comfortably (16.7 GB) and both load in ~10 s warm.
- The scratch service intermittently failed to *deliver* HTTP
  responses (three POST /jobs, one GET /jobs/<id>) while the request
  itself was fully processed — only inside busy GPU windows; a
  quiet instance hammered 50 times was 100% clean. Not a blocker
  (the window job now recovers the job id from the list endpoint),
  but worth a look when task-008 touches the deployed service.
- Host-env note: running the built service or test binaries from a
  non-interactive shell needs `LD_LIBRARY_PATH=/home/linuxbrew/
  .linuxbrew/opt/openssl/lib` (libssl.so.4) or they die with exit
  127 — this host has no system OpenSSL 4.
- The live container config still needs no change for this task:
  `vlm` is unset there, so the running service uses the engine's
  in-process PUP exactly as before. The cutover to the sidecar in
  the live config is task-009's job set.
