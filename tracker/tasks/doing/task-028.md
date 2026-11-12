# Task-028 (CURRENT — owner-triggered 2026-11-12) — Qwen native PUPs: PE-T2I for t2i jobs, PE-I2I for i2i jobs, per mode, under explicit per-family door names

**Created:** 2026-11-12 — the owner: "qwen actually released two fine
tune pups for t2i and i2i, maybe have each model with its designated /
native pup?" (links: `Qwen/Qwen-Image-2.1-PE-I2I`, `Qwen/Qwen-Image-2.1-PE-T2I`
on HF). This file documents the build. **Not parked** — the other
`todo/` files (023/025/026/027) stay parked on their triggers. The
reading that fits the two releases: *each* Qwen pipeline job mode gets
its **native** PUP — `PE-T2I` for text-to-image, `PE-I2I` for every
image-grounded job — on both qwen flavors. **Klein is untouched in
behaviour: its native PUP is BFL's own fine-tune, already the shared
door (task-010/019)** — but its door gets the explicit name (below).

**Naming (owner, 2026-11-12: "explicit better than implicit"):** every
PUP door is named after what it serves — one identity per door, used
everywhere (gateway id = llama.cpp `--alias` = the service's
`vlm.model` field = the boot-script file name, extending the 022c
"id == script name" convention):

| gateway id (= alias = service `model` = script name) | serves |
|---|---|
| `kokon-pup-flux-image-3-klein` | klein (BFL PUP — **renamed** from `kokon-pup`) |
| `kokon-pup-qwen-image-2.1-t2i` | qwen t2i jobs (PE-T2I — new) |
| `kokon-pup-qwen-image-2.1-i2i` | qwen image-grounded jobs (PE-I2I — new) |

Rule for future doors: `kokon-pup-<engine-family>[-<mode>]` mirroring
the engine ids (`kokon-<engine-family>`); the mode suffix appears only
when one family has more than one door, and the flavor (turbo/full)
never appears — the same door serves both flavors. The legacy wire
names (`q35`, `qwen3vl-8b` — artifacts of the 8B-door era) stay as
*additional* aliases on the klein door through the transition so a
mixed-state window or an old `.bak` service config can never 404; a
later cleanup drops them.

## The models (investigated 2026-11-12, HF API + READMEs)

Both released 2026-09-20 by Qwen, under the Qwen Research License (the
same license the box already runs for the 2.1 DiT and text encoder).
Both are **fine-tuned Qwen3.5-VL 9B** (hybrid linear/full attention,
`Qwen3_5ForConditionalGeneration`, 32 layers, ~9.4B params, BF16
18.82 GB per the index) — the same base family as the BFL PUP on the
box (also a Qwen3.5-9B fine-tune) and unsloth's stock `Qwen3.5-9B`
GGUFs.

- **PE-T2I** — text-to-image prompt rewriter. Input: a short request in
  any language (no images). Output: a `think` reasoning block, then a
  JSON object `{"rewritten_prompt": "...", "wh_ratio": "16:9"}` — the
  ratio from a 7-value set that maps to 2048-class render sizes in
  Qwen's own recipe. Reference sampling from the README: `do_sample`,
  temp 1.0, top_p 0.95, top_k 20, thinking enabled, max_new_tokens
  16256. The GGUF quantizations carry **no mmproj** — text-only usage.
