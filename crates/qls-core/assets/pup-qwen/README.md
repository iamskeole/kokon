# Qwen prompt bundle (`pup-qwen`)

The enhancement prompts Kokon sends to the Qwen-side VLM (the qwen pipelines'
`promptBundle`, schema 13): four system prompts for the built-in enhancement
path — text-to-image, image-grounded edit, composition (step 2), and the
per-reference label request (step 1 of a composition).

## Provenance

These are the Kokon fork's own built-in Qwen prompts, extracted verbatim from
`crates/qls-core/src/enhancer.rs` on 2026-11-03 (task-014, D-QW); re-extracted
on 2026-11-06 with the 0.12 refresh (the edit prompt gained the coloured-marks
bullet) so the set is
repo data in the `pup` (BFL) pattern: one folder, loaded at runtime from the
deployment's data dir, swappable by dropping a replacement folder in place.
Their design and the A/B history behind the texts live in
`docs/engineering-notes.md` ("Prompt+"). They are Kokon's own text (derived
for the open Qwen3-VL model) — unlike the BFL bundle next door, which is a
third-party contract asset and whose text is therefore never committed here.

`system_prompts.json` keys:

| key             | used for                                            |
|-----------------|-----------------------------------------------------|
| `t2i`           | text-to-image enhancement                           |
| `edit`          | image-grounded edit / combine                       |
| `compose`       | new image from reference images (step 2 of 2)      |
| `compose_label` | per-reference label (step 1 of 2)                   |

The runtime (service) loads this folder for a non-BFL pipeline when the
pipeline's `promptBundle` names it; a missing or broken bundle falls back to
the built-in defaults in `enhancer.rs` (the Qwen path must never hard-fail on
its bundle — unlike the BFL path, where the captions are part of the model's
training contract).
