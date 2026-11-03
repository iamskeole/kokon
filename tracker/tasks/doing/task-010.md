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