- **PE-I2I** — image-editing prompt rewriter. Input: an editing
  instruction plus **one or more** input images (referenced
  `<image1>`, `<image2>`, …; up to 10 per the 2.1 model card). Output:
  `think`, then JSON `{"rewritten_prompt": "...", "wh_ratio" |
  "ratio_follow"}` — the two fields mutually exclusive (new ratio for
  compositions, follow the input's ratio for in-place edits). Reference
  max_new_tokens 24000. The GGUF set **includes `mmproj-bf16` (922 MB)**.
- Both repos ship a **`system_prompt.txt`** — the fine-tune's own
  system prompt, the training contract (the analog of BFL's committed
  prompt bundle; see D-PE3).

**Quantized versions (the box runs GGUFs, never the 18.8 GB BF16):**

| Source | Files | Note |
|---|---|---|
| `prithivMLmods/Qwen-Image-2.1-PE-T2I-GGUF` (47k dl) | Q3_K_M/L, Q4_K_S/M, Q5_K_S/M, BF16 | no mmproj (text-only); the T2I door's source |
| `prithivMLmods/Qwen-Image-2.1-PE-I2I-GGUF` (101k dl) | same + `mmproj-bf16` | the I2I door's source |
| `lightx2v/…-Q8_0` (both) | Q8 only | reference quant, too big to pair (see below) |
| `mozophe/Qwen-Image-2.1-PE-MTP-GGUF` (23k dl) | MTP draft variants | speculative decoding; a latency lever, optional |
| `prithivMLmods/…-MLX` (both) | MLX builds | **out of scope** — the Mac engine is a separate task |

**llama.cpp support on this box: proven.** The Oct-14 llama.cpp tree
(`529ac893e`, `backends/llama.cpp/`) already ships `src/models/qwen35.cpp`
(including MTP block loading) and is the very build serving BFL's
Qwen3.5-9B PUP today — so the architecture, the jinja template with
the `enable_thinking` switch (the current door boots with
`--chat-template-kwargs '{"enable_thinking":false}'`), and the mmproj
path all work on this exact build. The new GGUFs are the same
architecture; remaining unknowns are file-specific (blockers, below),
not architectural.

## Current state (what changes, and what doesn't)

- **One shared PUP door** `kokon-pup` (→ `kokon-pup-flux-image-3-klein`
  after the rename step): BFL PUP Q4_K_M (5.63 GB) + mmproj-F16
  (0.92 GB) on the named llama.cpp; aliases `q35` + `qwen3vl-8b` (the
  live service config actually uses `qwen3vl-8b` for **all three**
  pipelines — the `q35` distinction is stale in the config comment);
  **member of all three image sets** so a family or flavor switch
  reloads only the DiT engine, never the sidecar (task-019).
  Non-thinking door, ctx 8192, image clamp 1024/4096 tokens. Boot:
  `backends/_scripts/serve-kokon-pup.sh` (pure boot).
- **Service** (0.12, schema 13): both qwen pipelines carry
  `vlm: {entrypoint: …/upstream/kokon-pup, model: "qwen3vl-8b"}` and
  `promptBundle: pup-qwen` (Kokon's own four prompts: t2i / edit /
  compose / compose_label — `crates/qls-core/assets/pup-qwen/`,
  `pup.rs::PupBundle::qwen_system` maps Generate→t2i, Compose→compose,
  every other mode→edit). Klein carries `promptBundle: pup` (BFL
  contract bundle, the six BFL assets) with the same door as its
  `vlm`. The service-side VLM client (`vlm.rs`, non-streaming
  `/v1/chat/completions`, images as base64 parts, 300 s timeout,
  one-degenerate-retry, D-FB hard-fail on an unreachable endpoint) is
  mode-agnostic: it sends the bundle's system prompt and takes the raw
  reply text as the new prompt.
- **Measured VRAM (3090 24 GB):** qwen-full pair ≈ 21.3 GB, klein pair
  ≈ 20 GB (task-014/010); a **Q8 PUP door OOM'd** the full pair
  (task-014) → Q4_K_M is the quant class, and **two 9B pups can never
  co-reside with a qwen engine** (files alone ≈ 22.8 GB + KV cache /
  CUDA context → OOM). The two Qwen pups therefore must be **exclusive
  of each other** behind the qwen engine — but the BFL door can stay
  *in* the qwen sets as an unused escape hatch (a set is a constraint,
  not a resident).

## Design

**Topology (10 gateway models):** the BFL door renamed, plus two
qwen-native doors, **exclusive per job mode**:

| gateway model | weights | set membership (matrix vars) |
|---|---|---|
| `kokon-pup-flux-image-3-klein` (`p`) | BFL PUP Q4_K_M + mmproj-F16 | `klein: k & p` (unchanged); also in the qwen `|`-groups as escape hatch |
| `kokon-pup-qwen-image-2.1-t2i` (`pt`) | PE-T2I Q4_K_M, **no mmproj** (text-only) | `qwen-t: qt & (p \| pt \| pi)`, `qwen-f: qf & (p \| pt \| pi)` |
| `kokon-pup-qwen-image-2.1-i2i` (`pi`) | PE-I2I Q4_K_M + mmproj-bf16 | same two sets |

The matrix `|`-group means "exactly one of": a t2i job requests `pt`,
an image-grounded job requests `pi`; whichever the job's VLM call names
gets booted, the siblings are evicted if resident (FIFO-deferred while
busy). The engine request then forms the pair, exactly as today. The
group **keeps `p`** through and after the cutover: rollback is then a
pure *service-config* revert (point the qwen pipelines back at the
renamed shared door + `pup-qwen` bundle) with **no gateway change** —
the group is only a coexistence constraint, an unrequested member
costs zero VRAM. (Dropping `p` from the groups is a possible later
cleanup, not a requirement.)

**Port map (final, 10 models, v269 alphabetical contract):**
9100 creative-fast · 9101 creative-slow · 9102
kokon-flux-image-3-klein · 9103 kokon-pup-flux-image-3-klein · 9104
kokon-pup-qwen-image-2.1-i2i · 9105 kokon-pup-qwen-image-2.1-t2i ·
9106 kokon-qwen-image-2.1-full · 9107 kokon-qwen-image-2.1-turbo ·
9108 qwen3.8-27b · 9109 qwen3.8-27b-vision.
Two properties worth keeping in mind: the **rename alone shifts no
ports** (`kokon-pup-flux-image-3-klein` holds exactly the sort slot of
`kokon-pup`), and the shift that the additive step does cause (full /
turbo / the vLLM pair each +2) is transparent — the service talks to
`/upstream/<id>`, never a port, and the vLLM boot scripts take the
daemon-assigned `${PORT}` arg with the served name as their identity.

**Service (schema 14):** the per-pipeline `vlm` stays the T2I door for
qwen (the explicit `kokon-pup-qwen-image-2.1-t2i` name in both the
`entrypoint` and the `model` field); add an **optional second endpoint**
(name: `vlmVision`) for the image-grounded door. `enhance_prompt`
picks the client by mode, mirroring the existing bundle-key mapping
(`engine.rs`): Generate → `vlm`; Compose and **every image-bearing
mode** (Edit, Inpaint, Outpaint, MultiReference, and 0.12's
area/Expand) → `vlmVision`, **falling back to `vlm` when the pipeline
names no second door** (stock installs and the klein pipeline stay
byte-identical — klein keeps one endpoint, its BFL door).
`absorb_secrets`' key-inheritance (the empty-key-inherits-gateway-key
fix, `793ed0a`) and the "enhancer available" capability check
(`config.rs` ~923) extend to the new field. Invariant: **an image part
never goes to a door without a mmproj** — routing is the guard, and a
violation fails loud (llama.cpp rejects vision tokens without a
processor). The llama-swap `capabilities` say the same
(`in: [text]` vs `in: [text, image]`).

**Native system prompts as a bundle (third-party asset handling).**
The two `system_prompt.txt` files ride as a new deployment data-dir
bundle folder (BFL pattern, not `pup-qwen` pattern: this is **Qwen's
text under the Qwen Research License, not Kokon's** — so the folder
lives in the deployment's data dir with `LICENSE` + provenance
(source repo, revision/etag, sha256 per file), the repo carries only
the reference and the loader code, and no byte of it appears in
commits, tests, logs or chat — same discipline as the BFL bundle).
The loader gains the native shape: the two native prompts replace
Kokon's `t2i`/`edit` texts for the qwen pipelines; **Kokon's own
`area`/`expand` words stay Kokon's** (they are built-in 0.12 prompts,
safe on any sidecar — the 0.12 rebase decision stands) and are sent
when the mode needs them, overriding the native prompt for that
request. Open sub-decision (A/B will decide, D-PE6): compose's
two-step label-then-write can be **collapsed to one native I2I call
with the reference images** — the native model is fine-tuned on
exactly that multi-image task, which Kokon's two-step dance
compensated for in the 8B.

**Output protocol (the wire change).** The native doors answer
`think` + JSON, not bare prompt text. The service's VLM path gains a
**post-processing step, active only for the native bundle**: split on
`
</think>

`, JSON-parse, take `rewritten_prompt`; record `wh_ratio` /
`ratio_follow` in the job view (the planner **keeps size authority** —
the user's aspect selection wins; surfacing the model's ratio as a
suggestion is a possible follow-up, not this task). Malformed reply →
the existing degenerate retry once; a second failure **keeps the raw
(think-stripped) text with a job note** (D-PE9 — the endpoint ran, the
D-FB hard-fail stays for a *down* endpoint). Sampling: the doors'
defaults are the README row (temp 1.0, top_p 0.95, top_k 20, thinking
on); the service's per-request values override on the wire as today,
and the bundle can carry per-mode sampling/max-tokens so the client
sends the native row explicitly (small extension to `pup.rs` /
`vlm.rs`). **Thinking on/off is A/B'd** (contract-faithful default on;
the BFL door proved non-thinking is viable and much faster — keep
whichever the quality check prefers). Context: the doors get
`--ctx-size` covering max_new_tokens (16256 / 24000) + the 4096-token
image budget; KV for a 9B Q4 at 32k is a few hundred MB — fits.

