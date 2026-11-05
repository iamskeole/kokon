# Task-021 — the vision vLLM variant (qwen3.8-27b-vision, same :18020) + the "add a model / model combo to the gateway" guide

**Created:** 2026-11-04 — after task-020 closed on `config.yaml.stock-016`.
The box has the pure-orphan vLLM main model (`qwen3.8-27b`, :18020); the
owner wants the **vision variant** of the same checkpoint served the same
way, as a second pure-orphan model (`qwen3.8-27b-vision`), plus a **written
guide** in `MATRIX.md` for adding any new model or model combo to the
gateway, so the next person (human or agent) never has to rediscover the
topology.

**Same-port decision (owner, 2026-11-04, mid-recon):** "i think we can use
the same port since both cant be active at the same time? given the vram
requirements." ADOPTED: both variants share **:18020** (the original
:18022 plan is dead). The port is **not** a model's identity — the
process's `--served-model-name` is. Both variants are pure orphans, so a
request for either makes the daemon evict the resident variant first
(cmdStop, 30 s SIGKILL backstop) before running the other's cmd; only one
process can hold the 24 GB card at a time anyway (0.97 of 24 GB).
Consequences baked into the design: (a) the wrappers are **model-aware**
(up/gone/booting checks match the served name in the process argv, never
the port — a 200 on :18020 might be the OTHER variant, which would 404 our
model name); (b) the wrapper's LAUNCH branch must wait for the port to
clear before launching (bounded: the daemon's stop backstop guarantees
≤30 s after its stop signal); (c) while the vision variant is resident, a
request for `qwen3.8-27b` (this session's route) gets a clean 404-class
error until a main request evicts it — the two simply cannot co-reside.

## Reopened (2026-11-05)

Same cause as task-020: the owner's `/home/p/ml` cleanup parked this task's
artifacts (the model-aware `vllm-serve-vision.sh`, the variant-gated
`start_qwen.sh`/`serve-vision.sh` of the bespoke repo, the 0.27.1 venv with
the ported tower-offload patch, `config.yaml.stock-017`, the `m021-*` jobs,
`refs/{thin-021,post-021}`) in `__new_mess/` and re-based the vLLM runtime on
`backends/hyperqwen-032` (new plain `serve-qwen3.8-27b-vision.sh`; the tower
offload is native in that tree). The vision variant, the 2 MP pixel cap, the
offload and the KV headroom all need re-landing and re-measuring on the new
tree: the new script currently runs 8000M KV / 4M-pixel longest-edge, not the
measured 7372M / 2 MP budgets of the old variant. **Re-scope: pending
discussion with the owner (2026-11-05 session).**

## Goal

1. **One listener, two identities, on :18020.** The vision variant
   (`qwen3.8-27b-vision`) boots through a new model-aware wrapper
   `vllm-serve-vision.sh` with the **task-020 wrapper contract** (pure
   orphan; no out-of-band kills; direct-parent `--no-scope` launch;
   adopt / wait / launch branches; 90 s concurrent-boot grace; persistent
   self-log — now `/tmp/vllm-serve-vision.log`; `gpu-empty.sh` as
   last-resort zombie net only, unchanged: its exclusions are
   port-based and cover both variants). `vllm-serve.sh` is re-derived to
   the same model-aware logic (it was port-based; its identity checks
   now match `--served-model-name qwen3.8-27b`).
2. **One launcher, env-gated variants.** `single-user/start_qwen.sh`
   gains `SERVED_NAME` (default `qwen3.8-27b`) and `VISION`
   (default 0 = `--language-model-only`; 1 = tower loads, no flag).
   `serve-vision.sh` (stable dir) passes `SERVED_NAME=qwen3.8-27b-vision
   VISION=1` plus its own context knobs (MAX_LEN 131072, MAX_SEQS 2);
   `serve.sh` passes the main values. Both fronts' do_start guards
   become variant-aware (a port held by a FOREIGN served name is a
   different failure than one held by our own).
3. **The guide.** `MATRIX.md` gains "Adding a model / model combo to the
   gateway": the five pieces every new model needs (the entry —
   proxy/cmd/cmdStop/ttl/capabilities; set membership, or none for an
   orphan; the wrapper contract for anything the daemon can't supervise
   directly; the versioned-config + `-validate` + window protocol; the
   journal lines that prove it works), with the vision variant as the
   worked example of **two models, one port**. Reflection in
   `docs/fresh-box.md` (the model list gains its 10th model).
