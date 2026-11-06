# task-17a — MOSS-Image research (doc only)

## goal
Document how MOSS-Image (OpenMOSS: 5B DiT + Qwen3.5-9B VLM text encoder +
FLUX.2 VAE; Apache-2.0; released 2026-10-16) fits our external-engine
stack, so a future adoption is a config change, not a project.
**Doc only: no code, no GPU, no A/B windows** — adoption is parked until
MOSS is stable end-to-end (owner, 2026-11-06).

## acceptance criteria
- A research doc at `tracker/audits/moss-image.md` (the audit-file
  convention) with, all sourced and dated:
  - architecture and sampling facts: 5B DiT (dim 3072, 42 layers, 128-ch
    packed latents), flow-matching (50 steps / CFG 3 in the reference
    pipeline; 20 in the sdcpp recipe default), 4K-token prompts, native
    JSON layout (bboxes `yxyx`, absolute-centred; objects + rendered
    text), Base vs SFT (SFT = curated-data fine-tune, same interface);
  - the technical-report numbers (arXiv 2610.23374) vs Qwen-Image 2.1 /
    FLUX.3 / FLUX.2 — the owner's "neck to neck" claim, in the authors'
    own numbers;
  - sdcpp support at our pin (`290148d` — already contains it:
    `src/model/diffusion/moss_image.hpp`, docs, both recipes): t2i works
    through the recipe slots (DiT GGUF/safetensors + Qwen3.5-9B GGUF +
    external `tokenizer.json` + FLUX.2 VAE); **image editing does not** —
    leejet PR #2203 / issue #2238 open (community edit models exist but
    are not stock); the re-check trigger = the PR landing in a pin we
    adopt;
  - weight inventory with pins (URL, size, sha256 where obtainable):
    leejet + unsloth GGUFs (Base/SFT, the quant list), unsloth
    Qwen3.5-9B GGUF, FLUX.2 VAE — and where each would sit in the host's
    models convention;
  - the gateway recipe: the MOSS set in stock-017 shape (engine door +
    the VLM door it would use) and the service config it would be
    (`models` list entry — the picker treats it exactly like klein/qwen);
  - the enhancer question: first candidate = the shared 9B PUP door with
    a new `pup-moss` bundle (PLAN rule 2, the same pattern as
    `pup-qwen`); fallback = a Qwen3.5-9B door (the model MOSS was
    trained with as its encoder — the most in-distribution prompt
    writer); what an A/B window would test (planned, **not run** —
    parked);
  - the box-editor / layout seam: can our `bbox.rs` data serialize into
    MOSS's JSON layout prompt (nice-to-have; the same concept as the
    klein box table);
  - the speed reality: 20–50 steps vs qwen-turbo's 8 on the same card;
    the community distilled variants (2/4/8-step DMD) noted as a future
    speed path — not stock, not adopted;
  - **verdict: PARK** — no picker entry, no A/B, no downloads; the
    re-check happens only on the trigger (edit in a stock pin we adopt,
    or an owner call).
- A `STATUS.md` line; task file → `done/` when the doc lands.

## blockers
- none (doc only; the network is the whole requirement)

## decisions
- **2026-11-06 (owner):** MOSS is a picker-model candidate, treated the
  same as the others (a config `models` entry); enhancement through the
  shared flux PUP first, as for the others (their agent-skill "prompt
  enhancer" is their cheap way out, not ours); the editing capability
  arrives from stock sdcpp when it lands ("we already know how").
  **Everything parks until MOSS is stable end-to-end** — this task
  documents and parks; it does not adopt.
