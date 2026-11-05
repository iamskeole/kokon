# task-019 — one shared PUP sidecar across all image families

**Status: CURRENT (2026-11-05 — task-020/021 closed this span; the spec
is locked in the last section — door quant, names, sampling, image
clamps, sets, service repoint, proof window). Implementation lands in
`backends/_scripts/serve-kokon-pup.sh` + the config swap; the proof
window (`w019`) runs the Klein→turbo→quality sequence with the door pid
asserted stable across both switches.**

## goal

Today every image family runs its **own** PUP sidecar process, and the
gateway router (llama-swap) is process-granular: it has no concept of
"these processes loaded the same file". A klein → qwen switch (or even
turbo → quality) therefore stops one PUP process and cold-loads another
— re-reading the same weights from disk into VRAM — even though the
file, and often the quant, are identical. The owner's question (2026-11-04,
when closing task-014): *"would it make loading/unloading more
efficient if we standardized the lower PUP quant across all
variations — would llama-swap evict just the image parts, or always
reload the whole bundle?"* Answer: it always reloads the whole bundle,
but the **v269 matrix router is the first router that can express the
fix**: a model may be a member of *several* sets (groups could not —
one group per model, which is why task-013 built two doors).

Goal: one PUP sidecar process (one model entry, one quant, one flag
set) as a member of the klein set, the qwen-t set and the qwen-f set,
so that **any** image-family request keeps it resident and only the DiT
engine reloads on family or flavor switches.

## what it would change

- **Today (post re-land, 2026-11-05):** three independent PUP processes,
  each a pure-boot daemon-tracked model — `kokon-pup` :39175 (Q8_0, the
  klein door), `kokon-pup-qwen` :39176 (Q8_0, the turbo door),
  `kokon-pup-qwen-f` :39177 (Q4_K_M, the quality door). Every cross-family
  or cross-flavor switch = one door stopped, another loaded: ~25 s (Q4,
  warm page cache) to ~1–1.5 min (Q8) of sidecar load per switch.
  (The old klein joint supervisor that shared a lifetime with the klein
  engine is retired by the re-land; the doors are standalone again.)