## Build steps

1. **Download + pin** (no GPU needed): `Q4_K_M` of both PE models and
   the I2I `mmproj-bf16` from `prithivMLmods` into
   `models/gguf/Qwen/Qwen-Image-2.1-PE-T2I/` and `…-PE-I2I/` with a
   `provenance.md` each (source URL, sha256, size, license, HF
   revision/etag, date) per the box's provenance convention
   (`docs/fresh-box.md` model list gains the same entries).
2. **Smoke on this llama.cpp tree before any window** (GPU-free or
   brief, vllm-resident): `llama-cli` loads each GGUF (I2I with its
   mmproj), one token each; confirm the T2I door runs **without** an
   mmproj and rejects an image request loudly; confirm the jinja
   template's `enable_thinking` switch behaves both ways (blockers a/b).
3. **Rename the existing door** — *independent, zero behaviour change;
   lands before the A/B in its own small window* (the new doors inherit
   the naming rule from birth, and even a failed A/B leaves the box
   with the cleaner name):
   - `backends/_scripts/serve-kokon-pup.sh` →
     `serve-kokon-pup-flux-image-3-klein.sh` (id == script name);
     `cmd` in the config updated; aliases become the explicit id + the
     legacy `q35,qwen3vl-8b` (transition safety, dropped later).
   - Config-ladder step: model id, matrix var `p`, the description
     texts and the header comment (port map is unchanged — see
     property above); `.last-good` re-seeded.
   - **Service config**: the four `vlm` entrypoints
     (`models[]` ×3 + `externalEngine.vlm`) get the new
     `/upstream/kokon-pup-flux-image-3-klein` path and `model` field =
     the explicit id; drop the stale `kokon-pup` key from
     `data/external-caps.json` if present (the file is keyed by
     gateway id; the new key populates lazily on first fetch).
   - **Repo**: the id appears in test fixtures / doc examples
     (`src/lib/types.ts`, `src/lib/logic.test.ts`, `config.rs` ×3,
     `vlm.rs` ×2, `engine.rs` ×5 — mechanical string updates; the
     `kokon-pup-qwen` fixture is a different, retired-era string and
     stays), `docs/fresh-box.md` (model table, matrix block,
     verification checklist), `docs/service-on-prod-box.md` (config
     seed, two places).
   - Window: the config reload stops loaded models (standard
     window), then verify: `/v1/models` on the renamed door lists the
     explicit id, all three pipelines' Prompt+ path answers through
     the new id, the matrix journal line, vllm revived canonically,
     marker last.
4. **Additive gateway step** (also before the A/B; breaks nothing —
   the service still requests only `p`): add the two new tenants
   (caps: t2i `in: [text]`, i2i `in: [text, image]`; pure-boot scripts
   cloned from the renamed one — model paths, alias = the explicit id,
   `--mmproj` only on i2i, `enable_thinking` per the A/B, ctx per the
   token budget, the README sampling row as door defaults, same image
   clamps and TTLs as today) and widen the sets to
   `qwen-t: qt & (p | pt | pi)`, `qwen-f: qf & (p | pt | pi)`. The
   port map shifts to the final 10-line map (above) — transparent, as
   noted. `.last-good` re-seeded.
5. **Service code** (repo): schema 14 migration (`vlmVision`), client
   selection by mode in `enhance_prompt`, `absorb_secrets` +
   capability checks extended, native bundle loader + JSON
   post-processing in `pup.rs` / `vlm.rs` (unit-tested with fake
   bundles/replies — no real prompt bytes in tests, keys and hashes
   only), mock-gateway harness: if its config carries a VLM entry,
   align it with the new names (it currently references only the
   engine ids). Green: cargo qls-core + qls-service, `npm test`,
   `cd mcp && npm test`, harness.
6. **A/B window** (autonomy protocol — job script, watcher,
   blocking wait last in the launch call; vllm handled per the yield
   rule; vllm revived canonically, marker last): temporary service
   config variants (native doors + native bundle) vs the current
   (shared door + `pup-qwen`), same seeds — t2i (a few cases incl.
   one multilingual), edit (the canary hard edit), compose
   (multi-reference; both the current two-step and the one-shot
   native call), area (painted region). **Measure:** enhance latency
   per mode (thinking on/off), the t2i↔i2i **pup-swap cost** inside a
   TTL (the new axis this design introduces — task-019 removed
   sidecar reloads for family/flavor switches; mode switches now
   reload one 5.6 GB door, page-cache-warm), VRAM peak per new pair
   (must stay inside today's measured pairs' headroom),
   degenerate/JSON-failure rate. The **owner's visual verdict**
   decides adoption (D-QW precedent; the old door stays selectable —
   nothing here forces a default-model change, so the AGENTS
   model-change bar applies to the A/B evidence, not to a DiT swap).
7. **Cutover** (if adopted): a **pure service-config step** (the
   gateway already carries all three doors): both qwen pipelines on
   the native doors + native bundle; rebuild + redeploy the service
   image (bind-mounted shape under `frontends/kokon/`); live
   regression (t2i + edit + compose through the new doors,
   `text_generation` capability, Prompt+ button visible after a model
   switch — the 2026-11-12 glitch class). Rollback = revert that one
   config (one step, D-PE8). Docs: `docs/fresh-box.md` (model list,
   matrix block, final port map), `docs/engineering-notes.md` (PUP
   section), this file + STATUS. If **not** adopted: close with the
   evidence; the additive gateway step stays harmless (or reverts
   with one ladder step), the rename stands.

## Progress (2026-11-12)

**Step 1 — done.** Both Q4_K_M GGUFs + the I2I `mmproj-bf16` are on the
box (`models/gguf/Qwen/Qwen-Image-2.1-PE-{T2I,I2I}/`) with
`provenance.md` each (source = `prithivMLmods/…-GGUF` @ rev
e18d4a3e…/55b9c1a3…, upstream = `Qwen/Qwen-Image-2.1-PE-*` 2026-09-20,
Qwen Research License, sha256 + size recomputed on disk):
T2I `340feb42…` (5,629,108,864 B), I2I `a5c6cb28…` (5,629,108,864 B),
mmproj `8dedb71d…` (921,704,608 B). The `docs/fresh-box.md` model list
gains their entries with the step-4 additive commit (they are not yet
registered in the gateway, so documenting them as live now would be a
lie).

