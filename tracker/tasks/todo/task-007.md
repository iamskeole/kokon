# task-007 — BFL prompts as repo data assets (never echoed)

## goal
The klein prompt bundle becomes committed data in the repo: the four BFL
system prompts (t2i / i2i / t2i_bbox / i2i_bbox — from
`system_prompts.json`), the BFL chat template (`.jinja`), the tokenizer
(`tokenizer.json`), and the vision preprocessor config. The service loads
them at runtime (replacing the compile-time prompt constants in
`enhancer.rs`). The assets are **never echoed** — not in logs, chat, docs,
or diffs-in-chat; they move around only via files on disk
(harness-cut-stream rule).

## acceptance criteria
- Assets committed (suggested home: `crates/qls-core/assets/pup/`) with a
  BFL license note (Apache-2.0) recording source repo + commit.
- `enhancer.rs` loads the bundle at runtime; a unit test asserts the loaded
  bundle matches the assets (hash-compare, never print).
- Grep the job log path: no prompt bytes appear anywhere.
- DCO commit.

## blockers
- task-006 (the runtime-loading path this feeds). Source material is on the
  host at the BFL research dir (copy it silently).

## decisions
- Asset location confirmed at execution (suggested above).
- If the BFL repo re-releases the prompts: re-copy + bump the recorded
  source commit (one-line note in the license file).
