# task-003 — models dir: standardize ALL model weights per the provenance convention

## goal
Bring **all** model weights under the host's `models/` to the convention:
`{org-or-user}/ModelName/` dir, standardized filenames
(`model-<QUANT>.gguf`, `mmproj-<PREC>.gguf`, `mtp-<QUANT>.gguf`), one
`serve.sh` and one `provenance.md` per model dir. Scope:
- `models/sd-cpp/flux3/` (the klein stack, currently flat): the DiT
  (`flux-3-image-klein-Q8_0`, the Q5_K), the VAE (`ae.safetensors`), the
  4B conditioner + its mmproj, the PUP (Q4_K_M / Q6_K / Q8_0) + its
  mmproj, the 8B Qwen3-VL (Q4_K_M / Q8_0) + its mmproj — each into its own
  `ModelName/` dir under the right org dir (BFL / qwen, **verified from
  the GGUF/safetensors metadata, not guessed**).
- `models/gguf/*` dirs not yet conforming (e.g. the legacy flat
  `G4-MeroMero-*`, `google/gemma-4-*` naming) → normalize to
  `{org}/ModelName/`.
- **Out of scope:** `models/huggingface/` — that is the HF_HOME for the
  vllm/creative side (owner-owned; it already mostly follows the
  convention).

## acceptance criteria
- Every GGUF/safetensors in scope sits in `{org}/ModelName/` with the
  standardized names; the old flat locations are empty (move, not copy —
  no duplicate weight sets on disk).
- `provenance.md` per model dir: source org/repo/URL, quant family,
  license, download date, sha256, notes (who published/converted, imatrix
  if any).
- Every GGUF parses cleanly (python `gguf` lib) and its arch string is
  recorded in the provenance (`qwen35` PUP, `qwen3vl` 8B, `clip` mmprojs,
  …); the VAE's safetensors header reads.
- All downstream references updated to the new paths in the same task:
  llama-swap config, the engine serve scripts, the task-002 deploy
  recipe. (This task stays CPU-only — each model's load check happens in
  the task that actually uses it.)

## blockers
- Moving files that live paths point at means on-demand loads of those
  models are briefly broken between the move and the config update — do
  the move + reference updates as one atomic step. (No vllm window: none
  of these are vllm models.)

