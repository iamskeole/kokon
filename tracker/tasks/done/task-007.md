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

## done — 2026-11-02

Acceptance, all met:

- **Assets committed**: `crates/qls-core/assets/pup/` — `system_prompts.json`
  (the four system prompts), `chat_template.jinja`, `tokenizer_config.json`,
  `preprocessor_config.json`, `processor_config.json`, `config.json` (copied
  from BFL's `flux-3-image-klein-pup` repo, commit `3f3edb52…`, taken
  2026-11-01, byte-verified against the repo's tokenizer hash), plus
  Kokon-authored `request_format.json` and `LICENSE.md` (Apache-2.0, source +
  commit, per-file sha256 table, contract-silence note).
- **The 20 MB `tokenizer.json` was deliberately NOT committed** — repo
  large-file policy; the sidecar tokenizes from its GGUF (the repo's copy
  stays in `models/gguf/BFL/flux-3-image-klein-pup/`, byte-identical, hash
  `06b95093…` in both provenance files).
- **Runtime load**: `pup.rs::load(dir)` — per-job, so a dropped-in
  replacement applies to the next job; the engine resolves the folder from
  the pipeline's `promptBundle` (task-005 surface) and loads before
  enhancing; a BFL pipeline without its bundle is a clean `InvalidConfig`
  failure (there is no compile-time fallback anymore).
- **No BFL strings in the binary**: the four system prompts *and* the two
  request-line formats (the t2i canvas line and the i2i scaffold — also
  moved into the bundle as `request_format.json`; they were the last
  hardcoded BFL text) now come from disk. `strings` on the built
  `kokon-service`: 0 fragments for all six texts.
- **Never echoed**: the task-006 window's job logs, sidecar log and digest
  grepped for distinctive fragments of all six texts: 0 hits. The service
  log carries token counts and its own stage lines; the sidecar log carries
  llama-server token counts.
- Tests: the loader hash-compares against the committed file (four prompt
  hashes + two format hashes); error paths name the file/key and carry no
  prompt bytes; cargo 313/313, vitest 160/160, mcp 35/35 + 3 skipped.

Decisions (final):

- `request_format.json` is Kokon-authored (marked as such in LICENSE.md);
  it closes the last compiled-in BFL text by data.
- An in-process (non-gateway) BFL-named model now hard-fails at enhance —
  it has no bundle source. In this fork every BFL engine is external/
  gateway-routed, so that path never meets a BFL model; the guard is
  dead-code defense (commented in code).
- Transitional: the live container still runs 0.11.1 (constants in the
  binary) until task-008 deploys this build, the config with `promptBundle`
  set, and the bundle in the service's data-dir assets folder.
