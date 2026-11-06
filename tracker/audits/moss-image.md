# MOSS-Image — integration research into the external-engine stack (task-17a)

**Doc-only, per the owner's 2026-11-06 call: everything about MOSS parks
until it is stable end-to-end (image editing in stock sdcpp). This document
is the standing record so a future adoption is a config change, not a
project.** All sources fetched 2026-11-06.

## 1. What it is

OpenMOSS (Open-MOSS team; China). Released 2026-10-16; arXiv 2610.23374
("MOSS-Image Technical Report"). Two checkpoints, same architecture and
sampling interface:

- **MOSS-Image-Base** — the pretrained model.
- **MOSS-Image-SFT** — fine-tuned from Base on curated data (cleaner text
  grouping / layout per the authors' own comparison).

Weights **Apache-2.0**; the pipeline code and third-party components keep
their own licences (the text encoder is Qwen3.5 — see §2).

### Anatomy (from the HF `model_index.json` + component configs)

| Component | Identity | Notes |
|---|---|---|
| transformer | `MOSSImageTransformer` — 5B single-stream DiT: dim 3072, 42 layers, 24 heads, ffn 8192, **128-ch packed latents**, text_dim 4096, RoPE base 10000 | trained from scratch with a "carefully designed recipe" (report) |
| text_encoder | **`Qwen3_5Model` = Qwen3.5-9B** (hidden 4096, hybrid linear/full attention, `image_token_id` 248056) | a *vision* model: conditions the DiT and can see images; BF16 total ≈ 16 GB (4 shards) |
| tokenizer | Qwen2-based tokenizer (`tokenizer.json` + `chat_template.jinja`) | **different vocabulary** — sdcpp needs the external `--tokenizer` file (its embedded Qwen tokenizer won't do) |
| vae | **FLUX.2** (`AutoencoderKLFlux2`, 32-ch latents) | ≈ 237 MB (the "small" variant; 321 MB full in the Comfy-Org repack) |

### Sampling and prompting

- Flow-matching **Euler**; reference settings: **50 steps, CFG 3.0, shift
  5, uncond branch skips DiT layer 22** (zero-based), empty negative
  prompt, dimensions positive multiples of 16, BF16 (VAE FP32).
- The sdcpp recipe defaults (below) run **20 steps** — the authors'
  reference is 50; quality-vs-steps is a per-deployment knob
  (`--flow-shift`, steps).
- **Prompts up to 4K tokens**, three dialects: natural language, tag
  sequences, and a **native JSON layout**.
- The **JSON layout** (the conceptually interesting part):
  `summary { meta, canvas, caption }` + `items [ { label: text|object,
  caption, text?, bbox } ]`, with `bbox_coord_space: abs_centralized` and
  `bbox_format: yxyx` — bboxes are `[y_min, x_min, y_max, x_max]` in
  pixels **relative to the canvas centre**. Objects and rendered text are
  placed by position; per-item captions describe each element. The
  authors note layout control "also supports controlled editing" and
  "user-friendly layout tools".
- The authors' **prompt enhancer is an agentic module, not a model**: a
  "MOSS-Image Prompt Enhancer" *skill* (GPT-6-family agents, with a
  coordinate-conversion script; the report calls it "intention-aware
  agentic prompt enhancement"). The sdcpp pipeline itself: "**Prompt
  enhancement is not included.**"

### Benchmark claims (the authors', 2026-11-06)

- New evals: **LongLongTextBench (LLTB)** — long-form EN/ZH text
  rendering: SOTA among open-weight models, **96.3% average text
  coverage**; **RealLayoutArena (RLA)** — end-to-end layout-rich
  generation: ranks **second** in the end-to-end system comparison,
  **first** among backbones evaluated with shared enhanced prompts.
- The abstract makes **no direct win claim over Qwen-Image 2.1** on
  general text-to-image/editing; the full comparison tables are in the
  PDF (`paper/MOSS_Image_Technical_Report.pdf` in the repo) — not
  extracted here (no PDF toolchain on this host; doc-only task). The
  owner's "neck to neck" reading therefore **remains an A/B question on
  our card** — that A/B is part of what's parked (§6).
- Ecosystem heat (HF, 2026-11-06): Comfy-Org repack **5.27M downloads**;
  distilled **DMD variants (2/4/8-step)** up to ~316k downloads (they
  change the 50-step speed math, but none is stock); an active LoRA
  scene (NSFW/beauty/gouache/hires, 7k–65k downloads). Watch, don't adopt.

## 2. sdcpp support — our pin already has it

Our named slot (`backends/stable-diffusion.cpp` @ **290148d**, built
2026-11-02) contains the complete stock integration:

- `src/model/diffusion/moss_image.hpp`,
  `src/model/integrations/moss_image.cpp`, `docs/moss_image.md`, and
  server recipes `examples/server/recipes/moss-image-{base,sft}.json`.
  (Landed 2026-10-17, PR #2148; actively perf-tuned through 2026-11-05:
  SiTU activation #2210, QKV slice/reuse #2232/#2208/#2209, CUDA
  modulation+RoPE fusion #2269, Vulkan normalization #2273 — all in our
  pin.)
- **Recipe slots**: `diffusion_model` (required) + `llm` (required — the
  Qwen3.5-9B GGUF, the *conditioning encoder*) + `tokenizer` (required —
  external file) + `vae`/`tae` (required-any). **No image/reference
  slot.**
- **Defaults**: `cfg_scale 3, steps 20, 1024×1024, flow_shift 5, euler`;
  `features: [flow_shift, slg]` (slg = the negative-branch layer-22
  skip, per the docs text). `size_multiple 16`.
- **What works: text-to-image only.**
- **What does NOT: image editing.** leejet **PR #2203** ("add image
  reference support to MOSS-Image") and **issue #2238** ("MOSS Image
  Edit support") are both **open** (2026-11-06). Community edit models
  exist (`chinoll/MOSS-Image-Edit`, `akhaliq/MOSS-Image-Edit-HQ`) but are
  not stock. Because there is no image input slot, **our MultiReference /
  compose modes cannot map onto MOSS at all** until that lands.
- **This edit gap is the park trigger** (Kokon is a photo *editor*; see
  §6).

## 3. Weight inventory (pins as of 2026-11-06)

| Component | Candidate | Source | Sizes | sha256 |
|---|---|---|---|---|
| DiT (SFT) | `MOSS-Image-SFT-{Q2_K…Q8_0}.gguf` | `hf.co/leejet/MOSS-Image-SFT-GGUF` (the repo the sdcpp docs link) | 2.06–5.31 GB | not published — record at download in `provenance.md` |
| DiT (SFT) | `MOSS-Image-SFT-{Q4_K_S, Q8_0}.gguf` | `hf.co/axiomofmind/Moss-Image-SFT-GGUF` | 3.3 / 5.1 GB | **published in its SHA256SUMS**: Q4_K_S `2AA25EBE FA78AC0A A243E7FB 29DC0B8F C47D0367 D5161EA8 7EF753BE D130AB1B`; Q8_0 `8349EBDD 643F2C66 DA6B2578 9DB7DF3C C708A86C 051C5620 7FDE6991 56BFAD8E` |
| DiT (Base) | `MOSS-Image-Base-{Q2_K…Q8_0}.gguf` | `hf.co/leejet/MOSS-Image-Base-GGUF` | 2.06–5.31 GB | not published |
| DiT (Base) | `MOSS-Image-Base-{BF16, Q4_K_M, Q8_0, UD-*}.gguf` | `hf.co/unsloth/MOSS-Image-Base-GGUF` | 2.1–9.4 GB | not published |
| DiT (safetensors) | `moss_image_{base,sft}_{bf16, int8_convrot}.safetensors` | `hf.co/Comfy-Org/MOSS-Image/tree/main/diffusion_models` | 9.7 / 5.2 GB | — |
| Text encoder | `Qwen3.5-9B-<quant>.gguf` | `hf.co/unsloth/Qwen3.5-9B-GGUF` (the repo the sdcpp docs link) | Q3_K_S 4.0 → Q8_0 8.87 GB (Q4_K_M 5.29); + `mmproj` 0.86 GB if a vision sidecar ever needs one | not published |
| VAE | `flux2-vae{,_small}.safetensors` | `hf.co/Comfy-Org/MOSS-Image/tree/main/vae` (the HF card ships its own 237 MB copy) | 321 / 238 MB | — |
| Tokenizer | `tokenizer.json`, `chat_template.jinja` | `hf.co/OpenMOSS-Team/MOSS-Image-{Base,SFT}/tokenizer/` | tiny | — |

**Host status: none of it is on the box** (`models/gguf/` =
BFL/Qwen/google/unsloth/zerofata; `models/sd-cpp/` = BFL/Qwen/
madebyollin/upscalers; no OpenMOSS dir). At adoption it lands per the
provenance convention: `models/gguf/OpenMOSS/MOSS-Image-*/` (GGUF +
serve.sh + provenance.md), `models/sd-cpp/OpenMOSS/` (VAE + tokenizer),
and a `backends/_scripts/serve-…moss…sh` pure boot.

## 4. What adoption would look like (the whole change)

1. **Engine**: zero new code. One pure-boot script (shape = the
   `kokon-klein` entry in `backends/_configs/llama-swap.yaml`: `cmd` =
   script + `${PORT}`, `proxy: http://127.0.0.1:${PORT}`,
   `checkEndpoint: /sdcpp/v1/capabilities`, `unloadTimeout: 120`,
   `ttl: 600`, `capabilities: {in: [text], out: [image]}`) pointing the
   stock build at the four recipe slots.
2. **Gateway set**: `moss: "mo & p"` — the engine plus the **shared PUP
   door**, exactly mirroring `klein: "k & p"` / `qwen-t: "qt & p"` /
   `qwen-f: "qf & p"`; if the A/B instead picks a dedicated door,
   `moss: "mo & pq"` (new `pq` = stock Qwen3.5-9B). Note the port map:
   the ID `moss` sorts between `kokon-pup` and `qwen3.8-27b`, so the two
   vLLM models shift by one each on the next config reload — harmless
   (the gateway owns the map; the config comments say so).
3. **Service config**: one `models[]` entry (label, e.g. "MOSS
   (layout)"), engine = the gateway model name, `vlm` = the chosen door,
   `promptBundle` = a new **`pup-moss`** bundle (to be authored — see
   §5). The picker, capability memory, status pill and
   `/models/switch` are all model-agnostic; nothing else changes.
4. **Enhancement** (the open design question, A/B'd later):
   - **First candidate — the shared 9B PUP door + `pup-moss` bundle.**
     The shared door *is* a Qwen3.5-9B-based fine-tune (it already
     serves the `q35` alias), so it is the **same model family** MOSS
     was trained with as its encoder — the most plausible writer with
     zero new doors, per PLAN rule 2 (one weight family, one mmproj,
     per-model serve scripts). The catch: that fine-tune's prompt
     contract is tuned to *its* maker's pipeline, and MOSS was trained
     on structured 4K-token prompts — hence the A/B.
   - **Fallback — a dedicated stock Qwen3.5-9B door** (Q4_K_M 5.29 GB +
     mmproj if vision is used): the exact base model of both the
     encoder and the PUP; the most in-distribution writer for MOSS.
   - **Zero-sidecar option**: run unenhanced (their reference pipeline
     ships no enhancer; the 4K-token model is prompt-tolerant). The
     D-FB contract (down VLM = hard-fail) simply doesn't engage.
5. **Layout ↔ box editor (the nice-to-have seam)**: our box data
   (`bbox.rs` rectangles on the image) can be serialized into MOSS's
   JSON layout — a pure coordinate transform (image-space boxes →
   `abs_centralized`/`yxyx` centred) plus per-box captions — unit-testable
   with no model involved. MOSS's layout is *richer* than the klein box
   table (per-item captions, rendered text items, a canvas/meta
   summary), so the editor would map into it, not the other way round.
   Same conceptual family; adoption-gated, not needed for basic t2i.
6. **Memory math (the 24 GB card, Q4 pair)**: DiT Q4_K ≈ 3.1 GB +
   Qwen3.5-9B Q4_K_M 5.29 GB (+ 0.86 mmproj if a vision door) + FLUX.2
   VAE 0.24 GB + activations/KV → **~9–11 GB resident** — the same
   order as the qwen pairs that already run; the router's set
   exclusivity with vLLM makes coexistence a non-issue either way.
7. **Speed reality**: 20–50 steps (vs qwen-turbo's 8) on the same card
   → expect **~2.5–6× turbo's runtime** per image; the community 2/4/8-
   step DMD distills may eventually close the gap (not stock, not
   adopted). Measured, not estimated, in the A/B window when it runs.

## 5. Verdict: **PARK**

Not added to the picker, no A/B, no downloads, no config — until MOSS is
stable end-to-end. Reasons, in order of weight:

1. **No image editing in stock sdcpp** (PR #2203 / issue #2238 open).
   Kokon's product is editing; a t2i-only model is a poster machine,
   not a photo editor. (Community edit models and the authors' own
   "controlled editing via layout" hint that this is coming, but it is
   not in our pin and not in the queue we can control.)
2. **The step-count gap** against our 8-step turbo default makes it a
   slow second model by construction until a stock distill lands;
   "neck to neck" is currently a claim, not a measurement on our card.
3. **Posture**: with the upstream PR expected not to land (the author's
   one-model vision), a second picker model is a fork-only feature
   either way — worth doing deliberately, with an A/B and owner visual
   verdict, not opportunistically.

**Re-check triggers** (any one reopens the question):
- (a) leejet PR #2203 (or a successor) lands in a commit we adopt for
  the sdcpp pin;
- (b) a stock sdcpp recipe gains an image/edit path for MOSS;
- (c) an owner call — e.g. layout/poster/text-heavy use cases become a
  priority (MOSS's layout + text rendering is its actual differentiator);
- (d) optional: a distilled variant becomes stock-stable at 8–20 steps.

**Adoption, when it happens, is**: download + pin per §3 → the boot
script + set line + `models[]` entry per §4 → an A/B window against
qwen-turbo on our standing prompts plus a layout-specific set (the box
editor feeding the JSON layout is the natural layout test) → picker
entry on the owner's visual verdict.

## 6. Open questions (for the A/B, when it runs)

- Writer choice: shared PUP vs stock Qwen3.5-9B door vs unenhanced (§4.4)
  — the one decision the A/B must settle.
- `size_multiple 16` vs the planner's size tables (our sizes are 8-
  multiples) — a planner check, not a blocker (1024² and up all work).
- Multi-image (MultiReference/compose): no image slot in the recipe —
  **unsupported until edit lands**; the UI's compose entry would need to
  be hidden per model (the capability memory already handles
  per-model capability differences — verify it covers compose).
- Which checkpoint: SFT (the curated fine-tune) is the likely pick;
  Base stays the fallback. The two are one recipe swap apart.
- TAE vs the FLUX.2 VAE slot on memory-tight boxes (the recipe accepts
  either) — only relevant if we ever run MOSS on a <12 GB card, which
  is not our target.
