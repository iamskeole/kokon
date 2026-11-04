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
  decision above; every artifact in this task is built same-port.

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
