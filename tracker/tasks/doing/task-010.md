# task-010 — PUP A/B: 8B {Q4_K_M, Q8_0} vs 9B {Q4_K_M, Q6_K, Q8_0} on the sidecar

## goal
Pick the production PUP + quant. Six candidates, all on the same sidecar
runner (llama-server, :39175, `--mmproj`, thinking off, `--jinja`), each
evaluated on the three standing prompts carried over from the A/B rig:
C1 kitchen t2i, C2 desk edit, C3 lantern+robot compose. The BFL system
prompt comes from the repo assets (task-007).

## acceptance criteria
- Per candidate: a digest file with the three outputs (captions, verbatim
  is fine — they are model *answers*, not BFL template strings), token
  counts + wall times, and a server-log tail (proves which model/quant was
  loaded each run).
- `DONE`/`FAIL` markers per run; every run is a self-contained GPU window
  (autonomy protocol: setsid job + watcher + blocking call + vllm reload
  at the end of each window).
- The owner visually validates the renders (the captions alone are not the
  test — the rendered images are).
- Winner recorded in `STATUS.md` (D-FB note: quant also bounded by memory
  headroom with the DiT co-resident on the 24GB card — Q8 9B + DiT Q8 ≈
  22.7GB; Q4/Q6 are the safe band; record the measured fit).

## blockers
- task-003 (files in place), task-004 (runner), task-007 (prompt assets) — all done.
- The 9B Q6_K and Q8_0 GGUFs are **not on disk** — downloading from
  `unsloth/Qwen3.5-9B-GGUF` (background job `dl9b.sh`, markers
  `dl9b-DONE`/`dl9b-FAIL` in the model dir). Window A (8B×2 + 9B Q4) does
  not depend on it; window B (9B Q6/Q8) waits for it.

## decisions (execution, 2026-11-03)
- **The candidate set is 5, not 6.** The goal line says six, but the
  enumerated set is 8B {Q4_K_M, Q8_0} + 9B {Q4_K_M, Q6_K, Q8_0} = 2+3. The
  Qwen-official 8B repo ships no Q6 (confirmed in the archived in-engine
  run8b), so there is no 8B Q6 to add. We run the five enumerated
  candidates; the "six" in the goal text is a miscount and is corrected
  here rather than inventing a phantom candidate.
- **Two windows, as the original note anticipated.** Window A =
  {8B Q4_K_M, 8B Q8_0, 9B Q4_K_M} (all files already on disk — runnable
  immediately). Window B = {9B Q6_K, 9B Q8_0} (runs once the download
  lands). Each window is one self-contained GPU unit per the autonomy
  protocol (setsid job + watcher + blocking call + vLLM restored at the
  end). Keeping vLLM down in two short windows instead of one long one
  bounds the owner's chat/agent-LLM outage each time.
- **The pair stays gateway-driven (single authority, task-009 design).** We
  do not hand-run the sidecar per candidate: the joint supervisor
  (the `kokon-klein` gateway cmd) sources
  `llama-swap-deploy/pupab-override.env` before its defaults, so each
  stage writes that file (candidate model + mmproj), triggers the group
  through the gateway, and the supervisor raises the engine + that
  candidate sidecar. The nudge remains a clean veto, and every GPU change
  is a clean gateway stop. The override file is removed at window end and
  in the fallback, so production is never left pointed at a candidate.
- **The three standing cases are driven directly against the live pair,
  not through the kokon service.** The unit under test is the PUP (the
  VLM), so we call its `/v1/chat/completions` directly with the matching
  BFL system-prompt variant from the repo bundle (`t2i_bbox` for C1,
  `i2i_bbox` for C2/C3; temp 0, max_tokens 3584, base64 data-URL image
  parts — service parity per `vlm.rs`), capture the caption + token
  usage + wall time, then render that caption with the stock klein DiT
  (`/sdcpp/v1/img_gen`: C1 plain t2i, C2 single-reference edit with no
  strength, C3 two-reference compose with `resize_before_vae=false`) to
  produce the image the owner visually validates. This measures exactly
  the model being A/B'd, without the planner's mode-resolution layer
  varying between candidates.
