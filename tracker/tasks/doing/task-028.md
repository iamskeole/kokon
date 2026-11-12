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

## blockers

None that block starting. Open questions, resolved in order during
the task (none expected to be show-stoppers — the architecture is
proven on this exact llama.cpp build):

- **a.** the Oct-14 tree loads *these* GGUFs as-is (gguf metadata
  version, MTP tensor presence in the prithivMLmods files) — the
  step-2 smoke answers it; fallback sources exist (lightx2v Q8,
  Moseyanling) if a file is at odds with the tree.
- **b.** T2I door without mmproj: confirm the qwen35 model in this
  tree boots mmproj-less and fails loud on image parts (the routing
  invariant's server-side teeth).
- **c.** whether `enable_thinking=false` keeps the JSON contract
  (the fine-tune may rely on the think prefix) — the step-2 smoke
  plus the A/B answer it.
- **d.** one-shot compose with the native I2I vs two-step — A/B.
- **e.** decode latency of the 9B Q4 with thinking on the 3090
  (hybrid attention should make it fast; the MTP draft variant is the
  lever if it isn't — `qwen35.cpp` in this tree already loads MTP
  blocks).

## Acceptance criteria

- [ ] Both Q4_K_M GGUFs (+ I2I mmproj-bf16) on the box, sha256-pinned
      with provenance; both boot on the named llama.cpp (1-token
      probes; i2i with an image part); T2I boots mmproj-less.
- [ ] **Rename verified** (pre-A/B window): the shared door answers as
      `kokon-pup-flux-image-3-klein` (alias list includes the legacy
      names), all four service entrypoints + `model` fields updated,
      stale caps key dropped, all repo references updated, all three
      pipelines' Prompt+ path green through the new id, port map
      unchanged by this step.
- [ ] **Additive step verified**: two new tenants boot under their
      explicit ids; sets `qwen-t: qt & (p|pt|pi)` /
      `qwen-f: qf & (p|pt|pi)` in the journal matrix line; final
      10-line port map in the config comment; current service
      behaviour byte-identical through this step.
- [ ] Schema 14: migration + `vlmVision` inheritance + capability
      check unit-tested; cargo (qls-core + service) / `npm test` /
      mcp green; mock-gateway harness aligned.
- [ ] Native bundle loaded at runtime from the data dir; **zero
      native prompt bytes** in the repo, tests, logs or chat (keys
      and hashes only) — `strings`-style check on the image, as
      task-007 did for BFL.
- [ ] A/B window per the autonomy protocol (marker last, vllm
      revived, digest): standing set complete, measurements in the
      digest, owner's visual verdict recorded.
- [ ] If adopted: cutover live on the box (service-config-only),
      one-step rollback demonstrated, `docs/fresh-box.md` +
      `engineering-notes.md` + STATUS current. If not: closed with
      the evidence; rename + additive step as-is.

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
