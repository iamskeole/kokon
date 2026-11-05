# Task-020 — stock llama-swap as the complete GPU arbiter: retire the wrappers' out-of-band kills and the policy gates

**Created:** 2026-11-04 — the owner, pushing back on the claim that the pinned daemon
"has no concept of in-flight requests", asked to explore whether stock llama-swap has
queue-type knobs that protect active requests. Exploration (below) proved him right;
he said "proceed".

## Reopened (2026-11-05)

The owner's `/home/p/ml` cleanup (2026-11-05) parked this task's artifacts —
the thin wrapper generation (`vllm-serve.sh`, the PUP doors, the supervisor
pair), `gpu-empty.sh`, `evict-gpu.sh`, `config.yaml.stock-016`/`window-020`,
the `m020-*` jobs, `refs/{pre-020,thin-020}`, `MATRIX.md`, `README.local.md` —
in `__new_mess/llama-swap-deploy/`, moved the bespoke vLLM repo
(`backends/stable/qwen38-27b-rtx3090`) to `__backups/`, and re-based the vLLM
runtime on `backends/hyperqwen-032` (the established vllm fork — the vision
offload and the other runtime knobs are native there, no 0.27.1 port) with new
plain direct-exec serve scripts in `backends/_scripts/` and a new unversioned
config `backends/_configs/llama-swap.yaml` (stock-017 content; its vLLM and
creative `cmd`/`cmdStop` paths are not re-pointed yet, and `ml-llama-swap` is
stopped until they are). The box chat route rides a direct
`serve-qwen3.8-27b-text.sh` boot meanwhile. The history below stands as the
record of what was proven on the old layout. **Re-scope: pending discussion
with the owner (2026-11-05 session).**

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
- **Chat-priority is the measured product fact (a3/a4/a6):** an image job is
  submit-and-poll, so a chat (vLLM) request arriving mid-job evicts the pair
  and the job fails cleanly with `lost track of the job` — no OOM, box
  recovers, the pair is reloaded by the job's own retry (the "take-back").
  The FIFO deferral protects only sustained in-flight requests (a vLLM
  stream, a VLM enhance) — never the gaps of a submit-and-poll client (the
  A-boundary, documented in `MATRIX.md`). Owner decision point: keep
  chat-wins (stock behavior, accepted here) or fail-the-chat-instead (needs
  router awareness of image jobs — a non-stock feature).
- **Chat priority — RESOLVED (owner, 2026-11-04): keep the stock behavior.**
  The owner accepts the compromise (a vLLM request mid-image-job fails the
  job cleanly; the box recovers) to get the current work out the door. The
  no-fork alternative — a CPU-only "hold" sidecar member of each image set
  that the service keeps a request open against for the job's lifetime,
  making the stock in-flight counter protect the set — is documented in
  **parked task-023** (owner-triggered: only if concurrent use becomes a
  real problem; the cleaner long-term path is an upstream lease feature,
  a candidate to ride the task-017 PR).