- **The original C2/C3 input photos are lost** (searched the host and all
  kokon docker volumes — the two `input:` hashes from the archived
  cases.json are gone). We regenerate two stand-in reference images once
  with the klein engine (stage 0 of window A): a desk-by-a-window scene
  containing a mug, a notebook and a paper lantern (serves C2's desk edit
  and C3's "lantern from image one"), and a small brass robot (C3's
  "robot from image two"). The stand-ins keep the cases' structure, bbox
  layout and seeds; only the specific pixel content of the original photos
  is not recoverable. This is noted in the digest so the owner's visual
  validation compares the candidates on identical (if stand-in) inputs.
- **The 8B baseline is re-run on the sidecar** (its in-engine A/B showed
  Q8_0 > Q4_K_M) so both models are compared on identical infrastructure,
  as the original note intended.

**Windows A and B are done — and the model identity was wrong for the PUP slot** (owner asked how our sampling aligns with BFL's maker recommendations; the audit in the BFL inference repo `black-forest-labs/flux-3-image-klein` surfaced a bigger problem):

- **Model identity correction (the owner's suspicion was right).** The "9B" candidates in windows A/B were **stock unsloth Qwen3.5-9B**, not the PUP. The real BFL PUP — HF `black-forest-labs/flux-3-image-klein-pup`, metadata `Flux3_Pup_Hf`, base `Qwen/Qwen3.5-9B` — has been on disk all along at `/home/p/ml/models/gguf/BFL/flux-3-image-klein-pup/` (Q4 5.63 GB / Q6 7.36 GB / Q8 9.53 GB + mmproj-F16 + tokenizer; GGUF conversion of BFL's bf16 weights, verified by the model metadata inside the file; the stock files carry unsloth metadata instead and differ even in Q4 size). The 8B slots were the official Qwen3-VL-8B GGUFs — stock Qwen3-VL, which is exactly the baseline the owner intended.
- **BFL's maker-recommended decoding (from the inference repo).** PUP sidecar: greedy (temp 0), top_p 0.95, top_k 20, min_p 0, max_tokens 3584, presence penalty **1.5 for t2i / 0.0 for edits**, max_model_len 8192, references LANCZOS'd to per-N pixel budgets (1 ref ≤ 1 MP — 768² unchanged; 2 refs ≤ 724² — 704×704), JPEG q92, natural image grid. DiT (distilled klein): **16 steps, no CFG (guidance 1), euler** (the 50-step/CFG 4 row is the base model). VAE is deterministic and the 4B Qwen3-VL text encoder produces embeddings only — neither is sampling-sensitive; the PUP and the DiT are the only two.
- **What windows A/B actually ran (the audit result).** Sidecar: temp 0 ✓, top_p 0.95 ✓, but top_k 40 (BFL: 20), min_p 0.05 (BFL: 0), no presence penalty (BFL: 1.5/0), and `--image-min-tokens 1024 --image-max-tokens 4096` forced an image grid of ≥ 1024 tokens (upscaling the 768² refs; BFL's processor sends the natural grid). Engine: my case driver sent **wrong field names** in `sample_params` (`cfg`/`sampler`/`steps` — the production shape per `crates/qls-core/src/sdcpp.rs` is `sample_method`/`sample_steps`/`guidance.txt_cfg`), so the engine silently ran its preset defaults (~10 steps t2i, ~20 with refs) instead of 16. → A/B data is directional only; **window C is the maker-spec matrix.**
- **A-vs-B window recap (asked by the owner).** A (01:33–01:49): 3 candidates × all 3 cases (8B-Q4, stock-9B-Q4, 8B-Q8); run 1 FAILed on three bugs (render response shape, VRAM collision with the previous stage's drain → gpu-free wait, and no re-trigger after the gateway's 120 s TTL killed the still-loading pair → 280 s re-trigger + non-fatal stage timeout); C3 (two-reference compose) then failed for all three candidates because case traffic bypassed the gateway, so the alias's TTL expired mid-render. B (08:31–08:42): C3-only re-runs for the same three candidates + full 9B-Q6 and 9B-Q8, with all case traffic routed **through the gateway** (TTL parity with production) → 15/15 cases, 9 renders.
- **Old-params results (directional).** The 9B family (stock) wrote the richest C1 prose (259–296 tok) but **dropped the locked bbox table on C2/C3** (33–55 tok bare instructions, one invented a `<table_1>` tag); the 8B family kept the full src/tgt table in every case. Fit: 8B-Q8 and the 9B Q6/Q8 all ran on the card (9B-Q8 the tightest at ~22-23 GB). C3 captions were short everywhere (33–220 tok).
- **Window C (owner gave preemptive GO).** The real A/B: **BFL PUP 9B {Q4_K_M, Q6_K, Q8_0} vs stock Qwen3-VL-8B {Q4, Q8} vs stock Qwen3.5-9B Q4** (the fine-tune's own base model, as a reference), all under maker-recommended decoding on both sides (supervisor sidecar line and case driver patched; supervisor startup grace 240→480 s for 9B cold loads), same three standing cases, same stand-in refs. Digest: `jobs/pupab-C-digest.txt`. The question the owner cares about: does the fine-tune restore the box table (and lift quality) at equal or better memory than the 8B?

**Window C results (2026-11-03 09:15–09:29, 14 min, 6 candidates × 3 cases, 18/18 renders, all under maker-spec decoding)** — digest `jobs/pupab-C-digest.txt`, images in the owner's chat. The owner's second nudge landed on the DONE marker (no collision — checked state before touching anything; this time nothing needed doing).

- **PUP (the real BFL fine-tune) — clear lift on t2i and compose, degeneration on single-ref edit.** C1: 540–584 tok (8B: 219–223; stock 9B: 145) with the box table — visibly richer scene prose across all three quants. C3 (two-ref compose): 410–752 tok with the table, clean renders. **C2 (single-ref edit): all three quants fell into a repetition loop** (repeated instruction sentences / repeated box rows with incrementing ids) and were truncated at max_tokens 3584 (finish=length) — the box table was lost in the cut. The loop is new: the last clean PUP edit run (task-008's job B) used the old sidecar flags *and* BFL's locked-record i2i scaffold, whereas window C used the maker-spec decoder (natural image grid, JPEG q92) *and* my simple "rearrange this scene [table]" scaffold — two variables changed at once, so the trigger isn't isolated yet (candidate: the scaffold deviation; a 2×2 sampler×scaffold window would pin it; a small temperature would also break a greedy loop).
- **8B (stock Qwen3-VL) — consistent and fast everywhere.** C2: 193 tok, finish=stop, full src/tgt table, 3.6 s sidecar (vs the PUP's 48–66 s loop-to-cap). C3 keeps the table too. Lighter sidecar (8.7 GB Q8) and the only candidate with zero degenerated cases.
- **Stock 9B (the fine-tune's base) — the contrast the owner wanted:** C2 keeps a table (in a json fence) but bare instructions, no rich descriptions; confirms the fine-tune's lift is in description richness, and its box-keeping training is what the PUP (partially) restores.
- **Memory (all fit on the 24 GB card, pair + nothing else):** PUP sidecar Q4 5.6 / Q6 7.4 / Q8 9.5 GB + engine 8.7 GB → PUP-Q8 the tightest (~20 GB, ran fine, pair healthy at t+10 s on warm cache).
- **State for the owner's call.** If a production PUP is needed today: 8B Q8 (consistent, fast, light, table in every case). The PUP is the better t2i/compose captioner, but its edit path needs the loop fixed first (BFL locked-record scaffold and/or a small temperature — one more short window). The owner visually validates the 18 renders and makes the final call; winner goes to STATUS when he decides.
