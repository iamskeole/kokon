# Incident 2026-11-12 — Prompt+ button disappeared after a model switch

**Status: FIXED** (code `9f4eafb`, live as image `external-engine-0.12.0-9f4eafb` on the
dev box; no GPU work, vllm untouched).

## Symptom

After the dev-box relayout (bind-mounted deployment, fresh gallery) the owner reported
that the Prompt+ button in the composer was no longer visible when creating a new
image, and asked whether it was the PUP/gateway again.

## Root cause

Two compounding facts, neither from the relayout itself:

1. **The morning's model switch left the picked pipeline with no enhancer at all.**
   `POST /models/switch` replaces the active `externalEngine` wholesale with the
   picker entry. The owner's switch to `kokon-flux-image-3-klein` carried that entry's
   data: `vlm: null` (the BFL entry had none of its own — the shared
   `qwen3vl-8b` PUP entrypoint lived on the *previous* active engine and was
   dropped by the replacement) and `promptBundle: ""` (the klein entry was created
   without one; the qwen entries carry `pup-qwen`). Result: the job's upsample routing
   (`engine.rs`) found no VLM and `caps.text_generation == false` (stock engines have
   no text endpoint) → no enhancer, and the `/status` field the UI's Prompt+ switch
   reads reported `false`.
2. **`/status.text_generation` only mirrored the engine's own capabilities.** In
   external mode the stock engine never reports a text endpoint, so a pipeline whose
   only enhancer is a configured VLM always read as "no enhancer" — the switch was
   hidden whenever the picked model had a cached capabilities entry (the qwen-full
   model had none, so the browser's remembered value showed it instead: which is why
   the button's visibility looked random and "used to work").

The PUP sidecar and the gateway were fine throughout (key present, door verified:
keyed 200 / keyless 401; the PUP loads on demand per job, as designed).

## Fix (code, `9f4eafb`)

- `/status` reports `text_generation: true` whenever the active pipeline has a
  prompt enhancer this service can use — a per-model `vlm`, or the shared
  `externalEngine.vlm` for a BFL model: the same two sources the job's upsample
  routing picks between (`AppConfig::external_enhancer_configured`). The stock path
  (no external engine) is untouched.
- `absorb_secrets` grows two more protections, same bug family as the 2026-11-11 key
  wipe (the 0.12 UI type simply does not carry these fields):
  - a switch that arrives without a `vlm` inherits the stored endpoint (the VLM is
    deployment-level here: one PUP behind the one-door gateway serves every
    pipeline);
  - a save/switch that arrives with an empty `promptBundle` keeps the stored one —
    within the same model family only; across families the empty value stands and
    the job fails explicitly instead of writing wrong-family captions. An explicit
    non-empty bundle (a picker entry's own) always wins.
- Tests: `model_switch_inherits_shared_vlm_and_keeps_same_family_bundle`,
  `external_enhancer_configured_follows_the_job_routing`; qls-core 378 green, UI 228
  green, service compiles.

## Data fix (live config, via `PUT /config`)

All three picker entries and the active engine now carry the PUP VLM
(`kokon-pup` entrypoint, `qwen3vl-8b`) and the right family bundle:
qwen-turbo/full → `pup-qwen`, klein → `pup`.

## Verification (no GPU work)

- `/status`: `text_generation: true` with klein active (the button shows again, for
  every pipeline).
- Door: keyed 200 / keyless 401.
- Regression: a UI-shape `PUT /config` with empty keys left key, VLM and bundle
  intact (the new absorb rules working live).
- Lab models present on disk (MobileSAM pair + `birefnet_lite.onnx`).
- Boot log: 0 errors.

## Notes

- The 48-char gateway key re-appeared in this session's transcript (config dumps);
  optional rotation, as noted 2026-11-11.
- The prod-box build (task-027) must use the new tag: `docs/service-on-prod-box.md`
  now names `9f4eafb` as the current one.