- **API-key consolidation → task-022 (owner question, 2026-11-04):** the
  owner asked whether the vLLM key can move to the gateway. Answer: the
  v269 gateway forwards the client `Authorization` as-is (no per-upstream
  key field), so a single gateway key works iff the upstream is keyless or
  shares the exact key. Target topology (all engines 127.0.0.1-only,
  gateway the sole LAN door with one key) is scoped in task-022 with two
  owner decisions (vLLM keyless-on-loopback vs shared key; kokon UI
  :39152 stay 0.0.0.0 or loopback-only). Not done mid-window (vLLM is
  this session's runtime).

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


**Window attempt 3 (12:43:53-12:53:38, FAIL @ P4d, 585 s)** — passed P1-P4
again and produced the decisive semantics discovery that redefined the
rest of the window:

- **The 12:46:04 race (new wrapper finding, fixed).** The job's 2-minute
  fallback started vLLM canonically (scope; venv 1321104, port bound early
  by the venv fork) at the same moment the daemon ran the new wrapper;
  the wrapper's child hit serve.sh's port-held guard and aborted in ~2 s,
  orphaning the sibling boot. Fix: after a fast child exit with
  `saw_port=0`, the wrapper now watches **90 s for a concurrent boot** and
  adopts it (exit 0 only when that boot later dies). Rehearsal S6 (fake
  vllm, exact shape) passes: adopt at t+6 s, exit 0 on its death. The
  12:46:19 retry hit the WAIT branch and adopted the same boot; healthy
  28 s later.
- **P3 (headline, passed again)**: klein job done in 25 s with vLLM
  resident; journal `matrix: model=kokon-klein set=klein dsl="k & p"
  evict=[qwen3.8-27b] target=[kokon-klein kokon-pup] cost=1`.
- **P4 (passed)**: turbo job 31 s; door pid 1322954 = the 9B Q8_0 PUP
  (the task-014 production door).
- **P4d failed — and correctly so, under the OLD test design.** The
  1-token vLLM request (12:48:18, 0 s router gap) evicted the generating
  pair (`starting swap for model qwen3.8-27b, evicting [kokon-full
  kokon-pup-qwen-f]`, both "exited cleanly"); vLLM booted in 38 s (the
  direct-parent wrapper's child 1324063; the stop at 12:48:57 → process
  gone by 12:49:00, seconds — the a2 15-minute class is dead) and
  answered; the job's retry took the card back the same second
  (`evicting [qwen3.8-27b]`); kokon-full reloaded 12:49:00-08; and **the
  job FAILED 12:49:08 with "The inference server lost track of the job
  (it may have restarted)"**. The old P4d asserted the job would end
  DONE — and that recovery is **impossible**: the service's img_gen is
  submit-and-poll against the engine's job table; when the engine process
  dies the job table dies with it, and the service has no resume path (a
  restarted engine cannot continue a generation it no longer knows).
  Since chat and image cannot co-reside on this 24 GB card at all, "the
  chat request wins; the job fails clean (no OOM, no zombies) and the box
  recovers in ~50 s" IS the product behavior on this box.
- **OPEN mystery (instrumented for a4)**: the 12:49:00 journal line
  `process stopped by llama-swap: exit status 1` indicates wrapper #3
  self-exited 1 ~3 s after the stop signal, even though its child WAS the
  live vllm (port bound ~10 s at the 1-s tick — `saw_port` should have
  latched). The daemon journal does not carry cmd stdout (v269: in-memory
  ring buffer only), so the wrapper now keeps a **persistent self-log**
  (`/tmp/vllm-serve.log`: every branch decision, child pid, exit rc,
  `saw_port` at exit; die() collects it into the workdir, boot log tail
  included) — attempt 4 will settle it directly.
- **401 discovery root-caused (cosmetic)**: the daemon's capcompat
  `/v1/models` probe 401s only when vLLM was launched canonically (key
  from `api_key.txt`) while the daemon config carries a different env
  key — two sources, drift. Daemon-launched vLLM (window path) matches
  and discovery works. Resolved by the owner's single-key proposal →
  **task-022**.

**Test redesign (attempt 4).** P4d asserts the true semantics: vLLM 200;
job ends **FAILED** (lost-track class; `done` would die the window — no
interruption proven); the take-back line `evicting [qwen3.8-27b]`
required; vLLM down + pair port resident at the end (box recovered);
job-error field + service-log lines collected. P5d adapts to the
post-P4d state (pair resident, vLLM down): the 1200-token request fires
first (it boots vLLM), the gate is `vllm_present` (healthy = in-flight;
the request is non-streaming so the response file stays empty until the
end — a file-size gate cannot work), the door request fires 5 s into the
generation window; plus a stronger journal proof: the door's `starting
swap` timestamp must be ≥ the stream's end. `refs/thin-020/` re-synced
and **extended to 12 files** (adds the stable `serve.sh` /
`serve-vision.sh` with `--no-scope`); the restore covers them. The deep
rollback (`refs/pre-020/`) predates `--no-scope`; the scope path of
serve.sh is behaviour-identical to pre-020 (the new code is an additive
branch on a new flag + a log-tag fix) and the exact reversion (delete
the `if [ "$no_scope" -eq 1 ]` block, restore the plain separator line)
is recorded here so a byte-perfect pre-020 serve.sh is reconstructible.

**Owner decision points accumulated** (both go into the post-window
report): (1) chat-priority semantics — stock behavior: a chat request
mid-image-job fails the job (cleanly, recoverable box); the alternative
(defer/fail the chat instead) needs the router to know about image jobs —
a non-stock feature, upstream-PR candidate (task-017 era). (2) the
owner's one-door single-key topology — new **task-022** (vLLM
loopback-only + keyless; gateway the sole LAN door with one key; all
engine binds 127.0.0.1; works on stock v269 because the gateway
forwards the client Authorization as-is and v269 has no per-upstream key
injection).

**Attempt 4** re-runs the window with the redesigned P4d/P5d.


**Window attempt 4 (13:40:33-13:47:32, FAIL @ P5d, 705 s)** — passed
**every real proof**; the sole failure was the window's own journal-parsing
code.

- **P1-P2**: window-020 live. The 13:42:45 wrapper (invoked on the P2
  route probe) raced the 2-min fallback's canonical boot; its child hit
  the port-held guard and the **new 90 s concurrent-boot grace worked
  exactly as designed**: adopt at t+15 s, watch, exit 0 on eviction.
  (a3's same shape had orphaned the boot; now one wrapper survives to
  adopt.)
- **P3 (headline)**: klein done 25 s over a resident vLLM
  (`evict=[qwen3.8-27b]`).
- **P4**: turbo done; door = the 9B Q8_0 PUP.
- **P4d (chat priority — complete)**: vLLM request 13:45:12 →
  `starting swap for model qwen3.8-27b, evicting [kokon-full
  kokon-pup-qwen-f]` (pair out 13:45:13-17; the job's own poll request
  queued as `collides with in-flight swap`) → vLLM 200 13:46:03 → the
  job's retry takes the card back 13:46:01 (`queued request for model
  kokon-full now starting swap, evicting [qwen3.8-27b]`) → vLLM process
  out 13:46:05 (wrapper self-log: child rc=0, `saw_port=1` — the stop
  propagates in ~4 s) → **the job FAILED 13:46:13 with exactly
  `generation_failed: The inference server lost track of the job (it may
  have restarted).`** (error field + service-log line collected) → box
  recovered (pair reloaded, vLLM down, ports free).
- **P5d (reverse deferral — complete)**: stream fired 13:46:18 (boots
  vLLM; wrapper `saw_port=1`, rc=0); **13:47:09 `queuing request for
  model kokon-pup-qwen (would evict in-flight process)`** (the stream
  mid-flight); stream lands intact 13:47:17 (5,913 chars);
  **13:47:17 `queued request for model kokon-pup-qwen now starting
  swap, evicting [qwen3.8-27b]`** — the deferred request released in the
  same second the stream ended; vLLM out 13:47:18; door ready 13:47:27,
  answered 13:47:32.
- **The FAIL was two test-code bugs of mine, not box behavior**:
  (1) the journal's timestamp is three fields (`Nov 04 HH:MM:SS`) but my
  awk took two → parsed midnight; (2) `grep -m1 "starting swap for
  model kokon-pup-qwen"` matched the **qwen-f** door (substring) first —
  and in any case v269 logs a resumed deferred request as
  `queued request … now starting swap`, never as a fresh `starting
  swap`. Fixed (`tail -n1` on the correct patterns + 3-field parse) and
  **dry-run against a4's own journal-early.txt passes**: deferral -8 s
  vs stream end, release +0 s.
- **Mystery closed — a3's 12:49:00 `exit status 1`**: a4's self-log
  shows the final wrapper handling that exact scenario correctly (both
  vLLM-lifetime wrappers: `saw_port=1`, rc=0, exit 0; stops propagate in
  3-4 s). Three wrapper invocations in a4, all clean — the a3 one-off
  is empirically non-recurring (its pre-rewrite log is gone; cause
  unrecoverable).
- **The 401 artifact is fully root-caused (cosmetic)**: capcompat
  discovery 401s when the daemon's probe key and vLLM's enforced key
  differ (a4's 13:47:00 line: daemon probe, no key in the config `env:`
  vs vLLM enforcing the `api_key.txt` fallback). Serving always carries
  the client's key, so it is unaffected. The restore probe's 401
  (13:52:37) was transient (a boot race between the canonical and a
  daemon-launched vLLM; the surviving listener 1370625 enforces the
  `api_key.txt` key — verified: wrong key 401 / api_key 200 / none
  401). The missing key in the worker child's environ is vLLM's own
  spawn behavior (it prunes the API key from worker envs) — an earlier
  check had hit the worker, not the API server.
- **P6 (orphan reclaim) and P7 (cutover) never ran** (the marker
  stopped at P5d). **Attempt 5** re-runs the full window with the fixed
  proof code; everything else is unchanged since a4.


**Window attempt 5 (14:07:14-14:13:23, FAIL @ P4d, 645 s)** — a new
finding, fixed in the window design (a6).

- **P1-P4 clean**: klein done 25 s over a resident vLLM (the 14:10:12
  headline line, `evict=[qwen3.8-27b]`); turbo done; 9B-Q8 door.
- **P4d died on the job gate: the quality job never reached
  `running`** — it failed with `The inference server is not reachable`
  at exactly +120 s after the enhance step. Root cause: **a
  cold-cache load of the full pair exceeded the service's 120 s
  per-request timeout**. The pair load (20 GB Q8 DiT + 9B-Q4 door)
  started 14:11:12 (0 s after the turbo pair's exit); the door was
  ready in 7 s (14:11:19) and the enhance finished (10.3 s,
  14:11:23), but the 20 GB DiT load needed more than 120 s — the
  engine's HTTP server binds early and held the img_gen POST in the
  load window until the service timed out (14:13:23, exactly 120 s
  later). The engine log ends mid-sequence (killed by the restore's
  cmdStop) — the load would have finished ~10 s later. In a4 the same
  load took ~7 s (warm page cache). Why cold in a5: vLLM had booted
  three times between a4 and a5 (a4-restore, a5-P2, a5-P4d's vLLM
  request); each boot reads ~22 GB of weights into the page cache and
  evicts the engine weights. A window-design hazard (the job's request
  clock racing a load), not a router failure.
- **Fix (a6): canary preloads.** P4d now fires
  `GET /upstream/kokon-full/sdcpp/v1/capabilities` (the router's own
  health endpoint) before the job: it triggers the same on-demand load
  and blocks until the router's health check passes, so the job's
  clock never races the load. P5d gets the symmetric 1-token vLLM
  canary before the stream (the vLLM weights are likewise evicted by
  the P4d pair loads; a cold boot could exceed the loop's 300 s
  wait). The job's self-deadline grows to 40 min (the watcher's
  42-min cap stays the true backstop). Canary results land in the
  digest.
- **The restore 401 recurred (14:18:27)** — same transient boot-race
  class (canonical vs daemon-launched vLLM during the restore); the
  surviving vLLM (1390317) enforces the correct key. Cosmetic; the
  one-door topology (task-022) eliminates the whole class.

## window attempt 6 — 2026-11-04 14:41-14:52 (DONE, 631 s) — the box is on stock-016

- **P2**: 2-min fallback → canonical start 14:43:28; the daemon's own
  invocation (the route probe) raced it — child hit the port-held guard,
  **adopted the canonical boot at t+15 s** (the grace), healthy
  through the gateway 14:44:21.
- **P3 (headline)**: klein done 14:44:47 (26 s); journal 14:44:21
  `matrix: model=kokon-klein set=klein dsl="k & p" evict=[qwen3.8-27b]`
  — the klein pair evicted a RESIDENT vLLM to run.
- **P4**: turbo done 14:45:22; door = 9B Q8_0 (pid 1410768, 10.9 GB).
- **P4d (chat priority, complete)**: canary 200 in 11 s (pair warm);
  job seed 22; vLLM request 14:46:0x → **14:46:04 `starting swap for
  model qwen3.8-27b, evicting [kokon-full kokon-pup-qwen-f]`**; vLLM
  answered 60 s after the request; the job's retry took the card back
  **14:47:02 (`queued request for model kokon-full now starting swap,
  evicting [qwen3.8-27b]`)**; **the job failed 10 s after the vLLM
  answer with exactly `generation_failed: The inference server lost
  track of the job (it may have restarted).`** — the expected
  chat-priority semantics; box recovered (pair reloaded, vLLM down,
  ports free).
- **P5d (reverse deferral, complete)**: vLLM canary 200 in 41 s (cold
  boot absorbed); stream in flight; **14:48:01 `queuing request for
  model kokon-pup-qwen (would evict in-flight process)`** → stream
  landed intact 14:48:12 (6122 chars) → **14:48:12 `queued request for
  model kokon-pup-qwen now starting swap, evicting [qwen3.8-27b]`** —
  the deferred request released in the same second the stream ended;
  vLLM clean exit 14:48:14; the deferred door answered 10 s later.
- **P6 (orphan reclaim)**: idle turbo door resident; vLLM request
  14:48:23 → **`matrix: model=qwen3.8-27b set= dsl="" evict=[kokon-pup-qwen]
  cost=0`** — immediate eviction, no deferral (an orphan). Note: v269
  prints the orphan set as EMPTY — the `auto:orphans` name never
  appears in the journal; the window's earlier grep pattern was wrong
  (fixed in `m020-test.sh`).
- **P7 (cutover)**: stock-016 live 14:49:17; the reload cycled vLLM;
  2-min wait → canonical start 14:51:28 (the daemon's invocation raced
  again, adopted at t+15 s); vLLM healthy and serving this session
  (pid 1417932, 23.2 GB); final state verified: `config.yaml ==
  stock-016`, active=klein, all image ports free.
- **Self-log (the forensics that was missing in a2/a3):** six wrapper
  invocations across the window, all clean exits, both canonical-race
  shapes absorbed by the 90 s concurrent-boot grace, every stop
  propagating in <50 s. The a3 12:49:00 one-off `exit status 1` class
  never recurred — empirically closed.
- **Product facts and journal formats** from a3-a6 are now in
  `MATRIX.md` (deploy dir): the A-boundary (deferral does not protect
  submit-and-poll clients → chat wins a mid-job conflict by design),
  the three journal line shapes, and the cold-page-cache load hazard
  (canary preload through the router's health endpoint).

**Task-020: DONE.** Acceptance criteria met (all window phases proven
live; wrappers make no out-of-band kills; policy gates retired;
vLLM a pure orphan under stock v269; cutover to `config.yaml.stock-016`
completed and verified; rollback ladders recorded). Remaining owner
decision points (non-blocking): chat-priority vs fail-the-chat (above)
and the task-022 one-door topology.

## Re-land on the new layout (2026-11-05, IN PROGRESS)

The owner's `/home/p/ml` cleanup re-based the box (see the Reopened
section). The re-scope discussion (2026-11-05) fixed the new shape with
the owner's decisions; the original goal of this task (the router as
the sole eviction authority, no out-of-band kills) is carried over
verbatim — it is the contract the new layout was built on.

