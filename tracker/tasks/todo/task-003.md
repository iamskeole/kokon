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
