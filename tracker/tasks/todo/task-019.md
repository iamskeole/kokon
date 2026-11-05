# task-019 — one shared PUP sidecar across all image families

**Status: parked (re-scoped 2026-11-05 — see the last section). Still the
first file in the queue; it becomes current after task-020/021 close.**

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
