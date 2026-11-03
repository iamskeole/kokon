# Audit: the qwen-path engine fork, `__mess/sdcpp-fork` branch `kokon`
(task-012, 2026-11-03 — read-only analysis; the fork is archived, nothing was forked or built)

**Scope.** Every commit from the base `0a06712` ("perf: optimize Vulkan kernels,
graph fusion, and workspace reuse (#2251)", upstream leejet/stable-diffusion.cpp,
2026-10-31) to the fork HEAD `aec3024` (2026-11-01). `git rev-list --count
0a06712..kokon` = **20** (the task brief said 21 — off-by-one at write time).
This is the build the **live qwen engine** runs (`backends/sd-cpp-serve.sh`:
"upstream 0a06712 + kokon's commits, ggml pin ceacb2d, built 2026-10-31").

**Reference build.** The stock slot `/home/p/ml/backends/stable-diffusion.cpp`
is pinned at **`290148d`** (2026-11-02) = `0a06712` + 9 upstream commits
(0a06712→290148d: c6dfa9d, 45a63ac, 5356982, df0fa26, fe2682b, 62c5443,
5d33b1b, 133087d, 290148d — mostly MiniMax-H3/Vulkan/CI; the two that touch
shared paths are `133087d` auto-fit discrete-GPU selection and `290148d`
H3-only operator fusion). The stock build is the one klein runs today
(Windows E+F+G of task-011 all on it).

**ggml pin trajectory** (submodule): `0a06712` → `6a894937` (upstream's own);
`d08af97` → `ea9b16d9` (kokon-patched ggml: graph compute-callback support,
needed for LLM decode-graph reuse); `ba47a5d` → `ceacb2d3` (unpatched
upstream ggml — "the commit upstream 0a06712 builds against", plus the
sdcpp-side graph-id guard of 58de325). The final fork build therefore ran an
**unpatched** upstream ggml; the patched one was only ever a transient of the
LLM-decode experiments.

## The table

| hash | title | class | rationale |
|---|---|---|---|
| ebb5061 | feat: add byte-level BPE decoding and a streaming token decoder | retire | decodes LLM *output* tokens (text generation); touches the shared tokenizer files, but the image path only uses encoding, which is unchanged |
| a32c966 | feat: add an on-demand LM head and next-token logits to LLMRunner | retire | LM head / next-token logits exist only for autoregression; image prompting runs the encoder forward pass, which the base already has |
| 66038e9 | feat: add a KV cache and greedy generation to LLMRunner | retire | KV cache + greedy loop = the text-generation core; the base LLMRunner (encoder-only) serves image prompting without it |
| c0c471c | feat: move other models to RAM when text generation needs the VRAM | retire | `prepare_text_generation()` + `offload_params_to_host()` — called only from the text-gen path; dormant without an LLM text model |
| 60e012b | feat: release the vision tower's staged weights before text generation | retire | llm.hpp-only; frees VRAM for text-gen prefill; no image-path effect (image runs release weights per segment in the stock code) |
| 3bbee6a | perf: reuse the LLM decode graph between steps | retire | decode-step graph caching for autoregression; the compute_workspace hunk only fires when a runner registers a CPU extra backend (LLM partial-offload), which the image path never does |
| d08af97 | build: use CUDA graphs only for reused graphs (ggml) | retire | pins the kokon-patched ggml (ea9b16d9) to enable the compute callback of 3bbee6a; itself superseded by ba47a5d |
| 3e8f1b2 | feat: compute the LLM layers that do not fit the VRAM budget on the CPU | retire | `enable_cpu_layers()` partial offload, driven entirely from `prepare_text_generation()`; the compute_workspace change is inert without an LLM runner carrying a CPU backend |
| b0b0be1 | feat: add a seeded sampler to LLM text generation | retire | new `text_sampler.cpp/.h` — seeded LLM sampling for the sidecar-replaced in-engine PUP; text-gen only |
| 0c25975 | feat: add generate_text() to the C API | retire | `text.cpp` + API surface for in-engine text generation; the sidecar (PUP on llama.cpp) is the replacement |
| e9aab8d | feat(server): add POST /sdcpp/v1/text_gen | retire | the HTTP door for in-engine text generation; replaced by the sidecar's OpenAI-compatible endpoint |
| 58de325 | fix: give reused graphs ids without ggml's internal counter | retire | atomic graph-uid source for the decode-reuse cache (ggml's counter is not exported in the unpatched build); serves 3bbee6a only |
| ba47a5d | ggml: pin the commit upstream 0a06712 builds against (plus the graph-id guard) | moot | build pin: moves the submodule back to unpatched upstream ggml (ceacb2d3); the stock build carries its own pin (d49c49e1) — no code dependency survives on the image path |
| 0f92815 | core: restore the LLM-runner header members the decode-graph merge dropped | retire | re-adds `compute_reusable()` / `set_graph_reuse_enabled()` / `persistent_buffer_bytes()` to ggml_runner.h — all LLM decode/KV-cache machinery (default virtual = 0; unused by the image path) |
| 01c970b | core: use kokon's final execute_segment form | moot | reverts the fork's execute_segment to the stock body (scheduler-or-runtime compute, eval callback) — i.e. *aligns with* upstream; the stock pin already is this code |
| 301d4e0 | core: keep the callback-bearing execute_segment (upstream's segmented execution needs it) | moot | re-adds the `on_before_compute` parameter; the stock 290148d has **exactly this signature and the same `weights.submit_next()` caller** — net difference vs stock is one dead `#ifndef SD_USE_UPSTREAM_GGML` block (inactive in an unpatched-ggml build) |
| d4c09a0 | engine: prompt upsampler LLM (--pup-llm) for text generation | retire | the second-LLM (PUP) slot and its text-gen wiring; the sidecar architecture replaces the in-engine PUP entirely |
| 9856a02 | LLM: the vision conversion applies to the PUP encoder too | retire | adds the `text_encoders.pup.visual.` mmproj-name conversion (+ flux3 to the version list) so the *PUP* encoder can take reference images; QWEN_IMAGE_2_1 was already in that list at the base, so the qwen image path (incl. i2i vision) is untouched |
| e2181bc | text_gen: size the KV cache for the expanded vision sequence | retire | KV sizing for vision-augmented text-gen prompts; text-gen only |
| aec3024 | text_gen: restore the single-placeholder image template | retire | prompt-template fix inside `text.cpp` (the text-gen module); no image-path surface |

**Tally: 17 retire · 3 moot · 0 keep-ported.**

## Conclusion

**The qwen pipeline is safe on the stock build (`290148d`, the same build as
klein) with zero of the 20 commits. There is no exception list.**

Reasoning, per code path:

1. **What the qwen image path needs** = DiT denoising + the Qwen3-VL-8B text
   *encoder forward pass* + its vision tower (mmproj) for reference-image i2i
   + the texture-fix VAE. The base `0a06712` already carries the
   qwen-image-2.1 pipeline, the Qwen3-VL encoder, and the vision-name
   conversion for `VERSION_QWEN_IMAGE_2_1` (visible in the 9856a02 pre-image).
   Image prompting never autoregresses: it needs no KV cache, no LM head, no
   BPE *decoder*, no decode-graph reuse, no sampler — which is precisely the
   17 commits classified retire.
2. **The three moot commits** are merge/build shims whose end state *is* the
   stock code: the final `execute_segment` matches 290148d's signature and
   caller line-for-line, and the final ggml pin is unpatched upstream (the
   stock build simply uses its own pin). Nothing to port, nothing missing.
3. **Shared-file changes are dormant on the image path** (verified in-diff,
   not by name): tokenizer (encode path unchanged), compute_workspace (the
   changed branch requires an LLM runner with a CPU extra backend),
   name_conversion (9856a02 adds only PUP/flux3 prefixes; QWEN_IMAGE_2_1 was
   present at the base), model_manager (`offload_params_to_host` is called
   only from `prepare_text_generation`), ggml_runner (the new members are
   default-inert; `compute_reusable`'s uid hunk serves decode reuse only).
4. **The one behavioral question — auto-fit** — resolves in favor of stock:
   the qwen engine runs `--auto-fit` (see `sd-cpp-serve.sh`), and the stock
   pin *includes* upstream fix `133087d` ("prefer discrete GPUs in auto-fit
   device selection") which the fork's 0a06712 base lacks. The stock build is
   strictly newer on the shared paths it touches.
5. **The empirical side** (context, not proof — the cutover is task-013's
   job): the fork build was "verified bit-identical output to the retired
   kokon-1013-1814814 build … on the Qwen turbo/full recipes" (serve.sh
   header), and its 20 commits were all *additive* text-gen machinery on top
   of that verified base — the image outputs it produced came from the base
   code, not from the 20 commits.

**Follow-up on keep-ported:** none exists, so there are **no upstream PR
candidates from this audit** (per the running rule, a PR is only made if the
owner says so — there is nothing to propose).

**Caveats / watch-outs for the cutover (task-013):**
- The stock pin's ggml (`d49c49e1`) differs from the fork's final ggml
  (`ceacb2d3`); if any qwen-recipe drift appears after the cutover, ggml
  kernel changes between those pins are the first suspect (the 9-commit
  upstream gap is mostly H3/Vulkan, but ggml moves fast).
- The in-engine PUP (`--pup-llm`) disappears with the fork; the qwen
  pipelines take the **sidecar** PUP like klein does (per the shared-PUP
  rule, adopted per-pipeline on A/B — that A/B is part of the WP3 work, not
  this audit).
- `POST /sdcpp/v1/text_gen` disappears from the engine's API surface; the
  service's PUP slot points at the sidecar, not the engine.

## Addendum (2026-11-03, after task-013's A/B window)

This audit's framing of the in-engine PUP was wrong in one respect that
matters for the record:

- **The qwen pipelines DID use the in-engine PUP.** The live fork engine
  was started without `--pup-llm`, so the in-engine LLM was the
  Qwen3-VL-8B *conditioner itself* doubling as the prompt upsampler
  (the service's qwen pipeline had no per-pipeline `vlm` entry; the
  non-BFL path fell through to the engine's `text_gen`). So the
  text-generation machinery was in active production use for qwen — not
  the `--pup-llm` second-LLM slot (that one was genuinely unused) and not
  the klein/BFL pipelines (those use the separate BFL sidecar).
- **Task-013's Option A (owner's call) replaced exactly that**: the qwen
  pipelines now use a **sidecar** PUP (llama.cpp, per-flavor doors
  `kokon-pup-qwen` :39176 / `kokon-pup-qwen-f` :39177) running the *same*
  Qwen3-VL-8B Instruct weights (Q4_K_M + mmproj-F16, alias `qwen3vl-8b`,
  temp 0 greedy, 1024 max tokens, image attached for edit) the engine
  conditions with — same weights as the in-engine PUP, different runtime.
  Klein's BFL sidecar path is untouched. This is why the audit's
  retire-everything conclusion still holds: nothing in the 20 commits is
  needed, and nothing the qwen pipeline consumed is lost (the weights are
  the same; only the LLM runtime changed).
- The A/B window (fork in-engine PUP vs stock+sidecar, 3 shared-seed t2i
  pairs + 1 i2i) measured no crash-level difference and no shingle-repeat
  hallucination in either path; the parity question (same weights, two
  runtimes: is the output equivalent enough?) is the owner's visual
  verdict on the 7 renders (`/tmp/kleenq/img-*.png`), not this audit's.