**Step 2 — done (inside the step-4 window).** A full
token-generation smoke needs the GPU, and vllm owns the 24 GB card:
even `llama-cli -ngl 0` dies in this build's CUDA-init
(`cudaStreamCreateWithFlags` OOM) while vllm is resident, so there is
no "CPU-only" path on this box to probe with. What *was* proven
pre-window: the exact named llama.cpp tree loads the sibling BFL
Qwen3.5-9B GGUF end to end (the journal's known-good boot), the PE
files are the same architecture and same gguf reader (blocker a's
architecture half), and their metadata is valid GGUF. The remaining
half ran inside the step-4 window (w028r2e), where the card was free
and the proof was free — see Step 4 below: both PE doors boot on the
named tree (1-token 200, page-cached), the T2I door is mmproj-less and
rejects an image part loudly (HTTP 500, the server-side teeth), the
`enable_thinking` switch behaves both ways (on: think block + JSON
contract; off: no think, same contract), and the JSON contract holds
under the PE system prompt on both doors (text and real-photo input).
Direct-exec pre-window runs of both boot scripts (under vllm
residency) already proved the flags parse and the files load (they
died at the CUDA OOM, exactly the expected signature).

**Step 3 — DONE (the rename window, w028r1d, 2026-11-12 12:34 UTC,
150 s).** Final state, verified after the marker:
- Gateway: 8 models; `kokon-pup` → `kokon-pup-flux-image-3-klein` (still
  4th alphabetically → port **9103 unchanged**); live == `.last-good`
  == staged `w028r1bfinal` (md5 `414416ab…`).
- Boot scripts: only `serve-kokon-pup-flux-image-3-klein.sh` remains
  (the old name removed in the window's success step); `--alias
  kokon-pup-flux-image-3-klein,q35,qwen3vl-8b` (one identity, legacy
  names as extras).
- Service config: all four `vlm` blocks (3 pipelines + externalEngine)
  on the new entrypoint **and** `model` field = the explicit id.
- Window proofs: the renamed door's 1-token probe answered 200 (4 s —
  page-cached) and `/v1/models` lists the id; a klein t2i with Prompt+
  ran end to end (20 s, PNG back); a turbo t2i with Prompt+ did the
  same (20 s); unloads clean (card empty); FINAL config (rename kept,
  vLLM pair re-registered) live and == last-good; vllm back on 9106
  (36 s, warm) answering the 1-token probe; box chat healthy on the
  restored runtime.
- Repo: fixture/doc renames landed (config.rs ×6, vlm.rs ×2, engine.rs
  ×4, types.ts, logic.test.ts, fresh-box.md ×7, service-on-prod-box.md
  ×2); the deprecated-era `kokon-pup`/`kokon-pup-qwen` pair in
  engine.rs (the 0.11 sidecar-routing test) stays as-is on purpose.
  All suites green: qls-core 379, UI 231, MCP 38 (the protocol tests
  need a freshly built service — built on this box).

**The incident (record for the next window-builder).** Three of the
four window attempts failed for reasons the *box* had, not the design:
1. **w028r1 (500 @ P3):** the live `serve-kokon-pup.sh` had been
clobbered *before* the window by an earlier failed draft rewrite (wrong
model dir, no `LD_LIBRARY_PATH`, invalid `--no-cache-reuse`, bare
`--chat-template-kwargs`) — the daemon's `upstream command exited
prematurely` was the only symptom. The original bytes were
unrecoverable; the recipe was **reconstructed from the 10:42
known-good boot in the daemon journal** (model files, aliases,
`-t 8`, ctx, clamps, non-thinking boot) and validated by direct exec.
2. **w028r1b (false FAIL @ P0):** my new content check grepped the
inlined full path while the script uses an `$M` variable — a checker
bug, not a script bug. Checks now match dir + file names.
3. **w028r1c (exec-format-error @ P3):** my sed that derived the
new-name script replaced **line 1 — the shebang** — with a comment;
the daemon's `fork/exec` then failed with `exec format error` while my
`sh script.sh` tests (sh reads the file anyway) and `sh -n` all
passed. **The lesson, now baked into every window's P0 audit: check
`head -c 2 == "#!"` on every boot script, and test the scripts the way
the daemon will run them (direct exec, not `sh`).**
4. **w028r1d: DONE** as above — the audit caught nothing because the
scripts were finally right, and the proofs all landed.

**Step 4 — DONE (the additive window, w028r2e, 2026-11-12 13:46–13:50
UTC, 225 s).** Four attempts; four distinct *checker-side* bugs, fixed
one per attempt (the window design held; the rollback path fired
cleanly each time, vllm revived <60 s every time, the service was
never touched). Final state, verified after the marker:
- Gateway: **10 models** — new tenants `kokon-pup-qwen-image-2.1-t2i`
  (**9105**) and `kokon-pup-qwen-image-2.1-i2i` (**9104**); alphabetical
  slots confirmed against the daemon's own `/running` `proxy` and the
  journal boot lines; live == `.last-good` == staged `w028r2final`
  (md5 `3e5214d2…`). The qwen sets are now `qt & (p | pt | pi)` /
  `qf & (p | pt | pi)` — mode-exclusive: one PUP door per qwen state,
  named by the job's VLM call.
- Boot scripts: two new pure-boot scripts (`serve-kokon-pup-qwen-
  image-2.1-{t2i,i2i}.sh`) cloned from the BFL door's shape: the shared-
  linked `LD_LIBRARY_PATH`, `--reasoning on` (this build's form of the
  `enable_thinking` switch; the old `--chat-template-kwargs` spelling
  is deprecated, not invalid), the reference sampling row as defaults
  (temp 1.0 / top_p 0.95 / top_k 20 — per-request values override on
  the wire), ctx 20480 (t2i) / 32768 (i2i) per the reference
  `max_new_tokens`, `--mmproj` on i2i only, the 1024/4096 image clamps
  on i2i, alias = the explicit id.
- Contract bundle pinned to the data-dir (D-PE3): `system_prompt.txt`
  (t2i 10045 B `a77c9a06…`; i2i 18344 B `e378fea6…`),
  `generation_config.json`, `LICENSE` (byte-identical across both PE
  models — one sha for both) appended to each `provenance.md`; the
  GGUFs' embedded chat templates already carry the `enable_thinking`
  branch (verified in the file strings), so no external template is
  needed.
- **Step-2's remaining proofs, all in-window** (the card was free):
  both PE GGUFs cold-boot on the named Oct-14 llama.cpp (1-token 200;
  ~2–3 s page-cached after the pre-window reads) and serve their
  explicit ids — **blocker a resolved**; the T2I door boots mmproj-
  less and rejects an image part with HTTP 500 `image input is not
  supported - hint: if this is unexpected, you may need to provide the
  mmproj` — **blocker b resolved, the routing invariant now has
  server-side teeth**; `--reasoning off` on the same T2I file
  (scratch direct-exec boot, port 9190) suppresses the think block
  (0 chars, 396 completion tokens) and **keeps** the JSON contract —
  **blocker c resolved, the A/B's off arm works**.
- **The JSON contract under the PE system prompt** (attached from the
  pinned data-dir bundle; structural verdicts only, replies stay in
  `/tmp/w028r2e`): t2i text-only → `{rewritten_prompt: str 2256 chars,
  wh_ratio}`, think 2486 chars / 980 tokens; i2i with a real
  klein-generated 1024² photo → `{rewritten_prompt: str 1016 chars,
  wh_ratio, ratio_follow}` (both ratio fields present — the card's
  mutual exclusion is at value level, which the step-5 post-processing
  must honour), think 7247 chars / 1887 tokens. VRAM with the doors
  resident: t2i 6098 MiB, i2i 7680 MiB — deep inside the 24 GB pair
  headroom.
- **Current behaviour byte-identical**: klein t2i with Prompt+ (22 s,
  PNG 1.3 MB) and turbo t2i with Prompt+ (25 s, PNG 623 KB) ran green
  through the widened sets (the service still names `p` — the BFL
  door — so the solver resolved `{qt, p}` / `{k, p}` exactly as before);
  the service config's md5 is identical before and after
  (`8b8936ff…`); the active model is back on klein.
- vllm revived canonically on **9108** (its shifted slot) in 104 s
  (warm — the weights were page-cached); all kokon ports free at the
  end; the service and its UI healthy.
- Docs: `docs/fresh-box.md` — the 10-model table, the additive history
  note, the `vars`/`sets` blocks, the set-explainer bullets, the PUP
  section, and the verification checklist (the journal example now
  matches what v269 actually logs at info level).

**The additive window's four failed attempts (all checker-side — the
record for the next window-builder).**
1. **w028r2 (P0):** the PE content check grepped the concatenated
   full path while the script builds it from a `$M` variable — the
   r1b lesson again; checks now match dir + file-name parts.
2. **w028r2b (P0):** `grep 'mmproj'` against the T2I script matched the
   script's own *comment* ("no mmproj, no vision") — content audits
   must match **flag lines** (`--mmproj`, `--reasoning`), never bare
   words the script documents in prose (D-AD3).
3. **w028r2c (P2):** the contract assertion expected JSON from a
   *system-promptless* request — a design bug, not a model failure:
   the JSON envelope is conditioned on the PE fine-tune's
   `system_prompt.txt`; without it the model answers with a rich
   plain-text description (which is itself good PE, just not the
   documented contract). The window now pins the system prompt to the
   data-dir and attaches it in the contract probes — exactly what the
   service's bundle will do at runtime in step 5 (D-AD1).
4. **w028r2d (P2):** the `/running` endpoint exposes the bound URL as
   the field **`proxy`**, not `address` — the jq path returned null
   (D-AD2).

**Step 5 — DONE (schema 14 service code, 2026-11-12, six commits, all
  suites green).**

- **`ae971a8` — config: schema 14 (`vlmVision`).** Optional second VLM
  endpoint on `ExternalEngine` and each `ExternalModel` (skipped in
  serialization when absent); schema-13 files load as-is and save as 14
  on first write; `absorb_secrets` keeps / inherits the vision key like
  the plain one. Tests: the 13→14 migration round-trip, key
  keep/inherit/explicit scenarios, all existing literals updated.
- **`c94eea0` — pup: the native bundle family.** `PupBundle` gains the
  native shape (`native`, `native_i2i`, per-mode reference sampling,
  `compose_one_shot`); `load()` dispatches on `manifest.json`
  (`family: "qwen-native"`, both `system_prompt.txt` files present and
  non-empty, `LICENSE` present — the Qwen Research License asset,
  data-dir only); `qwen_system` serves T2I for Generate and I2I for
  every image-grounded mode; the bundle's native system prompt **wins
  over a user-saved custom prompt** (D-PE11 — the fine-tune's output
  format is conditioned on it), and the per-mode sampling row is the
  reference row (16256 / 24000 max tokens) when the manifest omits one.
- **`8638545` — vlm: the native reply decode.** `decode_native` splits
  think + JSON envelope (`rewritten_prompt` mandatory; `wh_ratio` /
  `ratio_follow` mutually exclusive at value level, the w028r2e finding),
  `strip_think` loops (the fine-tune can emit several blocks), the
  decoder tolerates a trailing fragment past the closing brace, and the
  reasoning field is counted, not concatenated. Multi-request mock for
  the retry path.
- **`27e73da` — engine: mode-routed native upsampling.**
  `enhance_prompt` picks the door per mode (D-PE2: Generate → the
  pipeline's `vlm`; every image-grounded mode → `vlmVision`, falling
  back to `vlm` when one door only is named); a native bundle that
  answered through a VLM gets the decode (success → clean prompt +
  `suggested_ratio` on the view; out-of-contract → one warm retry; a
  second failure keeps the raw think-stripped text with a note —
  D-PE9; a *down* door still hard-fails per D-FB); `compose_one_shot`
  collapses the two-step label dance into one I2I call (D-PE13).
- **`593227c` — ui: the suggested-shape hint.** `suggested_ratio` on
  the job view (skipped when absent) surfaces as a quiet
  "Suggested shape: 3:2" line after the writing phase (D-PE12 —
  recorded, never enforced; the planner keeps size authority). Hidden
  in the compact pending container.
- **`fedbcb5` — docs:** the schema-14 surface in `fresh-box.md` (the
  `vlmVision` routing rule, the native doors, the decode/retry, the
  one-step adopt/rollback), `service-on-prod-box.md` (seed config 14,
  migration note), `engineering-notes.md` (the measured door VRAM),
  harness config aligned.
- **Tests:** qls-core **399 passed / 0 failed / 1 ignored** (was 379;
  +20 across config/pup/vlm/engine), UI 231, MCP 35+3 skipped, external-
  engine harness 16/16. Host fact: on this box `cargo test` needs
  `OPENSSL_DIR=/home/linuxbrew/.linuxbrew` and `LD_LIBRARY_PATH=/home/
  linuxbrew/.linuxbrew/lib` (linuxbrew's openssl 4.1); a bare run in a
  fresh shell dies in `openssl-sys` — an environment fact, not code. **Image + deploy:** built
  `ghcr.io/iamskeole/kokon:external-engine-0.12.0-fedbcb5`, container
  recreated, service at rest (`stopped`/not owned), live config still
  schema 13 with byte-identical md5 (`8b8936ff…`) — the native doors
  are wired only by the step-6 cutover, so the box's behaviour is
  unchanged until the owner adopts. The data-dir native bundle
  (`data/assets/pup-qwen-native/`: manifest, both system prompts, the
  Qwen LICENSE, provenance) was created for the window; a strings-style
  check (24-char mid-line fragments, file-based, zero hits) confirmed
  **no native prompt byte in the image or the committed bundles** —
  the text lives in the deployment data dir only (D-PE3, the
  acceptance criterion's `strings` check, done per window P0 since).

**Step 6 — A/B window DONE (w028r3, 2026-11-12 16:25–16:31 UTC, 317 s;
plus w028r3b, 16:43–16:46, 150 s, for the real thinking-off arm).**
Seven jobs, same prompt/seed (1234) per mode across arms; the window
left the box exactly as it found it (the cutover is the staged flip,
below). Measurements (wall time per job = door boot + enhance +
di-t generate, fast quality):

| arm | door + bundle | t2i | edit | compose |
|---|---|---|---|---|
| A — the box as it stood | BFL door + `pup-qwen` | 27 s (232 tok / 7.2 s, 1158 ch) | 18 s (120 tok / 2.5 s, 562 ch) | 28 s (3 calls: 2 labels + scene 195 tok / 6.9 s, 1041 ch) |
| B — native, thinking ON | PE-T2I / PE-I2I + `pup-qwen-native` | 41 s (1303 tok / 21.2 s, 2820→2777 ch, ratio **3:2**) | 45 s (1701 tok / 28.1 s, 1110→1040 ch, follows **<image1>**) | 57 s (**1 call**, 2507 tok / 33.3 s, 1391→1321 ch, follows **<image2>**) |
| B-off — native, thinking OFF (w028r3b, scratch :9190) | PE-T2I + native bundle | 24 s (441 tok / 6.2 s, 2149→2106 ch, ratio **3:2**) | — | — |

- **The native path is clean end to end**: 7/7 jobs decoded in contract
  on the first call (no retry, no D-PE9 fallback note, no think leak in
  any view, no errors); the I2I door served both images-grounded modes
  through the service's VLM client (base64 parts) without issue.
- **D-PE6 decided by evidence, pending owner confirmation:** the
  one-shot native I2I compose (1 call, 33 s, 2507 tokens) beat the
  two-step (3 calls, ~11 s of VLM time, 1041-ch output) on both time
  and prompt richness; the two-step path remains the `false` arm in the
  manifest for rollback.
- **D-PE5 data (on vs off, same door, same seed):** thinking ON = 3×
  more tokens, 3.4× the enhance time, a 31 % longer prompt, identical
  ratio suggestion (3:2). Both are contract-faithful. The quality
  question (longer prompt → better image?) goes to the owner's visual
  verdict on `b-t2i.png` vs `boff-t2i.png`.
- **Determinism check (validates the whole A/B):** the main window's
  first "B-off" job (mis-routed to the BFL door, D-AD10) produced an
  image **pixel-identical** to arm A's (max channel diff 0; only PNG
  metadata bytes differed) — same seed + same prompt + same DiT ⇒
  identical pixels, so the a-vs-b image difference (mean 54.8/255)
  is purely the upsampler's prompt, and the review can be read as a
  prompt-quality verdict.
- **VRAM:** peak 24096 MiB was vllm at P0; the window pairs stayed
  inside the measured headroom (turbo+T2I ≈ 17.4, turbo+I2I ≈
  18.7 GB — both doors resident alongside the engine, no OOM, no
  mid-job eviction; the klein-pair eviction observed in a stray test
  job is a pre-existing property of that pair, not of these).
- **Adoption is staged, not done:** the one-step cutover config —
  schema 14, both qwen pipelines on the native doors (`vlm` = PE-T2I,
  `vlmVision` = PE-I2I) + `pup-qwen-native` bundle; klein unchanged —
  is at **`/home/p/ml/frontends/kokon/config/config.json.adopt-028`**
  (mode 600; the window copy in `/tmp/w028r3/svc-adopt.json` is
  byte-identical, md5 `4baa5379…`) and goes live with
  `cp …adopt-028 config.json && docker restart kokon`; the rollback is
  the window's own restore path (byte-identical pre-window config —
  demonstrated, md5-verified, after both windows).
- **Window-ops record (the 8 failures before the DONE, all mine — the
  box was never at fault; vllm cycled 8× through die()-path restores,
  each self-recovering in <3 min; every cycle noted in
  `/tmp/w028r3-run8`/`/tmp/w028r3*` job logs):** the sampler `$( )`
  deadlock (D-AD8); the tag-vs-digest image check; a blank line in the
  fragment file matching everything in `grep -f`; the identical-swap
  reload (D-AD7); the yield false-positive on a *dying* daemon-stopped
  vllm (fixed with a 90 s observation loop: a dying process exits, a
  respawn reappears in `/running`, a human's stays alive); the inverted
  `|| die` on the yield's no-yield return; the `KAUTH`-as-variable
  header (D-AD11); the host-path image refs (D-AD9); the
  `modelName`-is-a-filename check (the engine health check compares
  the engine's reported file name, not the display name); the
  empty-card wait mid-window (D-AD12); the `models[]`-outrank (D-AD10).

**State at handover (2026-11-12 16:50 UTC):** gateway live ==
`.last-good` == the 10-model registry (md5 `3e5214d2…`); vllm ready on
9108 (its daemon-assigned port — 18020 has not existed since task-022;
wait-marker's 18020 probe is stale and can be updated); the kokon
service at rest with the pre-window config byte-identical (`8b8936ff…`)
on the schema-14 image (`fedbcb5`); the evidence set in
`/tmp/w028r3/` (7 images, 7 views, `jobs-index.jsonl`, `digest.json`,
journal slice, staged `svc-adopt.json`) and the off-arm's in
`/tmp/w028r3b/` (merged into the set).


## Decisions (pre-logged)

- **D-PE1** — klein keeps the BFL door in behaviour; it is klein's
  native PUP. Only the qwen pipelines change what they call.
- **D-PE2** — two **exclusive** native doors behind the qwen engine
  (one 9B pup + the DiT pair per set, 24 GB), routed **per job mode**:
  Generate→T2I door, every image-grounded mode→I2I door. Co-residing
  both pups with the engine is out (VRAM, measured class).
- **D-PE3** — the native `system_prompt.txt` files are third-party
  contract assets: data-dir bundle + LICENSE + provenance, referenced
  never inlined (BFL discipline; the Qwen Research License covers
  them like the DiT).
- **D-PE4** — `wh_ratio` / `ratio_follow` are recorded, not
  authoritative: the planner keeps size authority (the user's choice
  wins). Suggestion UI = possible follow-up.
- **D-PE5** — thinking on/off is an A/B axis, not a foregone
  conclusion; contract-faithful (on) is the default under test.
- **D-PE6** — compose: one-shot native I2I (images in) vs the current
  two-step label+compose is decided on A/B evidence; two-step stays
  only if it wins.
- **D-PE7** — Q4_K_M is the quant class for both doors (Q8 OOM'd the
  full pair, task-014; prithivMLmods' Q4_K_M = 5.63 GB, same class as
  the BFL door that fits).
- **D-PE8** — rollback = one service-config step back to the shared
  door + `pup-qwen`; the renamed BFL door stays registered and in the
  qwen sets throughout, so no gateway work is ever needed to roll
  back.
- **D-PE9** — malformed JSON after the retry: keep the raw
  (think-stripped) text with a job note. D-FB hard-fail remains
  reserved for a down endpoint.
- **D-PE10** — **Naming** (owner, 2026-11-12): one explicit identity
  per door — `kokon-pup-<engine-family>[-<mode>]` — used as the
  gateway id, the door's alias, the service `model` field and the
  boot-script name; the `kokon-pup` rename is an independent
  zero-behaviour step (build step 3) that lands before the A/B;
  legacy wire names survive only as extra aliases on the klein door
  until a cleanup.
- **D-RN1 (drift, 2026-11-12)** — before any window, re-seed
  `llama-swap.yaml.last-good` from the **live** file when they differ:
  the live file had an un-reseeded vLLM `env:` block (added after
  w024) and the last-good was stale; the served, validated file is the
  one to snapshot and restore.
- **D-RN2 (window shape, 2026-11-12)** — the rename window uses the
  three-config pattern: PRE (pre-rename live), WIN (renamed door, vLLM
  pair **out of the registry** — the daemon's reload kills the loaded
  vllm and the registry cannot resurrect it, which is exactly the
  yield-rule-safe way to free the card), FINAL (renamed door, vLLM
  pair re-registered — the terminal state).
- **D-RN3 (root cause, 2026-11-12)** — the first window failure was a
clobbered boot script, not a naming/routing problem; the daemon's
  `upstream command exited prematurely` means "read the journal's boot
  lines before touching the registry".
- **D-RN4 (reconstruction, 2026-11-12)** — the PUP door's recipe is
  reconstructed from its last known-good boot in the journal + the
  config comments + the model dir's provenance; the original file bytes
  are gone. Operational contract proven by the 10:42 journal line set
  and by direct-exec boot tests.
- **D-RN5 (library path, 2026-11-12)** — every llama.cpp script for
  the Oct-14 build sets `LD_LIBRARY_PATH=/home/p/ml/backends/llama.cpp/build/bin`:
  the binary's baked `RUNPATH` (`…/latest/llama.cpp/…`) predates the
  tree's move into `backends/` and is dangling; without the env the
  daemon's exec dies before a flag is parsed.
- **D-RN6 (flag correctness, 2026-11-12, this build)** —
  `--chat-template-kwargs` takes a JSON object string
  (`'{"enable_thinking":false}'`; a bare `k=v` is a parse error),
  `--no-cache-reuse` does not exist, and the known-good thread count is
  `-t 8` (16 is the draft's error). The build logs a deprecation for
  the kwargs spelling (`--reasoning on|off` is the new form) — harmless,
  re-check at the next tree update.
- **D-RN7 (two-script state, 2026-11-12)** — after a failed window the
  box can carry *both* script names; the next window therefore makes no
  mid-window rename: it starts from both (PRE references the old, WIN/
  FINAL the new), removes the old on success, removes the new on
  rollback — each config state has exactly one live script matching it.
- **D-RN8 (shebang audit, 2026-11-12)** — every window's P0 script audit
  now checks, per boot script: exec bit, `head -c 2 == "#!"`, syntax
  (`sh -n` / `bash -n`), plus door-specific content greps — because
  `sh -n` and `sh script.sh` both pass a shebang-less file while the
  daemon's `fork/exec` rejects it (`exec format error`).
- **D-RN9 (this box's test env, 2026-11-12)** — `cargo` on this box
  needs `OPENSSL_DIR=/home/linuxbrew/.linuxbrew` (no system libssl-dev)
  and `LD_LIBRARY_PATH=/home/linuxbrew/.linuxbrew/lib` (the built
  binaries link brew's libssl.so.4); the MCP protocol tests inherit
  that for the service they spawn.
- **D-AD1 (the contract is conditioned on the system prompt,
  2026-11-12)** — the PE JSON envelope is not a property of the weights
  alone: without the fine-tune's `system_prompt.txt` the model answers
  a rich *plain-text* description (still good PE, not the documented
  contract). Door-level contract proofs must attach the pinned system
  prompt (read from the data-dir, never printed); at runtime the
  service's native bundle does the same (step 5).
- **D-AD2 (the `/running` field is `proxy`, 2026-11-12)** — the daemon's
  `/running` exposes each loaded model's bound URL as `proxy` (not
  `address`); window assertions use `.proxy`.
- **D-AD3 (content audits match flag lines, 2026-11-12)** — boot-script
  content checks must grep **flag spellings** (`--mmproj`,
  `--reasoning on|off`, `--temp …`), never bare words: the scripts
  document their own flags in prose comments (the T2I header says
  "no mmproj"), so a bare-word grep false-positives on the prose.
- **D-AD4 (explicit unloads before pair boots, 2026-11-12)** — the
  window unloads (via `/api/models/unload`) any resident door not in
  the next pair's set *before* booting the pair, so the card state is
  deterministic (24 GB: one 9B PUP + one DiT per pair; a second PUP or
  the old door resident would OOM the boot). Matrix eviction is still
  proven for the service's own request flow (the klein/turbo jobs);
  the explicit unload is the window's determinism, not a substitute
  for the set's correctness.
- **D-AD5 (the off arm is a scratch boot, 2026-11-12)** — the
  `enable_thinking` A/B axis is proven by a scratch *direct-exec* boot
  of the same GGUF on a scratch port (9190) with `--reasoning off`:
  it is not daemon-managed, so the job owns its full lifecycle (launch,
  readiness wait, kill, wait-for-exit) and the watcher's cap path clears
  it too. One 9B PUP + the scratch fit the card simultaneously (~14 GB),
  so the arm cost no extra window time.
- **D-AD6 (launch hygiene, 2026-11-12)** — create the workdir *before*
  redirecting the job's output into it, and never a broad `pkill -f`
  (exact pids / process groups); a `pkill` pattern matching the job's
  own cmdline or a bystander's (the owner's `watch -n 1 nvidia-smi`
  processes appeared in every audit) is how windows kill the wrong
  thing.
- **D-AD7 (an identical config swap still reloads, 2026-11-12)** — the
  daemon's `-watch-config` reloads on any file change, and a reload
  stops and reboots every loaded model: a restore path that swaps in a
  config equal to the live one (every pre-P1 failure) cycled the
  owner's vllm twice in ten minutes for nothing. Every swap in the
  window/die/watcher paths is now md5-gated (skip when live == target).
- **D-AD8 (a backgrounded child inside `$( )` deadlocks the script,
  2026-11-12)** — `SAMPLER=$( { loop; } & echo $! )` never returned:
  the backgrounded sampler holds the substitution's pipe open, so the
  read side blocks until the loop exits (the window sat 12 minutes at
  that line with the card untouched). Background loops are plain
  `( … ) &` subshells; `$!` directly.
- **D-AD9 (image references are the service's forms, 2026-11-12)** —
  job bodies name images `input:<id>` / `gen:<id>` (or container
  paths): the service runs in a container where the data dir is
  `/data`, so a host path in `images` fails the job at load time
  (the 7th launch's a-edit).
- **D-AD10 (`models[].vlm` outranks `externalEngine.vlm`, 2026-11-12)**
  — the routing priority is the pipeline's own endpoints first: a
  config that stages a sidecar change only on `externalEngine` is
  silently overridden by the pipeline's `models[]` entry (the main
  window's first "thinking-off" job ran the BFL door + `pup-qwen`
  instead of the scratch — its result was pixel-identical to arm A,
  which is how it was caught). Off-arm configs must set the models[]
  entry.
- **D-AD11 (a function name is not a header value, 2026-11-12)** —
  `-H "$KAUTH"` with `KAUTH` a *function* sends the function's source
  as the Authorization header; the service answers with an empty body
  (no JSON error) and a naive submit loop retries into the timeout.
  Headers are built as plain strings (`"Authorization: Bearer $TOK"`);
  an empty response body from the service means "check my own request",
  not "service down".
- **D-AD12 (wait for the door, not for the card, mid-window,
  2026-11-12)** — after unloading a sidecar *while the engine stays
  resident* (the b-t2i → b-edit transition), the card cannot go empty
  (the engine is ~11 GB): the wait is the model's absence from the
  daemon's `/running`, not a VRAM threshold. Empty-card waits stay for
  the full-teardown points (P1, P6, pre-revival).

## blockers

None that block starting. Open questions, resolved in order during
the task (none expected to be show-stoppers — the architecture is
proven on this exact llama.cpp build):

- **a.** the Oct-14 tree loads *these* GGUFs as-is (gguf metadata
  version, MTP tensor presence in the prithivMLmods files) — **RESOLVED
  in the w028r2e window: both files load and serve (1-token 200, /v1/
  models identity); the MTP-block reader was already exercised by the
  BFL door on the same tree.** No fallback needed.
- **b.** T2I door without mmproj: **RESOLVED (w028r2e): boots clean,
  and an image part is rejected with HTTP 500 `image input is not
  supported - hint: if this is unexpected, you may need to provide the
  mmproj` — the routing invariant's server-side teeth, in the daemon's
  own error text.**
- **c.** whether `enable_thinking=false` keeps the JSON contract: 
  **RESOLVED (w028r2e): it does — the scratch `--reasoning off` boot
  produced the same `{rewritten_prompt, wh_ratio}` JSON with the think
  block absent (396 tokens vs 980); the fine-tune does not rely on the
  think prefix for the envelope.** The on/off quality question remains
  for the A/B (D-PE5).
- **d.** one-shot compose with the native I2I vs two-step — A/B.
- **e.** decode latency of the 9B Q4 with thinking on the 3090
  (hybrid attention should make it fast; the MTP draft variant is the
  lever if it isn't — `qwen35.cpp` in this tree already loads MTP
  blocks).

## Acceptance criteria

- [x] Both Q4_K_M GGUFs (+ I2I mmproj-bf16) on the box, sha256-pinned
      with provenance (step 1, 2026-11-12); the boot/probe half lands
      with step 4/6 (step 2's rest).
- [x] **Rename verified** (pre-A/B window w028r1d, 2026-11-12): the
      shared door answers as `kokon-pup-flux-image-3-klein` (alias list
      includes the legacy names), all four service entrypoints +
      `model` fields updated, no stale caps key was present, all repo
      references updated, all pipelines' Prompt+ path green through
      the new id (klein + turbo jobs in-window), port map unchanged by
      this step.
- [x] **Additive step verified** (w028r2e, 2026-11-12): two new tenants
      boot under their explicit ids (9105/9104, confirmed via /running
      proxy + journal); sets `qwen-t: qt & (p|pt|pi)` /
      `qwen-f: qf & (p|pt|pi)` live in the config and the solver
      resolved the service's requests against them (klein `{k,p}`, turbo
      `{qt,p}` — byte-identical behaviour, both jobs green); final
      10-line port map in the config comment and the docs; the service
      config is byte-identical before/after.
- [x] Schema 14: migration + `vlmVision` inheritance + capability
      check unit-tested; cargo (qls-core 399 / service) / `npm test`
      (231) / mcp (35+3) green; mock-gateway harness aligned (16/16)
      — step 5, `ae971a8…fedbcb5`, deployed as the `fedbcb5` image.
- [x] Native bundle loaded at runtime from the data dir (all 7 A/B
      jobs ran on it); **zero native prompt bytes** in the image or
      committed bundles — the fragment check (mid-line 24-char slices,
      file-based, 0 hits) runs in every window's P0 (step 5/6).
- [x] A/B window per the autonomy protocol (marker last, vllm revived
      canonically, digest): w028r3 (arms A + B, 6 jobs) and w028r3b
      (the real thinking-off arm — the main window's first off-job was
      mis-routed, D-AD10, and caught by the pixel-identity check);
      measurements in the Progress record above; the box restored
      byte-identical after each.
- [ ] **The owner's visual verdict on the 7 images** (the review
      prompt; vision-enabled session): then either the **cutover**
      (one config step + restart — staged persistently at
      `/home/p/ml/frontends/kokon/config/config.json.adopt-028`) with `fresh-box.md` /
      `engineering-notes.md` + this file + STATUS updated, or
      **closed with the evidence** (rename + additive step stand as-is;
      the native doors stay registered, unused).
      Rollback is already demonstrated: both windows' restore path
      returns the pre-window config byte-identical (D-PE8).

## Related (out of scope, noted)

- **Mac engine**: `prithivMLmods/…-MLX` builds of both PEs exist — a
  candidate for a future `engine-mac` task (that repo's own rules
  apply).
- **Pocket distills** (`ML-Intern-lab/…-PE-T2I-Pocket-2B/0.8B`,
  `qwen3_5_text`, text-only): interesting for low-VRAM boxes, note
  only.
- **Stock installs** are unaffected: the in-engine text encoder
  remains their PUP; this task touches only the fork's box topology
  and the service's mode routing (stock path byte-identical, as
  always in this fork).
