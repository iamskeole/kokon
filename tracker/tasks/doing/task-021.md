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
  decision above; every artifact in this task is built same-port.- **KV headroom (a1 measured, 2026-11-04): keep `7372M` as-is.** The
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