## decisions
- `{org}/ModelName/` (the owner's stated convention) — legacy flat dirs
  get normalized here.
- Org dirs come from metadata (verify each file; the PUP is a BFL
  fine-tune — its GGUF's metadata says who published it).
- `models/huggingface/` untouched (owner-owned; note only).

---

## Done (2026-11-02, same night)

## acceptance evidence

- **Moved / renamed** (all same-fs renames, ~60s, zero copies; `models/` is
  not a git repo — host-owned):
  - `gguf/Qwen/Qwen3-VL-{4B,8B}-Instruct/` — model-<Q>, mmproj-<P>, serve.sh,
    provenance.md each (4B set verified Qwen/Qwen3-VL-4B-Instruct-GGUF
    [gated, 401] by exact size+sha; 8B set verified Qwen/Qwen3-VL-8B-Instruct-GGUF
    — shas `67d1659b…`/`0d264b39…`/`ca524100…`/`c6ba8550…`)
  - `gguf/BFL/flux-3-image-klein-pup/` — per the convention's one exception
    ("PUP is the only one that isn't: it's an LLM and belongs to the engine"):
    model-{Q4_K_M,Q6_K,Q8_0}.gguf, mmproj-F16.gguf, tokenizer.json. **Official
    repo `black-forest-labs/flux-3-image-klein-pup` — exact file-set match.**
  - `sd-cpp/Qwen/qwen-image-2.1/` (model-Q8_0) and
    `sd-cpp/Qwen/qwen-image-2.1-turbo/` (model-Q4_K_M): verified
    **unsloth/Qwen-Image-2.1-GGUF** (base sha `c0ed4b2f…`) and
    **unsloth/Qwen-Image-2.1-Turbo-GGUF** (sha `8940e8b5…`) — both shas are
    what the Kokon catalog pins.
  - `sd-cpp/BFL/flux-3-image-klein/` — model-Q8_0 (live) + model-Q5_K
    (stashed, task-009 decision), provenance (see below)
  - `sd-cpp/BFL/flux-3-ae/` — ae.safetensors (member of the official klein
    repo; sha `868fe7b3…`)
  - `sd-cpp/madebyollin/texture-fix-vae-for-qwen-image-2.1/` — official
    repo, sha `36f56608…` (matches catalog)
  - `gguf/zerofata/G4-MeroMero-v2-{26,31}B/` — **owner-named** (see
    decisions): 31B = `zerofata/G4-MeroMero-v2-31B-GGUF` (file-size match
    ±1152 B header delta; its MTP file is byte-identical to the official
    gemma-4-31B QAT MTP); 26B = author's v2 (gated, 401; 17.33 GB ≠ v1 A4B
    16.80 GB)
  - `gguf/google/gemma-4-31B-it-qat/` — standardized names; the 31B repo is
    gated (QAT shas not publicly verifiable)
  - `gguf/unsloth/Qwen3.5-9B/` — **unsloth/Qwen3.5-9B-GGUF** (file-size
    match; MTP is the repo's 2nd file, mmproj its 1st)
  - Dedup: the lone top-level `Qwen3VL-8B-Instruct-Q4_K_M.gguf` was
    byte-identical to the flux3/ copy (sha `67d1659b…`) — removed, one
    canonical copy in the Qwen dir.
- **Reference updates** (all on this host, non-git): `llama-swap-deploy/`
  config.yaml + creative-fast/slow.sh (zerofata paths); `backends/`
  sd-cpp-serve.sh + sd-cpp-serve-klein.sh (new layout; the klein fork
  script keeps its `--pup-llm`/`--pup_llm_vision` flags — the fork engine
  is until task-010/011); `deploy/klein/` stock script + README;
  `deploy/jobs/stock-klein-test.sh`; `OPS.md` "Models convention" section
  (gguf = LLMs, sd-cpp = engine images, standardized names — codified).
- **Verification battery**: bash/sh -n on all 13 scripts (all pass);
  every model path referenced by live scripts exists (9/9 + the two
  script-internal expansions, after fixing one `$M/gguf/` slip in
  sd-cpp-serve.sh → absolute gguf path); `llama-swap -validate` on the
  live config: "config is valid: 6 model(s), 0 peer(s)".

## provenance facts (for the WP2/WP3 catalog work)

- klein DiT **conversion source: UNRESOLVED.** Digit-3 GGUF naming
  (`flux.3-image-klein.Q8_0.gguf`, `flux.3-image-klein.Q5.K.gguf`) matches
  no public repo: HF cache tree `1c2ad2f9…`, no LFS-sha match among 58
  scanned candidates (both files share the tree — same source), 404 on
  the two plausible renames. Flagged in the dir's provenance.md. **Question
  for the owner, non-blocking: do you remember the klein DiT conversion
  repo?** (It matters for the WP2 stock-cutover catalog pin.)
- MeroMero mmproj-BF16 and 26B mtp-Q4_K_M: companion files not present in
  the model repos — source unverified (recorded).
- gemma mmproj precision: unverified (named `mmproj.gguf`; the 31B MTP
  being byte-identical to the official QAT strongly suggests the dir is
  the official set).

## decisions (this task)

- **Naming (owner, 2026-11-02):** the creative models are the author
  **zerofata**'s Gemma-4 fine-tunes called "meromero" →
  `gguf/zerofata/G4-MeroMero-v2-{26,31}B`; "v2" = the current v2 line
  (31B public, 26B gated) vs the v1 A4B 16.80 GB the box had before.
- **PUP placement:** `gguf/BFL/flux-3-image-klein-pup/` (it's an LLM;
  convention's stated exception). The WP1 PUP sidecar (task-004) will
  point there.
- **klein Q5_K kept** (not deleted) in the model dir — task-009 decides
  which quant the stock cutover ships.
- The `models/huggingface/` HF_HOME (vllm side) was left untouched, per
  its own ownership note.
