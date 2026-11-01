# task-005 — service: per-pipeline config surface (entrypoints + key + prompts)

## goal
The service config becomes fully pointable per pipeline:
`{ engine entrypoint, VLM entrypoint (optional), API key, prompt-bundle
reference }` for klein / qwen-full / qwen-turbo. Today the engine
endpoints are partially baked into the service; after this task nothing is
hardcoded — a fresh deployment is config-only.

## acceptance criteria
- Config read/write test passes (schema versioned — extending the existing
  `models` list pattern, schema 12 → 13; old keys still read).
- Unit tests: klein and qwen pipelines each resolve engine + VLM endpoints
  from config (no constants left in the code).
- `cargo test -p qls-core` green (host OPENSSL env; **no cargo fmt** — no
  rustfmt.toml on this host); DCO-signed commit.

## blockers
- None (no GPU needed).

## decisions
- Shape: the existing per-model config entry gains a `vlm` sub-entry
  (entrypoint + model alias) — confirm the exact shape at execution.
- Prompt-bundle reference: an identifier resolved to a repo asset dir
  (task-007) or an override path from config.
