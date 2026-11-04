# Task-020 — stock llama-swap as the complete GPU arbiter: retire the wrappers' out-of-band kills and the policy gates

**Created:** 2026-11-04 — the owner, pushing back on the claim that the pinned daemon
"has no concept of in-flight requests", asked to explore whether stock llama-swap has
queue-type knobs that protect active requests. Exploration (below) proved him right;
he said "proceed".

## Goal

The v269 gateway is a **request-aware FIFO scheduler** at its core: it already
implements the owner's protection rule — *don't boot someone up mid-generation,
only when they're not active* — for every eviction **it** decides. The remaining
gap is that this box's wrapper scripts make **out-of-band kills** (`evict-gpu.sh`,
`evict-kokon.sh`) and honor a **policy file** (`kokon-policy.txt`) that bypass the
scheduler entirely, so on the klein axis and the creative axis an in-flight
request can still be cut. Make the router the sole eviction authority:
stock-clean sets + thin wrappers, so the scheduler's deferral covers every axis
and the non-stock residue shrinks to what v269 structurally cannot do (start two
processes from one request → the klein joint supervisor).

## Exploration record (2026-11-04, v269 `9fbc47c`, pinned; source is the record)

**The daemon's core is a request-aware FIFO scheduler** (`internal/router/scheduler/`;
introduced v225, PR #823; the 429 concurrency limits moved into it in v227, PR #849;
in-flight request details in PR #895/#923). It has been running on this box since the
v269 pin (task-001) with zero configuration — the live config has no
`routing.scheduler` block, so defaults apply: `use: fifo` (the only implementation),
no priorities, per-model `concurrencyLimit` default 10, no global limit.
Per-request decision tree (`fifo.go OnRequest`):

1. unknown model → reject
2. a swap for the same model is in flight → join it
3. target ready, evict set empty, no collision with active swaps → serve (fast path)
4. collides with an in-flight swap (mutual-evict / overlapping evict sets) → queue
5. **`conflictsWithInFlight`: any model in the evict set has in-flight requests > 0 →
   `queuing request for model %s (would evict in-flight process)` → enqueue.**
   `OnServeDone` (a serve completes) and `OnSwapDone` both `drainQueue()` — the tree
   is re-run on every completion, so a parked request proceeds as soon as the busy
   model drains.
6. otherwise start the (possibly parallel) swap.

Queue order: `routing.scheduler.settings.fifo.priority` (per-model int, default 0;
higher serviced first, equal keeps arrival order); positions are broadcast to clients
(`PositionCh`). **In-flight accounting:** `inFlight[model]` increments on a granted
serve and decrements when the handler returns — for a vLLM stream, when the stream
completes. `OnCancel` (client disconnect / ctx cancel) prunes the request from queue
and swap waiters; a swap whose sole waiter was cancelled self-completes.