**Owner decisions (2026-11-05):**

1. **Structure:** `backends/*` non-underscore dirs = pure stock trees
   (clean for future origin updates — no deployment artifacts inside
   them); `backends/_scripts/` = boot scripts only; `backends/_configs/`
   = the config home; previous generations parked cold in
   `__new_mess/` (this task's: `llama-swap-deploy/`, and the old sdcpp
   wrapper generation in `sdcpp-deploy/`); the bespoke vllm repo is in
   `__backups/` (out of service).
2. **Scripts are pure boots:** preflight + `exec` of the engine — the
   daemon's tracked process IS the engine. **No `cmdStop` anywhere:**
   the daemon's default process-group stop is the only unload path. v269
   starts every cmd with `Setpgid` (verified in `internal/process/`
   `runtime_unix.go` + `process_command.go`), so the group signal reaches
   the engine and its children — including vLLM's `EngineCore` child.
   SIGTERM first, SIGKILL after the model's `unloadTimeout` (240 s on the
   vLLM variants, 120 s on the rest).
3. **Zombie nets retired:** `gpu-empty.sh` / `evict-gpu.sh` /
   `ensure-gpu-free.sh` are gone (the owner deleted the last one).
   A process started OUTSIDE the daemon is invisible to its eviction —
   it is now a manual problem, resolved manually (the mutual-exclusion
   protocol; nothing the gateway runs may kill it).