4. **Lands as `config.yaml.stock-017`** via the standard window
   (window-021 = stock-017 + `logLevel: debug`; marker last; the launch
   call IS the blocking wait).

## Blockers

- **The window interrupts this session's chat route** (it rides the main
  vLLM; the vision boot evicts it). The standard accepted protocol: the
  owner is awake for the window; the marker wakes the agent onto a
  restored box.
- **task-022 (one-door topology) is still parked on the owner's two
  decisions** (vLLM keyless-on-loopback vs shared key; kokon UI
  0.0.0.0 vs loopback). It is **not** a blocker: the vision variant
  builds the current way (0.0.0.0 bind, the canonical `api_key.txt` key
  via the start-script fallback, same as main) and the 022 outcome is a
  one-line adjustment to both variants afterwards.
- No free 24 GB card: the vision fit (weights 22 GB + KV cache + the
  vision tower) must be **measured in the window** (P2/P2b VRAM
  captures), not assumed. The KV budget currently inherited from main
  (`--kv-cache-memory-bytes=7372M`, "stability for spikes, ~1.5 GB
  headroom") may not be the right number with the tower loaded; if the
  measured headroom is not ~1.5 GB, `serve-vision.sh` gets the adjusted
  number **with its source** (the measured default carries its source —
  house rule).

## Decisions

- **Same port :18020 for both variants** (owner, 2026-11-04) — see header.
  Identity = the served name in the process argv; the wrappers match on
  it, never on the port.
- **Vision flags** (recon, 2026-11-04, verified against
  `models/Qwen3.8-27B-W4A16-AutoRound-fast/config.json`): the checkpoint
  is genuinely multimodal (`Qwen3_5ForConditionalGeneration` with a
  `vision_config`; the current launch just passes
  `--language-model-only`, which makes vLLM skip the tower). The vision
  variant = the main launch **minus** `--language-model-only`, with
  `--served-model-name qwen3.8-27b-vision` and the vision context knobs
  (MAX_LEN 131072 / MAX_SEQS 2 — the tower costs context budget).
  Everything else (0.97 gpu-util, MTP draft, fp8 KV, the parsers, the
  key) is identical: same weights, same measured performance shape.
  Engine parameters otherwise stay unchanged (task-020 constraint).
- **This session keeps using `qwen3.8-27b`** (the text variant). It will
  404 while the vision variant is resident on :18020; any main request
  (including the session's next one) evicts vision and restores the
  route. Actually consuming vision from an agent is a host-runtime change
  (the qwen-fastani prompt/endpoint config) — the owner's domain, out of
  scope here; the window proves the model works, not that the agent uses
  it.
- **The old task text said :18022** — superseded by the same-port
  decision above; every artifact in this task is built same-port.- **KV headroom (a1 + a6 measured, 2026-11-04): keep `7372M` as-is.** The
  vision variant totals **24128 MiB** (448 MiB headroom to the 24576
  ceiling) vs the main variant's **23236 MiB** (1340) — the vision tower
  costs ~900 MiB. `--kv-cache-memory-bytes` is a **cap, not a target**
  (vLLM allocates at most that much), and `MAX_SEQS=2` bounds the
  batch spikes the comment worries about, so the tighter headroom is
  acceptable; the measured 448 MiB is recorded here as the source. If a
  concurrent image+text load ever OOMs, the knob to turn is lowering
  the cap (vLLM then auto-caps `max_num_seqs`/`max_model_len`), and the
  number gets re-measured with its source at that point.


## Progress (2026-11-04)

- **File surgery done (host files, non-git; snapshot `refs/thin-021/`
  holds the pre-window generation, reconstructed + byte-verified):**
  - `single-user/start_qwen.sh`: `SERVED_NAME`/`VISION` env gate
    (default = current main behavior, byte-identical launch line when
    unset — verified by re-apply round-trip).
  - `serve.sh` / `serve-vision.sh` (stable dir): variant-aware fronts —
    `served_pid()`/`holder_name()` helpers; do_start: healthy-ours →
    no-op; port held by foreign name → refuse naming it; fresh-core
    boot-grace → wait then re-check (ours+healthy → no-op, foreign
    healthy → refuse, port re-held → refuse, boot died → launch); stale
    core → stop. Both keep the task-020 `start --no-scope` exec path
    (direct parent) and the scope path for humans.
  - `vllm-serve-vision.sh` (new) and `vllm-serve.sh` (re-derived):
    model-aware entry state machine — ADOPT (mine healthy: watch until
    gone, 120 s hung-unhealthy release guard), WAIT (mine booting: never
    double-launch), foreign-holds (port held by a different served name
    / fresh core / unattributable healthy: wait for the router's evict
    to clear it; still held at budget → exit 1 naming the holder, never
    kill out of band), LAUNCH (fully down: `gpu-empty.sh 900` → direct
    child `FRONT start --no-scope` → 1 s-tick loop, `saw_mine` latch,
    90 s grace: stuck-kill after grace, concurrent-boot watch during
    grace — ours → adopt, **foreign → exit 0 without adopting**, the
    request gets a clean error and later requests retry with the port
    properly evacuated). Self-logs: `/tmp/vllm-serve.log` (main),
    `/tmp/vllm-serve-vision.log` (vision).
  - `config.yaml.stock-017` = stock-016 + the vision entry (proxy
    :18020, `cmd` = the vision wrapper, `cmdStop` = `serve-vision.sh
    stop`, `unloadTimeout: 240`, capabilities in:[text,image]
    out:[text] tools, context 131072, in **no set**).
    `config.yaml.window-021` = stock-017 + `logLevel: debug` (the
    deferral lines are Debugf). Both: `-validate` → "10 model(s), 0
    peer(s)" on the pinned v269 binary.
  - `jobs/m021-{test,restore,watch,digest}.sh` written (`sh -n` clean):
    P0 preflight (live==stock-016; window-021 validates; key drift
    assert: listener-env key == `api_key.txt`; image ports free;
    exec-bit audit; SLOG byte offsets for the digest slice); P1 swap to
    window-021 (session route down from here); P2 vision request boots
    + answers, identity proofs (`/v1/models` == exactly
    [qwen3.8-27b-vision], port-name == vision), VRAM capture; P2b an
    actual image in → a description out (generated 64×64 PNG, data-URI
    in an OpenAI-style chat message; proves the tower is real); P3 main
    request → journal `starting swap for model qwen3.8-27b, evicting
    [qwen3.8-27b-vision]` (orphan-vs-orphan, direction 1) + session
    route back; P4 klein job (weights pre-warmed against the a5
    page-cache failure) → done + journal `evict=[qwen3.8-27b]` (the
    sets are untouched); P5 main request → journal evict containing
    `kokon-klein` (orphan-vs-orphan, direction 2, box recovery); P6
    cutover to stock-017 + canonical MAIN revival (the session rides
    main) + final asserts.
    `m021-restore.sh`: stock-016 + thin-021 generation + canonical MAIN
    last (variant-aware: healthy main → leave; foreign holder → stop →
    main). `m021-watch.sh`: 42 min hard cap (job self-deadline 36).
    `m021-digest.sh`: journal decision lines, the identity proofs, the
    P2b description, both SLOGs sliced to the window, VRAM, final state.
- **Rehearsal (fake-vllm kit, `/tmp/vllm-wrap-rh/`; the fake process now
  execs with a vllm-shaped argv so the wrappers' name matching resolves
  exactly as in production):** R1 LAUNCH-from-down → adopt own boot →
  SIGTERM → exit 0 ✓; R3 ADOPT-at-entry → kill → exit 0 ✓; R2
  foreign(main-name) holds at entry → wait → foreign dies → LAUNCH →
  adopt → exit 0 ✓; R4 child aborts before binding + FOREIGN appears in
  the 90 s grace → **exit 0 without adopting, foreign survives** ✓; R6
  foreign holds past the budget → exit 1, names the holder, never
  touches it ✓. R5 (main-variant derivation) accidentally validated the
  strongest guard of all: run on this box, it saw the **real** main
  vLLM's name match and **refused to double-launch** for its full wait
  budget — the no-second-boot invariant holds even when a same-name
  process is alive and unhealthy. The main wrapper's LAUNCH path is
  covered by the window itself (P3/P5 invoke it for real).

## Progress (continued — wrapper hardening, attempts 1-2, 2026-11-04)

**m021 attempt 1 (17:32–17:38, FAIL at 345 s) — the vision variant is
proven end-to-end; the failure was a test assertion plus a real wrapper
defect.**

- **The vision variant works.** P2's request booted it through the
  router on :18020: boot 188 s (engine init 129.82 s incl. 75.97 s
  compilation; the port binds only at the *end* of init — "Application
  startup complete"), `/v1/models` returned **exactly**
  `['qwen3.8-27b-vision']` (max_model_len 131072, fp8 KV 7372 MiB as
  configured, no `language_model_only`), the 1-token completion was 200.
  VRAM: 24128 MiB (see the KV decision).
- **P2's assertion was wrong (our bug).** It demanded the set line
  `matrix: model=… set= dsl="" … cost=0`, which v269 prints only for an
  orphan request **with evict targets**; a box-empty orphan takes a fast
  path that prints no set line at all (`matrix: model=X starting (no
  models running)` + `starting swap for model X, evicting []`). Fixed to
  that shape and dry-run against a1's own journal before relaunch
  (the standing discipline that caught the m020 bugs).
- **The wrapper killed a healthy, already-serving process (its bug —
  the serious one).** a1's boot took 188 s; the wrapper's LAUNCH loop
  declared its child "stuck" (no port holder, no fresh engine core)
  from t+1 and the 90 s stuck-kill fired at t+191 — **4 s after the
  port bound and the process was healthy and serving** (the job got a
  200 in that window; the EngineCore log shows the abort at the kill
  moment). SLOG tick arithmetic decoded it: the kill line's "173" was
  the tick counter (191 s / 173 ticks), `gone()` had in fact been false
  for ticks 91–172 (a fresh `VLLM::EngineCore` was visible) and flipped
  true only in a 2–4 s microsecond state right after bind — a window
  that does **not** reproduce when the identical checks run from the
  daemon's own context (a systemd-user-service probe in the daemon's
  exact netns/pidns sees its child's socket and core fine; the daemon
  has no namespace directives). Root cause: a transient detection gap,
  plus a threshold shorter than vLLM 0.27's own boot time.
- **The hardening (both production wrappers, `bash -n` clean):**
  (a) `gone()` is now health-aware — a 200 on `/health` outranks every
  absence heuristic; (b) the stuck-kill threshold is
  `STUCK_KILL_SECS=1200` (the daemon's own `healthCheckTimeout` — the
  daemon gives a boot 20 minutes; the wrapper must never give up before
  it) and fires only while `gone()` (health-aware) still holds; (c)
  from 60 s, any sustained `gone` window logs **forensics every 5 s**
  (health code, port pid, the raw `ss -ltnp` line, the engine-core
  pid) into the SLOG, so the next a1-class event is self-documenting.
  A healthy or serving process is now structurally unkillable by the
  wrapper.
- **Restore bug (the old `m021-restore`):** its service-health probe hit
  `/api/v1/status` unauthenticated → 401 (the endpoint requires the
  bearer token even for liveness) → read as "down" → needless
  `docker restart kokon` (~1 min of service outage). Fixed: token first,
  auth-aware probe, tokenless root endpoint as fallback. a2's restore
  ran the fixed path: no restart.

**m021 attempt 2 (17:47–17:50, FAIL at 180 s) — a generation mismatch,
not a product problem.** a1's restore correctly rolled the stable-dir
files back to the pre-021 generation; a2's *new* vision wrapper then
launched the *old* `serve-vision.sh`/`start_qwen.sh`, which have no
`SERVED_NAME`/`VISION` gate — so the "vision" request booted the **main
text variant** (the boot log proves it: `served_model_name=qwen3.8-27b`,
`--language-model-only`). The 404 `The model 'qwen3.8-27b-vision' does
not exist.` was the correct behavior of a main-variant process; a
second subtlety cost the single-shot request: vLLM 0.27 binds its port
only at the end of a 101–130 s init and the served model can 404 for a
few seconds after "Application startup complete". Fixes: (a)
`refs/post-021/` — a snapshot of the **post**-021 generation (the
stable-dir files, reconstructed byte-exact from this session's transcript
edit records and verified by dry-running the assembled argv against
a1's actual boot args), re-applied by the job at **P0b of every
attempt**; the restore keeps rolling back to `refs/thin-021/` and now
`rm -f`s the new `vllm-serve-vision.sh`; (b) the job's one-token
requests are now a bounded retry loop (first attempt blocks up to
1500 s through the gateway's health-gated proxy; then 25 × 60 s);
(c) the 401 noise on `/v1/models` in the boot log is the daemon's
keyless discovery probe — cosmetic, not a key failure (the
completion request got past auth).

**Rehearsal round 2 (the hardened kit, isolated).** All eight scenarios
re-ran on a fresh kit with rehearsal-only served names and a throwaway
port — a first same-box attempt was contaminated for exactly the reason
the name anchoring exists: the **production** main vLLM on this box
matches the main wrapper's pattern (it is, by definition, "ours"), so a
rehearsal must never reuse a name that runs in production here.
Results, all as designed: R1 LAUNCH-from-down → `saw_mine=1` → SIGTERM →
exit 0; R3 ADOPT-at-entry → release on death; R2 foreign holds at entry
→ dies t+10 → LAUNCH → adopt; R4 child aborts t+1 + foreign appears in
grace → **exit 0 without adopting, foreign survives** (the a2 shape);
R5 main-derivation LAUNCH clean; R6 foreign holds forever → exit 1
naming the holder, foreign untouched; R7 **the a1-killer shape scaled**
(slow bind at t+38 with no core visible before it, threshold 60) →
**no kill, no forensics, adopted** — the old 90 s threshold would have
killed this; R8 a child that never binds → three forensics lines
(health 000, empty port, raw `ss` header-only, core pid) then the kill
at t+76 → exit 1. The kill path and its forensic trail work; the
a1-shape cannot be re-killed.

**State before attempt 3:** on-disk deploy-dir wrappers = the hardened
pair; `refs/thin-021/` = pre-021 (rollback), `refs/post-021/` = the
tested generation (5 files; re-applied at P0b); stable dir = pre-021
(by design); `config.yaml == stock-016` (LIVE); the main variant is
serving this session; window-021 validates "10 model(s), 0 peer(s)".

## Attempts 3-6 and DONE (2026-11-04, 19:43-20:23)

- **a3 (died 30 s):** a stray `$old` token left in the job after the P0b
  restructure — a `set -u` trip, caught immediately, removed.
- **a4 (died 1 s):** the sequencing hole the a2 fix created: the restore
  of a failed attempt `rm -f`s `vllm-serve-vision.sh` (correct — the
  pre-021 generation has no such file), but the next attempt's exec-bit
  audit of the window config ran **before** P0b re-applied the file.
  Fix: the audit now runs **after** P0b (it audits the generation that
  will actually run).
- **a5 (died at P2b, 1336 s):** P2 passed for real — the vision variant
  booted through the router (102 s, `/v1/models` exactly
  `['qwen3.8-27b-vision']`, 200) and **P2b's image request returned
  200 with the vision tower genuinely processing the 64×64 PNG**
  (`multimodal_tokens: {image: 64}`; the JIT log shows the vision
  kernels compiling on first use). The die was the assertion: with
  `--reasoning-parser qwen`, vLLM 0.27 puts the model's thinking in a
  `reasoning` field and `content` stayed **null** — the 120
  `max_tokens` were entirely consumed by thinking (`finish_reason:
  length`). The response the model produced while thinking was already
  a correct description of the image. Fix: `max_tokens` 512, and the
  acceptance is a non-empty `content` **or** `reasoning` (seeing the
  image is proven by either; the digest prints whichever exists).
  Operational note from a5's restore: the canonical main revival after a
  vision boot took **20+ minutes** (cold inductor + cold weights after
  the 22 GB page-cache churn) — the route probe caught it healthy at
  ~21 min; a restore that follows a variant switch must budget for the
  cold-boot worst case, and the session's quiet window in a FAIL is
  correspondingly long (a5's was ~23 min; a6's DONE path is ~3 min).
- **a6 — DONE, 391 s (20:15:30 → 20:22:45).** Every phase passed with
  the journal as evidence:
  - **P2** (95 s warm boot): box-empty orphan shapes
    (`matrix: model=qwen3.8-27b-vision starting (no models running)` +
    `evicting []`); `/v1/models` exactly `['qwen3.8-27b-vision']`
    (max_model_len 131072); 1-token 200; VRAM 24118/24128.
  - **P2b**: 200 + the model's own description — "a single flat square
    divided into two solid color regions … a solid blue square … in the
    top-left corner … [on a red field]" — exactly the generated image;
    `multimodal_tokens: {image: 64}`; VRAM unchanged after (24130).
  - **P3** (56 s): `starting swap for model qwen3.8-27b, evicting
    [qwen3.8-27b-vision]` + the orphan set line
    (`set= dsl="" evict=[qwen3.8-27b-vision] cost=0`) + vision
    `exited cleanly` — **orphan-vs-orphan, direction 1**; session route
    back at 20:18:17 (~3 min quiet total).
  - **P4**: `matrix: model=kokon-klein set=klein dsl="k & p"
    evict=[qwen3.8-27b] target=[kokon-klein kokon-pup] cost=1`; job done
    in 30 s (seed 3, enhanced) — the sets are untouched by the new
    model.
  - **P5** (60 s): `starting swap for model qwen3.8-27b, evicting
    [kokon-klein kokon-pup]` + both exited cleanly — direction 2, box
    recovery; image ports free.
  - **P6**: cutover to **stock-017** (the production config now carries
    the 10th model), active = kokon-klein, canonical main revived —
    including the recurring daemon/canonical launch race, which the
    wrapper absorbed by ADOPT ("child pid … exited rc=1 at t+1s …
    our vllm alive after child exit — ADOPT: watch until gone"); final
    main healthy (23236 MiB), gateway 200, ports free.
  - The vision wrapper's self-log finally **caught the a1 blind window
    on film**: at t+95-104 of its only LAUNCH, the forensics line shows
    `health=000 port_pid='<empty>' raw_ss='<header only>'
    core_pid='<empty>'` while the child was alive — the 2-4 s
    post-bind detection gap is a real (and brief) state of vLLM 0.27,
    and the hardened wrapper rode it out (no kill; `STUCK_KILL_SECS`
    1200 gives it 20 min).
- **Closed.** `config.yaml.stock-017` is live and serving this session
  (main variant). Rollback ladder: `stock-016` (pre-task) is one atomic
  cp away; the on-disk vLLM generation stays the post-021 set in
  production (the stock-016 config simply doesn't reference
  `vllm-serve-vision.sh`); `refs/thin-021/` + `refs/post-021/` keep
  both generations restorable. Two minor job-file blemishes noted, not
  product defects: two P4 pre-warm paths were wrong guesses (fixed in
  the job file; the job passed because the pair was already
  page-cached), and the old same-box rehearsal kit (`/tmp/vllm-wrap-rh`)
  is superseded by the isolated kit (`/tmp/vllm-wrap-rh2`).

## Acceptance criteria

- `config.yaml.stock-017` live after the window: vision entry in **no
  set**, six kokon models unchanged from stock-016, both vLLM variants
  on :18020; validates on the pinned binary.
- The window (m021) proves, with journal lines as evidence: (a) a vision
  request boots the vision variant and answers, with an image actually
  seen (P2/P2b); (b) orphan-vs-orphan **both directions**: a main
  request evicts the resident vision (P3) and a main request evicts the
  resident klein pair back to main (P5); (c) a kokon job still evicts
  main as before (P4, the sets are untouched); (d) after the window,
  cutover to stock-017 with the box serving this session (main variant
  resident).
- **KV headroom measured** (P2/P2b VRAM captures): if the headroom is
  not ~1.5 GB, `serve-vision.sh`'s `--kv-cache-memory-bytes` is adjusted
  with the measurement recorded as its source.
- `MATRIX.md` carries the "Adding a model / model combo to the gateway"
  guide (five pieces; vision as the worked example of two models, one
  port); `docs/fresh-box.md` lists the 10th model with its provenance
  line.
- No out-of-band process kills anywhere (the task-020 invariant,
  extended to the second variant — the wrappers never kill a foreign
  holder, even when it blocks them); both wrappers' self-logs land in
  `/tmp/`; the window ends on the production config, marker last.

## Live follow-up (2026-11-04 evening): the owner's photo — 2 MP cap + tower offload

First real use of the variant (the owner's 1500×2000 phone photo in a new pi
session, 20:40) failed: the vision process died ~6 s after boot and the
client timed out at ~2 min. Three data points, then the fix, then proof:

1. **20:42 OOM (uncapped).** The processor default budget is 16.7 MP; the
   3 MP photo = ~2914 visual tokens = ~3.2 GiB of encoder working memory
   vs 2.85 GiB free above the settled 24128 MiB → `torch.OutOfMemoryError`
   in the ViT inductor graph → process dead. (An intermediate fix attempt
   cut the KV to 2048M: below the 4.59 GiB engine minimum for 131k, so it
   died at boot every time, and a probe-retry loop turned it into ~5 dead
   boots in 8 min — a protocol violation, not an engine defect; the KV is
   back at the owner's 7372M.)
2. **22:07 OOM (2 MP cap alone).** The cap cut the photo to 2028 tokens
   and the settled VRAM to 23314 (vLLM profiles the encoder at its largest
   accepted image — capping that peak pays back VRAM), but the photo still
   OOM'd — this time in the **GDN chunked-attention kernel**
   (`third_party/flash_linear_attention/ops/chunk_o.py: o =
   torch.empty_like(v)`, 20 MiB tried against 15.81 MiB free): on this
   hybrid model an image prefill's transient is dominated by the LM's GDN
   layers, and with the 0.86 GiB tower resident there was ~1.2 GiB of
   headroom, not enough. The same wall the sibling HyperQwen tree's
   `VISION_OFFLOAD` knob was built for.
3. **Fix (both owner-sanctioned parts).** (a) the 2 MP pixel cap
   (`--mm-processor-kwargs {"max_pixels":2097152}` in `serve-vision.sh`
   `EXTRA_ARGS`) — owner ruling: on this path vision is needed at most for
   code-dev screenshots, 1920×1080 = 2.07 MP, which the 2 MP budget passes
   uncapped; and (b) the **vision tower in pinned host RAM** — owner:
   "worst case scenario just keep it on cpu", and the knob lives in the
   sibling trees (`stable/hyperqwen-031` / `latest/HyperQwen`:
   `VLLM_VISION_CPU_OFFLOAD_GB`, `patches/vision-tower-cpu-offload.patch`,
   exported from cpuchip/vllm 13299b528 for 0.28.0). Ported 1:1 to the 0.27.1
   venv as `stable/qwen38-27b-rtx3090/patches/vision-tower-cpu-offload-027.patch`
   (two files: `envs.py` declaration+loader, `qwen3_vl.py` tower wrap — the
   0.27.1 `UVAOffloader` and `Qwen3_VisionTransformer` are API- and
   shape-identical to the patch's targets). Static verification before any
   GPU time: clean `patch` apply, env reads 0.0/1.0, both files
   `py_compile` + import clean, and the 0.27.1-specific bulk-copy forward
   wrapper measured **bit-exact** on this card (0.00e+00 delta, both call
   directions, real CUDA). Pre-patch venv files backed up at
   `/tmp/venv-backup-photofix/`. Wired with one `export
   VLLM_VISION_CPU_OFFLOAD_GB=1` in `serve-vision.sh` (the child inherits
   it); `refs/post-021/` re-synced. KV 7372M, 131k max-len, 1024 batched,
   MAX_SEQS 2, GPU_UTIL 0.97 all untouched; the main variant is unchanged.
4. **Proof window (22:17, single-shot, guaranteed-restore):** DONE in
   685 s. Vision booted in 210 s with the offload engaged in the production
   path (in-boot line `Total CPU offloaded parameters: 0.85`; settled
   22372 MiB = −942 vs the tower-resident 23314); **the same 20:42 photo →
   200 in 11 s** (`multimodal_tokens: 2028`, 743-char answer + 1470-char
   thinking, peak 23410/24576 — fits, no OOM, no restart); main back in 61
   s. During both windows the owner's photo session — a second client on
   the same shared port — made its own vision/main requests and the router
   handled every direction correctly (the journal shows four clean
   evict/restore decisions); that concurrent-use pattern is what parked
   task-023 would address if it ever bites.

Net production change: `serve-vision.sh` (2 MP cap + offload env), the
venv (2-file patch), `refs/post-021/` (re-synced). The a6 KV-headroom
acceptance is superseded by the measurement this follow-up made real:
with the tower offloaded the settled vision VRAM is 22372 MiB, leaving
~2.2 GiB for image prefills — the 131k/7372M KV config stands as the
owner tuned it.
