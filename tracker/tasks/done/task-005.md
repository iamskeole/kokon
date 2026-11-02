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

---

## Done (2026-11-02)

## acceptance evidence

- **Schema 12 → 13 (additive).** `config.rs`: new `VlmEndpoint`
  (`entrypoint` + `model` + optional `apiKey`); the per-model entry
  (`ExternalModel`) gains a `gateway` override (None = the service-wide
  one), a `vlm` sub-entry (None = no upsampler) and a `promptBundle`
  reference; the active engine (`ExternalEngine`) gains the same `vlm`
  and `promptBundle` so the switched-in pipeline carries them. All new
  keys default empty — a schema-12 file (and the schema-12 `flavors`
  alias) still reads clean; the first save writes schema 13 with the
  keys only when set.
- **Resolution, no constants:** `resolve_prompt_bundle(bundle, base)`
  (pure; tested against a temp dir): "" = none, absolute path as is,
  relative-with-separators joined to the data dir, a plain id = a repo
  asset dir under `<base>/assets/` (the asset dirs land in task-007);
  `ExternalModel::engine_gateway(service)` = own override else
  service-wide; `ExternalModel::prompt_bundle_path()` = the resolver
  against the deployment's data dir. Every URL an engine or VLM call
  needs now comes from the config — a fresh deployment is config-only.
- **Unit tests (the three pipelines):** klein (gateway override + keyed
  VLM + bundled set), qwen-full (service-wide gateway + keyless VLM +
  path override), qwen-turbo (service-wide, no upsampler, no bundle) —
  `pipelines_resolve_engine_and_vlm_from_config`; plus
  `schema_13_adds_vlm_and_prompt_bundle` (read/save/round-trip).
- **UI:** `ExternalEngine`/`ExternalModel` types gain the same surface;
  `modelRequest` now rides the gateway override, VLM sub-entry and
  prompt bundle onto the `/models/switch` body (the endpoint contract is
  unchanged — it stores the full active engine as before); the
  `modelRequest` tests cover override-wins and ride-along.
- **Test results:** `cargo test -p qls-core --lib` 302/302 (host env:
  `OPENSSL_DIR=/home/linuxbrew/.linuxbrew/opt/openssl` +
  `LD_LIBRARY_PATH=.../openssl/lib` for the reqwest native-tls stack);
  vitest 160/160; `tsc --noEmit` clean; MCP untouched (no
  external-engine references). No cargo fmt (no rustfmt.toml on this
  host), hand-formatted.

## decisions (this task — the exact shape, as allowed)

- **The API key lives in the `vlm` sub-entry**, not as a separate
  per-pipeline field: the only call the service makes that can be keyed
  is the VLM call (the engine is reached through the keyless gateway in
  this stack; a keyed engine door later takes the same one-field
  pattern). Empty key = header not sent (a keyless llama-server sidecar
  is the common case).
- **`promptBundle` is a string** (id or path) on both the picker entry
  and the active engine; the id→`<assets>/<id>` convention is the only
  code-side mapping (a repo fact, not a deployment fact). task-007
  commits the klein BFL set and task-014 the qwen set; deployments
  override by path without touching the id.
- **The `/models/switch` API contract is unchanged** (a full
  `ExternalEngine` body): the UI just fills the new optional fields, so
  an older UI against a newer service (and vice versa) keeps working.