- **After:** one `kokon-pup` entry (Q4_K_M, ~7.3 GB), member of all
  three image sets. Switches reload only the DiT (the ~8–14 GB, the
  real cost, unavoidable per family). The sidecar load disappears from
  every switch; the 600 s TTL on the one entry (one idle clock,
  refreshed by any family's traffic) unloads it after 2 min of total
  image inactivity, as today.

## preconditions (must be decided before work)

1. **One file, one quant: Q4_K_M for every family.** VRAM fits every
   pair (klein 8.4 + 7.3 ≈ 15.7 GB; turbo 8.7 + 7.3 ≈ 16.0 GB;
   full 13.6 + 7.3 ≈ 20.9 GB — all under 24 GB; the Q8 full door OOM'd
   exactly why Q4 exists — task-014, measured). Quality input already
   in hand: the owner found the 9B quants "virtually indistinguishable"
   (task-014 close, 11-04), and klein has prior PUP-at-Q4 measurements
   from task-010 (window C/D: clean at all three quants on the
   production path). Re-confirm visually on the klein side during the
   verification window.
2. **One flag set.** The three doors differ:
   - klein (:39175): `--image-min-tokens 0 --image-max-tokens 0
     --top-p 0.95 --top-k 20 --min-p 0` (explicit sampling)
   - qwen doors (:39176/77): `--image-min-tokens 1024
     --image-max-tokens 4096` (BFL maker-spec clamp), default
     sampling, `--alias qwen3vl-8b`
   The image-token window changes what the VLM sees per image, so
   whichever family's flags win needs a small prompt-quality re-check
   (a task-014-style A/B lite on the losing family).
3. **Lifetime — DEAD (re-land, 2026-11-05).** The original lifetime
   rework (freeing the klein sidecar from the joint supervisor) is
   already done: the re-land retired the supervisor, and every PUP
   door is now an independent daemon-tracked model with its own
   pure-boot script and the default group stop. No lifetime work
   remains; the only lifetime consideration is the one shared TTL
   clock noted in "what it would change".

**New mechanics note (post re-land):** one process, two names.
llama-server serves its loaded model under both its native GGUF name
and its `--alias`, so a single door can answer the klein pipeline's
vlm entry (native name) and the qwen pipelines' vlm entry
(`qwen3vl-8b`) at once — the service config's per-pipeline vlm names
need no change. Verify both names resolve in the window.

## acceptance criteria

- One PUP model entry (Q4_K_M, one flag set — owner picks which family's
  baseline; the native name + `qwen3vl-8b` alias both served) as a
  member of the klein, qwen-t and qwen-f sets (the sets share the `p`
  var: `klein: "k & p"`, `qwen-t: "qt & p"`, `qwen-f: "qf & p"`);
  config `-validate` clean; one-copy rollback = the current
  three-door `.last-good`.
- Verification window (no vLLM phases — owner rule): a klein job, a
  turbo job, a quality job, each after a cross-family/cross-flavor
  switch, with **the PUP pid proven stable across the switches**
  (`/proc/<pid>` + the journal's matrix decision lines showing no
  sidecar eviction); both served names answer (native + alias); a
  VRAM sample per pair; visual re-check of one render from the
  flag-losing family (owner's eyes).
- Measured before/after: sidecar seconds removed per switch class
  (klein↔qwen, turbo↔quality).

## blockers

None hard — no task dependency; it is an efficiency/ops improvement on
top of the re-landed topology. It waits on: (a) task-020/021 closing
(queue order), (b) the owner's flag-set choice (precondition 2).

## decisions

- **Parked by the owner, 2026-11-04** ("park but record"): the idea and
  its analysis stay here; scheduling is open. It is the realization of
  the original task-018 note ("one qwen sidecar serving both taste
  groups") and only became expressible with the matrix router (015).
- Not done as part of task-014/015: each of those closed with the
  owner's "not worse, not different" bar on *quality*; this changes
  load mechanics (and one family's sidecar flags), which wants its own
  window and its own verdict.
- **Re-scoped by the owner, 2026-11-05 (not retired):** in the
  discussion that fixed the re-land's shape, the owner chose three
  separate PUP scripts for the re-land itself (zero quality risk on
  the cutover) and re-scoped this task to the shared-door end state.
  The new layout changed only the lifetime half of the work
  (precondition 3 is dead — the supervisor is gone); the quant
  unification, the flag decision and the pid-stability window stand
  as written above. It stays parked ahead of task-020/021 in the
  queue per the standing convention.

## Spec locked (2026-11-05, owner decisions incorporated)

Everything below is decided; the only open call was the door's
sampling defaults, resolved in the final discussion of the span:

- **Model/quant:** one door process — the BFL PUP fine-tune
  (`BFL/flux-3-image-klein-pup`, a Qwen3.5-9B-based VLM) at
  **Q4_K_M** (the only quant that fits all three pairs in 24 GB;
  owner's call, per task-014's OOM data). All three current doors
  already load this same fine-tune (Q8_0/Q8_0/Q4_K_M), so no family
  changes weights — only process count (3→1) and quant for two of
  them (Q8_0→Q4_K_M, owner-accepted, "virtually indistinguishable
  between pup quants", 11-04 verdict).
- **Names:** the GGUF's own name is `Flux3_Pup_Hf`, but this llama-server
  build reports the **model path** as the `/v1/models` id (w020r proof)
  and accepts arbitrary `model` strings leniently (the klein service's
  `q35` rides on that leniency today). To make the contract explicit
  instead of lenient, the shared door carries **both service names as
  aliases: `--alias q35 --alias qwen3vl-8b`** (the klein service entry
  requests `q35`; both qwen entries request `qwen3vl-8b` — legacy name
  from the 8B era; it is the wire contract, not a statement about the
  model size). One process answers both; the window asserts both names
  in `/v1/models`. The `--alias` mechanism is already proven on the
  qwen doors.
- **Sampling defaults = Qwen3.5-9B's official non-thinking "general
  tasks" row, verbatim:** `--temp 0.7 --top-p 0.8 --top-k 20 --min-p 0
  --presence-penalty 1.5` (repetition 1.0 = the server default,
  omitted). Owner's reasoning: the door is not aiming for
  determinism (temp 0 "is finicky"), and the door only ever runs in
  non-thinking mode, so the non-thinking row is the right baseline
  (not the thinking row, not my earlier temp-0 proposal — withdrawn
  after the owner's orthogonality correction: "no sampler" (argmax)
  and "temperature 0" are different dials that happen to coincide in
  vLLM/HF/llama.cpp for a single-token decision, because any
  top-k/top-p truncation keeps the argmax; the practical difference
  is `min_p`, which the row pins at 0 as do we).
- **Thinking stays off:** `--jinja
  --chat-template-kwargs '{"enable_thinking":false}'` (all current
  doors already do this; matches BFL's reference, which disables it
  in both its vLLM upsampler and the model card's standalone block —
  a PUP must return one clean caption, thought tags would poison the
  DiT prompt). BFL klein traffic is unaffected by any of this: the
  kokon aide sends its own per-request values on the wire (vllm.rs
  sends temperature/top_p/top_k/presence_penalty; the BFL branch
  sends temperature 0 → pure argmax, plus the reference's per-task
  presence penalty 1.5 text-to-image / 0 edits — verified to match
  BFL's `pup.py` exactly), which override the door defaults.
- **Image clamps (locked after the per-workload sizing trace of the
  span):** `--image-min-tokens 1024 --image-max-tokens 4096`, ctx
  8192, batch/ubatch 1024, `--n-gpu-layers 999 --flash-attn on`,
  mmproj-F16. The processor (patch 16, merge 2 → 1024 px/visual
  token, read from the fine-tune's own mmproj metadata; the file
  carries no pixel limits, so these flags set the clamps): 1 MP
  photo ≈ 1024 tokens, the 4 MP cap = 4096 tokens. The 1024 floor
  is the qwen doors' current value (zero behavior change for that
  family), llama.cpp's own recommendation for Qwen-VL grounding
  (its startup warning cites the <1024-token case), and the right
  floor for the BFL bbox workloads; the only behavior change is
  klein edits of sub-1 MP inputs getting a 1 MP upscale instead of
  pass-through (owner: "sub 1MP is never going to be an issue").
- **Config:** one model entry `kokon-pup` (cmd → the new
  `serve-kokon-pup.sh`, proxy :39175, checkEndpoint /health, ttl
  600, unloadTimeout 120); sets become `klein: "k & p"`,
  `qwen-t: "qt & p"`, `qwen-f: "qf & p"` (one `p` member in all
  three); the two extra door entries (`kokon-pup-qwen`,
  `kokon-pup-qwen-f`) retire; 10 models → 8. `.last-good` re-seeded;
  the swap happens inside the proof window (a successful edit is a
  hot reload that stops loaded models — vLLM included — so the
  window's P1 owns the swap and the owner's message errors once,
  by design, until P6 restores vLLM).
- **Service repoint:** the kokon service config's three VLM
  entrypoints all point at `/upstream/kokon-pup` (klein keeps model
  `q35`; both qwen entries keep `qwen3vl-8b`); nothing else in the
  service config changes. The two retired door scripts park in
  `__new_mess/pup-doors-2026-11-05/`.
- **Proof window (`w019`, no vLLM phases — the swap's unload/restore
  is the only vLLM contact, same protocol as w020r):** preflight →
  P1 config swap + hot-reload wait + empty-state assert → klein pair
  boots (record the door pid; both alias names verified in
  `/v1/models`) → turbo switch: **door pid asserted unchanged, only the klein engine
  evicted** (journal `evict=[kokon-klein]`) → quality switch: **door
  pid still unchanged** (`evict=[kokon-turbo]`) → explicit UNLOADs
  (the door's group-stop measured) → vLLM restored last → DONE
  marker last. Digest carries the before/after sidecar seconds
  (baseline: w020r's P2/P3 at 9 s / 11 s, each including a full door
  reload; the new world expects zero door work on the switches) and
  a VRAM sample per pair (expect the Q4 door to free ~1.7 GB vs the
  Q8 turbo door and remove the Q4-for-full OOM constraint for the
  turbo pair).
