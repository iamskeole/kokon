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