4. **The vLLM scripts are the owner's final versions** (`serve-
   qwen3.8-27b-{text,vision}.sh`: pure boots of the `hyperqwen-032`
   venv with the owner's own measured parameters — KV 8000M / fp8, the
   vision offload and mm cap on the vision variant). Untouched by the
   re-land, not re-measured, **not part of any window** (owner rule:
   no vLLM phases in the verification window).
5. **Three separate PUP scripts** for the re-land (zero quality risk on
   the cutover); the shared single door is task-019 (re-scoped, parked).
6. **Config:** `backends/_configs/llama-swap.yaml` is the config home
   (successor of the parked `.stock-013…017` ladder; same 10-model
   topology and the stock-017 sets); a `.last-good` snapshot lives
   beside it (a failed edit is ignored by `-watch-config` — the last
   good config stays live; a successful edit is a hot reload that stops
   loaded models — never edit mid-job).
7. **The window** (pending owner sign-off on the plan): no vLLM phases
   (decision 4); the owner stops the manual vLLM boot before the window
   and revives it after (card housekeeping is the owner's, not the
   window's); phases prove the non-vLLM invariants on the new layout.

**Done (2026-11-05, host side — non-git):**

- `backends/stable-diffusion.cpp/` decontaminated: `deploy/` (the whole
  old wrapper generation: supervisors, stop scripts, PUP doors,
  `QWEN-ROLLBACK.md`, logs, and the copied build artifacts) +
  `.build-DONE` → `__new_mess/sdcpp-deploy/`; the tree is now clean
  (`git status` empty; `build/` + `build.log` are gitignored build
  state). The `llama.cpp` / `llama-swap` / `hyperqwen-032` trees were
  already clean.
- **8 boot scripts in `backends/_scripts/`** (all `sh -n` clean, exec
  bit set, `exec`-terminating): `serve-kokon-qwen-image-2.1-{turbo,
  full}.sh` (stock sd-server @ 290148d, :39160, the two DiTs),
  `serve-kokon-flux-image-3-klein.sh` (:39170), and the three PUP doors
  (`serve-kokon-pup-flux-image-3-klein.sh`, `serve-kokon-pup-qwen-image-
  2.1-turbo.sh`, `serve-kokon-pup-qwen-image-2.1-full.sh` — renamed by the
  owner 2026-11-05 so the doors mirror the engine names; llama-server
  Oct-14 tree on :39175/76/77, the task-010/014 production flag sets,
  ports baked in)
  — recipes carried over byte-for-flag from the parked wrappers, minus
  every eviction/kill/adopt/pidfile line. `serve-g4-meromero-v2-{26,31}b.
  sh` fixed in place: the `gpu-empty.sh` line deleted, the binary
  re-pointed to `backends/llama.cpp/build/bin/`, and an `LD_LIBRARY_PATH`
  for the shared build added — **the Oct-14 `llama-server` binary bakes
  an absolute RUNPATH at its old path** (`backends/latest/llama.cpp/
  build/bin`, which no longer exists); the `.so` sits next to the binary
  and the env var resolves it (verified: `--version` prints
  `0.6.0-dev … commit 529ac893e`). The `_todo_` stubs are gone;
  `ensure-gpu-free.sh` deleted (decision 3).
- **`backends/_configs/llama-swap.yaml` rewritten:** same 10 models and
  stock-017 sets; every `cmd` points at `_scripts/`; **all `cmdStop`
  entries deleted**; header + per-model comments describe the new
  contract (router = sole eviction authority; the pair forms from the
  job's two requests; the vLLM scripts are the owner's final versions);
  `logToStdout: both` (owner, 2026-11-05: every engine's stdout — all ten
  models' cmd processes — is copied to the daemon's stdout = the systemd
  user journal, so `journalctl --user -u ml-llama-swap` carries the
  engines' own output, not just the daemon's decision lines).
  `llama-swap -config … -validate`: **"10 model(s), 0 peer(s)"** on the
  built v269 binary. `llama-swap.yaml.last-good` seeded (re-seed after
  the window passes).
- **`ml-llama-swap` repointed and started:** unit now runs
  `backends/llama-swap/build/llama-swap-linux-amd64 -config
  backends/_configs/llama-swap.yaml -listen 127.0.0.1:8081
  -watch-config`; active, `/health` OK, `/v1/models` 200, **no engine
  processes running** (the daemon boots nothing until a request).

**Acceptance criteria (re-landed):**

- [x] Config `-validate` clean (10 models); **zero `cmdStop` entries**;
      no script kills or evicts out-of-band (grep proof over
      `_configs/` + `_scripts/`); every `cmd` has the exec bit (the
      qwabl lesson).
- [ ] **Daemon default stop releases VRAM for each engine class**,
      measured in the window: sd-server (stock build — **watch item:**
      its SIGTERM reliability on this build is unproven (the fork era
      observed zombies); if it hangs, the `unloadTimeout` SIGKILL
      backstop is the fallback — measure the propagation time and lower
      the timeout if it's the 120 s worst case) and the llama-server
      trees (llama.cpp handles SIGTERM natively; the PUP door and the
      creative writers both prove this path).
- [ ] **Window (no vLLM phases; owner stops the manual boot before,
      revives after):** (a) the klein pair forms from two requests
      (kokon-pup, then kokon-klein — journal set lines for both, the
      two pids co-resident); (b) a turbo job evicts the pair (journal
      `evict=[kokon-klein kokon-pup]` — both members, one set); (c) a
      quality job; (d) a creative writer boots, serves a 1-token
      request, and is unloaded by the daemon (group-stop propagation
      time); (e) teardown + final state (config identity, ports free,
      vLLM untouched by the daemon all along) + **DONE marker last**.
- [ ] Docs (after the window, owner sign-off): `docs/fresh-box.md`
      (the new layout: stock-tree purity rule, `_scripts`/`_configs`
      homes, no cmdStop, the `.last-good` convention, the mutual-
      exclusion protocol); the host OPS notes; the add-a-model guide
      re-homed from the parked `MATRIX.md` (task-021's); the daemon
      unit line in whatever records it.
- [ ] `.last-good` re-seeded on the window-passed config.
- The task moves to `done/` only after the window passes and the docs
  land. The original task-020 record above (the v269 scheduler
  exploration, the stock-016 design, the six window attempts) stays as
  the provenance of the invariants being re-landed — the FIFO deferral,
  chat-priority semantics, TTL-skip and journal formats all carry over
  unchanged (same v269 binary).