**TTL cannot interrupt a live job:** the per-process TTL ticker
(`process_command.go`) does `if p.inflight.Load() != 0 { continue }` before checking
the clock — a model serving a request is never TTL-unloaded no matter how long the
job runs; `lastUse` refreshes on serve activity. The only paths that *can* interrupt
an in-flight request: **explicit Unload** (API unload, config reload, manual —
`base.go Unload`: "In-flight requests being served by an unloaded process are not
waited for — Stop kills the upstream, those callers see whatever error the reverse
proxy surfaces and may retry", after the `unloadTimeout` grace) and daemon
**Shutdown** (waits for in-flight up to its timeout, then cancels).

**The knobs (config schema):** `routing.scheduler.use` (enum `["fifo"]`, default);
`routing.scheduler.settings.fifo.priority`; per-model `concurrencyLimit` (default 10,
over → 429 with retry-after); `globalConcurrencyLimit` (0 = off — a single shared
semaphore across all models when set); per-model and global `unloadTimeout`
(default 10 s). The gateway's MCP tools can surface in-flight request details
(`InFlightRequestsEvent`).

**The boundary (honest):** deferral is **non-preemptive**. A queued request — even a
higher-priority one — cannot cut a busy model; it waits for the drain or for its own
client to give up. Nothing in stock makes a busy model yield early. On one 24 GB card
that is the best possible — exclusivity is physical, not a scheduling choice.

**The two holes in the live (stock-015) config** — the scheduler only protects models
that are *in the evictor's evict set*; our wrappers kill out-of-band regardless:

1. *Klein axis:* the klein set `k & p & v` contains vLLM → a chat request landing
   mid-klein-job has an **empty** evict set (v's chosen set is klein, which contains
   k and p) → the scheduler sees no conflict → `vllm-serve.sh`'s fully-down branch
   runs `evict-gpu.sh` → **the klein pair is killed mid-generation**. Reachable:
   a klein job runs only when vLLM is down, and a chat message arriving during the
   job is exactly this case.
2. *Creative axis:* the small set `(cf | cs) & v` contains vLLM → a creative request
   mid-chat has an empty evict set → `creative-{fast,slow}.sh`'s own `evict-gpu.sh`
   call **cuts the stream**.
3. *(The qwen axis is fine as it stands: the qwen sets lack `v`, so vLLM lands in
   their evict set and deferral works — it has simply never been exercised live;
   every A/B window ran its traffic sequentially.)*

**Correction to the record:** my earlier statement in this session ("the daemon is a
state machine; a preempted in-flight request dies the moment the eviction fires")
described the pre-v225 daemon from the task-001 exploration. The owner's recollection
("we explored this before … some queue type knobs … protect active requests") was
correct. The v225+ scheduler replaced that design; the compaction between spans hid
it from me.

## Design (stock-clean: the router is the sole eviction authority)

1. **`config.yaml.stock-016`** = stock-015 with two set changes:
   `klein: "k & p"` and `small: "cf | cs"`. vLLM leaves **every** set → it becomes a
   **pure orphan** (the router synthesizes the reserved `auto:orphans` set for it) →
   a vLLM request evicts **everything** running; every image/creative request evicts
   vLLM plus the other image families (the card cannot physically hold two 20+ GB
   DiTs, and a 26/31B creative next to anything else either). The qwen sets are
   unchanged (`qt & pq`, `qf & pqf` — they already lacked `v`).
   Consequences (all recorded as intended, all of them a *finishing* of existing
   stock guarantees, not new policy):
   - a **klein** request with vLLM resident: the router evicts vLLM (`evict=[qwen3.8-27b]`)
     and the job runs — the `protect-vllm` refusal dies with the gate. The card is
     the card; the previous "klein fails cleanly while you're chatting" state is
     replaced by "your image job takes the card, chat resumes when a vLLM request
     reclaims it (warm boot ~60–90 s)".
   - a **creative** request with vLLM resident: clean router eviction instead of a
     mid-stream wrapper cut (and instead of the old OOM-next-to-vLLM class).
   - a **vLLM** request reclaims a resident image/creative pair: unchanged in
     effect (the nudge-404 fix), now via the synthesized orphan set, and now
     **deferred while any of them is busy** (new, the headline guarantee).
2. **`gpu-empty.sh`** (new; the only automatic GPU-clearing script): idempotently
   stops the known image-tenant ports (9100 9101 39160 39170 39171 39175 39176
   39177), then waits (budgeted, default 900 s) until nvidia-smi reports zero compute
   apps and < 1500 MiB. **It never touches :18020** — vLLM is stopped only by the
   router's `cmdStop` or by a human. In stock-016 the router has already evicted
   every *in-state* tenant before the cmd runs, so the script is a **last-resort
   zombie net** (human-booted or stale processes), not a policy tool. Its wait
   doubles as the 22:46 "exited prematurely" grace: a tenant whose stop was just
   triggered is still freeing VRAM, and the supervisor waits for an actually-empty
   card before loading instead of polling endpoints.
3. **`vllm-serve.sh`:** keep the adoption branch and the booting branch (the
   2026-11-04 idempotency fix — both are still required under the daemon's
   fresh-cmd-per-EnsureReady-attempt behavior). Fully-down branch:
   `evict-gpu.sh 900` → `gpu-empty.sh 900` (last-resort only).
4. **`creative-{fast,slow}.sh`:** drop the `evict-gpu.sh 600` line →
   `gpu-empty.sh 600` (last-resort). A 26/31B load cannot share the card with
   anything, same physical justification as vLLM; without the net, an out-of-state
   zombie would OOM the load and leave the creative model dead until a human
   notices — the visible-failure property is kept, minus the mid-stream cuts.
5. **`sd-cpp-serve-qwen-stock.sh`:** the policy gate is deleted (no `kokon-policy.txt`
   read, no `protect-vllm` refusal, no take-gpu tenant stops); the fresh-boot branch's
   `evict-kokon.sh 300` → `gpu-empty.sh 300` (last-resort). The healthy-:39160
   adoption stays.
6. **`sd-cpp-serve-klein-stock.sh`:** the policy gate is deleted (same); the fresh-boot
   branch's `evict-kokon.sh 300` → `gpu-empty.sh 300`. The **one-lifetime joint
   supervisor stays** — v269 cannot start two processes from one request; that is the
   structural (non-wizardry) residue, decided in task-009. Adoption stays.
7. **`kokon-policy.txt` is retired** (last value: `protect-vllm`, recorded here; the
   file is archived, not deleted-silently). The sets are now the policy.
   `evict-kokon.sh` is deleted (replaced by `gpu-empty.sh`); **`evict-gpu.sh` is kept
   as a manual-only tool** — a human deliberately clearing the whole card including
   vLLM (it still stops :18020; nothing automatic calls it after this task).

## Acceptance criteria

- [ ] `config.yaml.stock-016` passes `llama-swap -config … -validate`; vLLM is a
      leaf of no user-defined set; the first vLLM request's journal decision line
      shows the synthesized orphan set (the m020 window captures the actual line).
- [ ] No serve/stop script calls `evict-gpu.sh`/`evict-kokon.sh`, reads
      `kokon-policy.txt`, or stops the kokon docker container. `gpu-empty.sh` never
      touches :18020. Pre-launch exec-bit audit of every `cmd`/`cmdStop` in the new
      config (the standing rule).
- [ ] **Verification window** (marker-last; the window config adds `logLevel: debug`
      to capture the scheduler's otherwise-invisible lines; restore target =
      stock-015 **plus the pre-020 wrapper set**, see Decisions):
      - P2: a **klein** job with vLLM resident → journal `matrix: … evict=[qwen3.8-27b]`
        (the gate is gone: the job runs instead of refusing), klein pair loads, job
        completes.
      - P3: a **turbo** job (production path; the 9B-Q8 door cmdline proof).
      - P4: **deferral, forward** (the headline): a qwen **quality** job in flight
        (~2–4 min) → a 1-token vLLM request fired mid-job → expected journal
        `queuing request for model qwen3.8-27b (would evict in-flight process)`;
        the vLLM request completes **after** the image job finishes + vLLM load;
        the image job completes successfully, unharmed.
      - P5: **deferral, reverse**: a long vLLM completion in stream (~1200 max
        tokens) → a 1-token request to the turbo PUP door (`kokon-pup-qwen`, via the
        gateway) fired mid-stream → expected: the door's swap does **not start**
        before the stream ends (journal `starting swap` after stream end); the
        stream completes intact; then the door loads and answers.
      - P6: a vLLM request reclaims a **resident, idle** qwen pair via the orphan
        set (journal shows set = the synthesized orphans name) — the nudge-404 path
        under stock-016.
      - P7: teardown (stop scripts, drain), canonical vLLM revival + 1-token proof,
        final state (config identity, ports, active service model), digest, **DONE
        last**.
- [ ] Docs: `MATRIX.md` (a **scheduler** section — the other half of `routing:` —
      deferral semantics, the knobs, the non-preemptive boundary; the stock-016 sets
      replace the stock-015 block; the wrapper contract: router = sole eviction
      authority, wrappers are idempotent launchers with a last-resort zombie net);
      `docs/fresh-box.md` (the routing block, the script inventory, the policy file
      is gone); `README.local.md` (script inventory); `QWEN-ROLLBACK.md` /
      `KLEIN-ROLLBACK.md` (the rollback ladder note: a pre-020 rollback restores the
      gate wrappers from `refs/pre-020/`; the config-only ladder 013↔014↔015 stays
      valid within the gate-wrapper era).
- [ ] task-019 note: the shared-sidecar design lands on top of the stock-016
      topology (its set is `k & p & qt? …` — no: it adds the PUP var to three sets;
      its precondition 3, the supervisor lifetime rework, is relative to the
      post-020 wrapper, not the pre-020 one).
- [ ] STATUS updated; the task moves to `done/` only after the window passes **and**
      the stock-016 cutover is live with vLLM revived.

## Blockers

- A GPU window (~30–40 min) with the standard protocol: vLLM out of the box during
  the image phases and revived before the marker; this session's chat route (vLLM)
  is down across the window and any message sent mid-window **queues and lands
  late** — that is the feature being demonstrated, and the owner is awake.
- Nothing else: 015 is done, 017 is parked on the owner, 019 is parked on its own
  preconditions.

## Decisions

- **Stock-clean sets (owner "proceed", 2026-11-04):** vLLM out of every set → pure
  orphan. The product tradeoff I had posed ("images always take the card, chat dies
  mid-stream") dissolves under the FIFO scheduler: the in-flight side always
  finishes first, the requester waits; what the card holds is physical, not a
  scheduling choice. The `protect-vllm` stance (klein/creative refuse while vLLM is
  resident) is retired — it existed to prevent OOMs that the router now evicts
  cleanly, and it cost the user working image jobs.
- **Wrappers stop evicting (same decision):** every automatic evict call goes; the
  last-resort `gpu-empty.sh` zombie net stays in all four heavy loaders (vllm-serve,
  creative-fast, creative-slow, qwen/klein supervisors) because an out-of-state
  zombie OOMs a big load invisibly, and the visible-failure property (abort with a
  message if the card never empties) is kept. The non-stock residue after this task:
  the klein joint supervisor (structural) and the vllm-serve adoption/booting
  branches (idempotency under the daemon's cmd-spawn behavior).
- **Policy file retired:** last value `protect-vllm` (this file is the record).
  From stock-016 on, the sets express the policy; the one-word file was a workaround
  for a router that could not express it.
- **Rollback ladder changes at the 016 boundary:** the config-only one-copy rollback
  (013↔014↔015) stays valid *within the gate-wrapper era*; a rollback across the
  016 boundary restores the config **and** the pre-020 wrapper set (snapshot at
  `llama-swap-deploy/refs/pre-020/` taken before the surgery). Recorded in both
  rollback docs.
- **Residual gaps (accepted, recorded):**
  (a) a **config reload / API unload / daemon stop** can still interrupt a live job
      after the `unloadTimeout` grace — standing rule: windows never swap config
      mid-job (now with the source citation above);
  (b) **in-flight visibility is per-gateway-request.** Verified 2026-11-04: the live
      service config (`/tmp/qwabl5/service-config.json`, schema 13) routes its VLM
      through `externalEngine.vlm.entrypoint = http://127.0.0.1:8081/upstream/…` —
      the `/upstream/` path, so the door's in-flight **is** visible to the scheduler
      and a vLLM request mid-enhance defers. Invariant for future config work: the
      per-model `vlm.url` override must never be pointed at a door directly
      (:39175/:39176/:39177) — a direct call would be invisible and a vLLM request
      mid-enhance would kill the door.
  (c) the **non-preemptive boundary**: a long busy (a 10-minute creative job) blocks
      every other model for that duration plus load time — physical on one card;
      the only stock knob is queue priority (ordering among *unblocked* requests),
      which does not preempt. If the owner ever wants "chat can always cut a long
      creative job", that is a feature to ask upstream for (priority preemption with
      a drain deadline), not something to bolt on here.

## implemented — 2026-11-04 (autonomy span, after the owner's "proceed")

All of unit 1's files landed (host side; the llama-swap tree stayed clean):

- **`config.yaml.stock-016`** — stock-015 with `klein: "k & p"`,
  `small: "cf | cs"`, comments rewritten to the router+scheduler model
  (vLLM the only orphan; the wrapper contract; deltas vs 015).
  `llama-swap -validate`: valid, 9 models; all 9 model blocks identical
  to stock-015 except descriptions (byte-level checked).
  **`config.yaml.window-020`** = stock-016 + `logLevel: debug`.
- **`gpu-empty.sh`** (new) — the last-resort clear: 9100/9101/39160/
  39170/39171/39175/39176/39177 SIGTERM→10 s→SIGKILL, wait for
  nvidia-smi 0 apps + <1500 MiB, **never :18020**, no policy, no docker
  touches. Its wait doubles as the 22:46-style in-flight-unload grace.
- **Wrappers made thin** (all syntax-checked): `vllm-serve.sh` (the
  fully-down branch now `gpu-empty.sh 900`; adoption + booting branch
  keep); `creative-fast/slow.sh` (`gpu-empty 600`); `sd-cpp-serve-qwen-
  stock.sh` + `sd-cpp-serve-klein-stock.sh` (the gate block and
  `heavy_up` deleted; fresh-boot branch `gpu-empty 300`); the three PUP
  doors (`qwen-pup-serve-9.sh`, `qwen-pup-serve.sh`, `pup-serve.sh`) got
  the **vLLM fit-check** — 30 s wait for a mid-stop, then `exit 1` with
  the unload hint; never stops vLLM.
- **`evict-gpu.sh` re-headered MANUAL** (whole card incl. vLLM; no
  automatic caller); **`evict-kokon.sh` and `kokon-policy.txt`
  deleted**; the complete gate-era generation (11 files) snapshotted at
  **`refs/pre-020/`** for the cross-generation rollback.
- **Docs**: `MATRIX.md` rewritten (the three rules; a new scheduler
  section — in-flight deferral, TTL skip, non-preemptive, the boundary,
  the knobs; the wrapper contract; the new-set recipe; live-change; the
  vllm-serve trap; the rollback incl. the pre-020 rule); `README.local.
  md` (the swap section and the routing paragraph); `docs/fresh-box.md`
  (routing block → stock-016, the supervisor paragraph, the ops
  bullets); `QWEN-ROLLBACK.md` + `KLEIN-ROLLBACK.md` got the
  016-boundary note (config-only rollback is valid within a wrapper
  generation; a v-containing klein set needs the `refs/pre-020/`
  scripts too).

**Next (unit 3):** the `jobs/m020-*` family, then launch: P1 swap to
window-020 → P2 canonical vLLM revival → P3 klein job with vLLM
resident (journal `evict=[qwen3.8-27b]`, job completes — the refusal is
dead) → P4 turbo job (9B door cmdline proof) → **P4-def:** a vLLM
1-token request mid-quality-job (journal "would evict in-flight
process"; the job unharmed; the response after the job + boot) →
**P5-def:** a ~1200-token vLLM stream in flight, a 1-token turbo-door
request through the gateway parked until the stream ends (the reverse
direction) → P6 vLLM reclaim of an idle qwen pair (journal shows the
orphan set) → P7 teardown + active-klein + DONE last. Restore target:
stock-015 **and** the `refs/pre-020/` wrapper generation.

## window attempt 1 — 2026-11-04 09:44 (FAIL at P3, 616 s; box auto-restored)

The window ran P0–P2 clean, then died in P3 (the klein job with vLLM
resident). **The router worked exactly as designed; my surgery broke the
pair.** Evidence: `/tmp/m020-attempt1/` (journal-p3-recovered.txt is the
key file).

Failure chain (all timestamps UTC, from the gateway journal):
1. `09:47:36` — the klein job's first gateway request (the service's
   3 s readiness probe) triggers the swap: `matrix: model=kokon-klein
   set=klein dsl="k & p" evict=[qwen3.8-27b] target=[kokon-klein kokon-pup]
   cost=1` — the headline line the window wanted. vLLM evicted cleanly in
   3 s.
2. `09:47:39` — the (new thin) joint supervisor runs, starts the engine
   **and** its own sidecar.
3. `09:47:41` — the service's 3 s probe times out and is cancelled
   (`matrix: cancelled request … pruned from scheduler`). By service
   design (`crates/qls-core/src/process.rs::ensure_external`) this is
   harmless: a 900 s follow-up request joins the same swap and carries
   the load.
4. `09:47:42` — the router runs the `cmd` for the **second alias**
   (kokon-pup), which at that moment was still the **standalone
   `pup-serve.sh`** → a **second** PUP load.
5. The engine's DiT loaded in 3.02 s (warm cache) and sd-server binds its
   port only after the full load, so the router's check endpoint
   (`/sdcpp/v1/capabilities`) answered 200 at t+5 s — a true ready, but
   the 120 s TTL clock started there.
6. ~30 GB of concurrent loads on a 24 GB card (engine 8.4 + supervisor
   sidecar 10 + standalone sidecar 10): the supervisor's sidecar wedged
   in `fitting params to device memory` and never came up; the standalone
   sidecar never became healthy within the router's 120 s start budget.
7. `09:49:44` — the engine's 120 s TTL fires (no in-flight: the probe was
   cancelled, the 900 s request had completed at ready) → router Stop →
   the joint supervisor's one-lifetime dies with it → the standalone
   sidecar is killed at its failed-start budget → `matrix: starting
   kokon-pup failed: upstream command exited prematurely` → 500 on the
   service's enhance → job hard-fail (code `upsampler`) → die.

The die behaved per protocol: restore → digest → FAIL marker last; the
blocking wait landed on a live route (the nudge-404 class did not
recur). The restore, as written, returned the box to the **gate-era**
generation (its wrapper list was the pre-020 snapshot) — correct for a
rollback to stock-015, but the thin surgery was thereby wiped and had to
be re-applied.

## the fix (implemented 2026-11-04, pre-attempt-2)

- **One sidecar provider.** `kokon-pup`'s `cmd` is now the **same joint
  supervisor** as `kokon-klein` (and its `cmdStop` is the klein stop
  script), made **re-entrant**: a second instance (the gateway runs the
  cmd once per alias) detects the live first supervisor via
  `supervisor.pid` — with a 2 s re-check against same-second double
  spawns — launches nothing, waits up to 600 s for the pair, then watches
  the sidecar until it goes away. One PUP load per pair, ever; a Stop on
  either alias takes the pair down (the one shared lifetime, made
  router-visible).
- **TTL 600 s on all six kokon models** (stock-016/window-020): the
  klein engine becomes ready ~5 s into a load, long before the
  60–120 s sidecar; a 120 s TTL anchored on that early ready reaps the
  pair mid-job (attempt 1, step 7). 600 s outlives any realistic
  enhance→img_gen gap; on-demand eviction (another set's request) is
  unaffected — it takes the card at once regardless of TTL.
- **The thin wrapper surgery re-applied** to all live scripts (the
  restore had put the gate era back): both engine supervisors (no gate,
  `gpu-empty.sh 300` in the fresh-boot branch), the three PUP doors
  (fit-check → `exit 1`, never stop vLLM), `pup-serve.sh` (standalone
  loader, no longer a gateway cmd; `gpu-empty.sh 900`), `vllm-serve.sh`
  (fully-down branch → `gpu-empty.sh 900`), the creatives (`gpu-empty.sh
  600`), `evict-gpu.sh` re-headered **manual-only**; `evict-kokon.sh`
  and `kokon-policy.txt` deleted again; the thin generation snapshotted
  at **`refs/thin-020/`** (10 files) so the next restore is
  self-sufficient — `m020-restore.sh` now restores **thin-020** (and
  removes gate-era relics) and its double-logging bug is fixed;
  `m020-test.sh`'s `die()` captures the journal before anything else.
- `config.yaml.stock-016` / `config.yaml.window-020` rebuilt with the
  deltas (single-provider cmd/cmdStop, six × `ttl: 600`, descriptions);
  both `-validate` clean (9 models). Verified delta vs stock-015:
  description + ttl on the six kokon models, cmd/cmdStop on kokon-pup,
  the two set lines.

**Attempt 2** launches with the same window plan; restore target =
stock-015 + the thin generation.


## attempt 2 (window 10:47-11:12 UTC; FAIL at P4d after 1441 s; the wrapper's blind spot)

**P1-P3 all passed, including the task's headline proof for the first
time:** with vLLM (22-23 GB) resident, a klein job completed in 25 s -
journal: `matrix: model=kokon-klein set=klein dsl="k & p" evict=[qwen3.8-27b]
target=[kokon-klein kokon-pup] cost=1` - and vLLM later reclaimed its slot
via the `auto:orphans` set. The re-entrant second supervisor instance
worked as designed (the `pup` cmd spawned, detected the live pair,
joined). The turbo job (P4) completed in 31 s (9B-Q8 door pid proven from
the process list).

**P4d failed for three compounding reasons - only one of them a bug:**

1. **The 1-token vLLM request landed in the enhance->img_gen gap** (job
   10:56:22; enhance 10:56:36-40; the request reached the router at
   10:56:57, after enhance had drained). At that moment neither the door
   nor the engine held a *sustained* in-flight request - qwen generation
   is submit-and-poll: the journal shows ~1 req/s to the engine, each
   <50 ms, gaps of 1 s - so the FIFO deferral correctly did NOT apply
   and the router started the vLLM swap, evicting
   `[kokon-full kokon-pup-qwen-f]`. This is the **A-boundary** flagged in
   the design: deferral covers in-flight request windows, not
   submit-and-poll generation. On this 24 GB card the two cannot co-reside
   anyway (27B + any engine > 24 GB), so the interruption is the *correct*
   policy (chat wins), not a scheduler error - but the test had assumed
   deferral. **Design note for MATRIX.md**: on the image side, "busy" =
   sustained HTTP windows (VLM enhance, vLLM streams) only, not polling.
2. **vLLM was healthy for exactly one second** (10:57:53 -> :54). The
   job's 60 s retry (drained 10:57:54) immediately evicted vLLM again
   (journal: `starting swap ... evicting [qwen3.8-27b]`). The vLLM
   process: EngineCore force-killed in <1 s (the GPU freed instantly -
   the "no compute apps" the owner saw for the next 15 minutes), but the
   API server hung at "shutting down FastAPI HTTP server" until
   do_stop's +30 s SIGKILL backstop (a vLLM 0.11.1 shutdown-path quirk;
   future upstream item, no action on our side).
3. **The real bug - my wrapper's 900 s health blind loop.** The 1-second
   200 window fell between the wrapper's 5 s /health ticks, so it never
   saw a 200 and kept polling a dead port until the post-launch loop's
   900 s expiry (11:12:03), then `exit 1`. The router's `sendStopSignal`
   runs `cmdStop` but does NOT signal the tracked wrapper process - the
   blocked swap waited for the wrapper to exit on its own. The job had
   meanwhile burned its 900 s service `EXTERNAL_LOAD_TIMEOUT` (failed
   11:12:10, 7 s after the wrapper finally released the swap). Under the
   old design this same 1-second window would have cost every subsequent
   swap on the card 15 minutes.

**The fix (implemented 2026-11-04, rehearsed, ready for attempt 3):** the
gateway launch path no longer detaches into a systemd scope. `serve.sh`
(and `serve-vision.sh`, task-021) gains `start --no-scope`: identical
env/args/`start_qwen.sh`, but it **execs** into the venv vllm, so the
subshell the wrapper spawns *is* the vllm process - the wrapper becomes
vllm's **direct parent** and watches the child process (`kill -0`)
instead of health: death is visible on any timescale, group signals reach
vllm directly. The scope path stays for human launches (terminal-death
history). Rehearsal against a fake vllm on :18099 (four scenarios): the
a2 replay (1 s 200 window + SIGTERM) -> wrapper exits 0 **1 s** after
death (was 900 s); early crash (port never bound) -> exit 1 promptly;
foreign boot appearing mid-launch -> adopt + watch, exit 0 on its death;
stuck launch (no port for 90 s) -> child tree killed, exit 1. Rehearsal
also caught the 1-s tick on the port latch (a 5 s poll missed a 2 s port
lifetime - the same failure class one level down). `refs/thin-020/`
re-synced with the parent wrapper; the daemon's config hot-reload picks
the new script up on the next window.

**Test redesign - P4d becomes "chat wins; the job must recover"** (the
deferral semantics it assumed do not apply to submit-and-poll
generation): the 1-token vLLM request is fired mid-generation; the
expected and asserted outcome is the pair evicted, vLLM boots and
answers, the job's 60 s retries take the card back, the pair reloads,
and the job ends **DONE** - which now also *proves* the new wrapper
unblocks the vLLM stop in ~30 s (a2's 15-minute dead stretch becomes a
~3-minute recovery). Journal asserts: vLLM swap start required; the
deferral line is NOTE-level (present or not depending on timing); the
job's take-back line (`evicting [qwen3.8-27b]`) is NOTE-level. P5d (busy
vLLM stream -> door deferral) is unchanged, fire moved 15 s -> 8 s for a
tighter stream hit. Restore unchanged (stock-015 + thin-020).

**Attempt 3** re-runs the same window plan with the new wrapper.
