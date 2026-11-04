# task-019 — one shared PUP sidecar across all image families

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
but the **v269 matrix router — now live (task-015) — is the first
router that can express the fix**: a model may be a member of *several*
sets (groups could not — one group per model, which is why task-013
built two doors).

Goal: one PUP sidecar process (one model entry, one quant, one flag
set) as a member of the klein set, the qwen-t set and the qwen-f set,
so that **any** image-family request keeps it resident and only the DiT
engine reloads on family or flavor switches.

## what it would change

- **Today (stock-015):** klein sidecar = `kokon-pup` :39175,
  Q8_0, child of the klein joint supervisor (one lifetime with the
  klein engine); qwen doors = `kokon-pup-qwen` :39176 (Q8_0) and
  `kokon-pup-qwen-f` :39177 (Q4_K_M), separate processes. Every
  cross-family or cross-flavor switch = old PUP process killed, new
  one loaded: ~25 s (Q4, warm page cache) to ~1–1.5 min (Q8) of
  sidecar load per switch.
- **After:** one `kokon-pup` entry (Q4_K_M, ~7.3 GB), member of all
  three image sets. Switches reload only the DiT (the ~13–20 GB, the
  real cost, unavoidable per family). The sidecar load disappears from
  every switch; the 120 s TTL clock on the one entry still unloads it
  after 2 min of total image inactivity, as today.

## preconditions (all three must be decided before work)

1. **One file, one quant: Q4_K_M for every family.** VRAM fits every
   pair (turbo ≈ 13.6+7.3, full ≈ 13.4+7.3, klein ≈ 13.6+7.3 — all
   under 24 GB; the Q8 full OOM was exactly why Q4 exists). Quality
   input already in hand: the owner found the 9B quants "virtually
   indistinguishable" (task-014 close, 11-04), and klein has prior
   PUP-at-Q4 measurements from task-010 (window C/D: clean at all
   three quants on the production path). Re-confirm visually on the
   klein side during the verification window.
2. **One flag set.** The two current sidecars differ:
   - klein (:39175): `--image-min-tokens 0 --image-max-tokens 0
     --top-p 0.95 --top-k 20 --min-p 0` (explicit sampling)
   - qwen doors (:39176/77): `--image-min-tokens 1024
     --image-max-tokens 4096` (BFL maker-spec clamp), default
     sampling, `--alias`
   The image-token window changes what the VLM sees per image, so
   whichever family's flags win needs a small prompt-quality re-check
   (a task-014-style A/B lite on the losing family).
3. **Lifetime rework.** The klein sidecar is today a *child* of the
   klein joint supervisor (one process lifetime shared with the klein
   engine). Making it a gateway-managed entry means: klein supervisor
   → engine-only supervision; the sidecar gets its own
   serve/stop pair (generalize `qwen-pup-serve-9.sh` with the chosen
   flags, port 39175). The klein set stays `k & p & v`. vLLM safety is
   preserved: the `vllm-serve.sh` fully-down branch runs
   `evict-gpu.sh`, which already kills :39175 — the wrapper, not the
   set, is the final GPU arbiter (task-015's booting-branch logic
   unchanged).

## acceptance criteria

- One PUP model entry (Q4_K_M, one flag set — owner picks which
  family's baseline) as a leaf of the klein, qwen-t and qwen-f sets;
  config `-validate` clean; one-copy rollback to stock-015.
- klein supervisor engine-only; sidecar its own cmd/cmdStop.
- Verification window (task-015 style, marker last): a klein job, a
  turbo job, a quality job, each after a cross-family/cross-flavor
  switch, with **the PUP pid proven stable across the switches**
  (`/proc/<pid>` + the journal's matrix decision lines showing no
  sidecar eviction); vLLM request mid-pair still reclaims cleanly
  (evict-gpu path); VRAM sample per pair; visual re-check of one
  render from the flag-losing family.
- Measured before/after: sidecar seconds removed per switch class
  (klein↔qwen, turbo↔quality).

## blockers

None hard — no task dependency; it is an efficiency/ops improvement on
top of the locked task-014/015 state. Owner's flag choice (precondition
2) is the one decision it waits on.

## decisions

- **Parked by the owner, 2026-11-04** ("park but record"): the idea and
  its analysis stay here; scheduling is open. It is the realization of
  the original task-018 note ("one qwen sidecar serving both taste
  groups") and only became expressible with the matrix router (015).
- Not done as part of task-014/015: each of those closed with the
  owner's "not worse, not different" bar on *quality*; this changes
  load mechanics (and one family's sidecar flags), which wants its own
  window and its own verdict.
