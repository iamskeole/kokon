# Task-023 (PARKED — owner-triggered) — the job-hold sidecar: make a running image job count as "busy" to the router, without forking llama-swap

**Created:** 2026-11-04 — the owner accepted the stock compromise (chat
request mid-image-job fails the job; the box recovers) and asked that
the **no-fork** alternative be documented for later exploration, "if
concurrent use becomes an issue down the line." This file is that
document. **Do not start this task unless the trigger below is real.**

## Trigger (when this stops being parked)

Concurrent use actually hurts: e.g. two people on different machines
running *different* image models at the same time and one side's job
keeps dying (`lost track of the job`), or a chat request mid-image-job
dying jobs often enough to annoy. If the box stays a mostly-single-user
machine, the stock behavior (last-come-first-served, clean failures,
re-submit works) is fine and this stays parked.

**Framing for the future decision — who waits is the trade, not
whether it's safe.** The status quo: a chat request mid-image-job wins
in ~1-2 min and the *job* dies. The holder inverts that: the *job*
runs to completion and the *chat* waits (its client's own timeout
decides whether it sees latency or an error). Adopt the holder only if
lost jobs hurt more than slow chat.

## Why it works (source-cited, v269 `9fbc47c`)

`internal/router/scheduler/fifo.go` (the stock FIFO scheduler):

- "Busy" = the per-model `inFlight` counter: +1 when the gateway hands
  a request to a model (`grantHandler`), −1 when that request's
  response completes (`OnServeDone`). There is **no declaration
  channel** — the full yaml config surface has no active/hold/lock/
  lease flag (verified 2026-11-04).
- A new request is deferred **iff its evict set contains a model with
  `inFlight > 0`** (`conflictsWithInFlight`); the queue drains when a
  model's counter reaches **zero** (all its requests done).
- Cancellation is self-cleaning: a cancelled/dying handler "will fire
  its tracked serve and reach OnServeDone in the normal way"
  (`OnUnload` comment) — a client disconnect decrements the counter.

So the only ingredient missing for "protect a running image job" is an
**open request against a member of the job's set for the job's
whole lifetime.** The gateway's own counter then marks the set busy,
and the *stock* deferral does the rest. No daemon change.

## The design

**One shared "hold" model** (not one per set): a trivial
**CPU-only, zero-VRAM** HTTP server (~30 lines of Python:
ThreadingHTTPServer; on any request: read/drain the body, then hold
the socket open until the client closes it; serve a health endpoint
for the daemon's probes). One process, one port (pick a free one in
the 3918x-3919x range — none of the image ports are in use there
today), one model entry in the gateway config, one pidfile-based
stop script. It loads and dies like any other member.

**Set membership (why it protects everything):** add the hold to the
image sets' DSL — `klein: "k & p & h"`, `qwen-t: "qt & pq & h"`,
`qwen-f: "qf & pqf & h"` (three-member ANDs are already proven —
stock-015's klein set was `k & p & v`; the 4,608-case replay covers
N-member sets). Because `h` is a member of *every* image set:
- a job's first gateway request loads the pair **and** the holder
  (holder starts in <1 s — no weights, no GPU);
- any newcomer from a *different* set (vLLM orphan, creative, another
  image set) has the holder in its evict set → while the hold request
  is open, the newcomer **defers** instead of evicting;
- the holder is never evicted by image-set-to-image-set swaps (it's
  in both) — same sharing trick as task-019's PUP idea.
Creative (meromero) sets: optional addition — they already
self-protect via their long request; only their *load phase* has the
gap.

**Service-side change (small, the real work):** when the service
submits an image job, open **one long-lived hold request** to
`/upstream/hold/hold` through the gateway, kept open for the job's
lifetime; close it on done/failed/cancelled. Requirements:
- a dedicated client with **no short timeout** — the hold must
  outlive the job (2-4 min); the service's 120 s per-request budget
  must not apply to it;
- on service crash/restart the TCP connection dies with it → the
  holder's socket closes → `OnServeDone` → counter drains → the hold
  releases **automatically** (verify in the window: kill -9 the
  service, confirm the daemon's in-flight for `hold` drops to 0
  within seconds — the cancel path is inferred from the `OnUnload`
  comment; a 5-minute window proves it).

**Timing coverage:** protection must be up before the gap matters.
The job's first gateway round-trip loads the holder (<1 s); the
service opens the hold right after; the enhance step (~10-30 s) and
generation (2-4 min) are then fully protected. If the holder load
ever lags, protection degrades to today's behavior — never worse.

## What it changes (the honest list)

- Chat (vLLM) mid-image-job: **waits** (client-perceived latency, or
  its own client timeout) instead of winning-and-killing-the-job.
- Image-vs-image, different models: the **running job is protected**;
  the newcomer waits (the *service* side still has its 120 s request
  patience — so a newcomer whose wait exceeds ~2 min fails *itself*
  cleanly, re-submittable — the failure moves from the victim to the
  latecomer, which is the fair direction).
- One more small process on the box; zero GPU impact; the holder is
  in no position to OOM anything (no weights).
- Config becomes the next versioned file after whatever lands first
  (stock-017 is currently spoken for by task-021's vision variant —
  order them, or fold in).

## Alternatives considered (same day)

1. **Upstream lease feature** (the clean long-term): a header/endpoint
   the client sets — "this model has a live job until I say otherwise"
   — checked by `conflictsWithInFlight` alongside the open-request
   counter; heartbeat so a dead client can't pin the GPU. Small,
   general, a strong candidate to **ride the task-017 upstream PR**.
   Same service-side change as the holder, no extra process — but it
   is a router modification, and the owner chose stock-only (2026-11-04).
   If a task-017 push ever happens, prefer this over the holder.
2. **Abuse the PUP sidecar** (huge `max_tokens` generation as the
   "open request"): rejected — real GPU compute for no output, and
   llama-server may serialize it with the enhance call it must not
   block.

## Acceptance criteria (when it runs)

- Versioned config with the hold model + updated set DSLs; validates;
  window proves: (a) vLLM 1-token request mid-image-job is **deferred**
  (journal `would evict in-flight process`), the job completes, vLLM
  is served after; (b) hold releases on job completion (next vLLM
  request is immediate, no deferral); (c) `kill -9` of the service
  releases the hold within seconds (daemon in-flight for `hold` → 0);
  (d) same-set jobs unaffected (fast path, no holder involvement);
  (e) image-vs-image: the running pair's job survives a newcomer
  from the other image set, the newcomer's request fails cleanly at
  its own patience if the job is longer.
- Holder process: CPU-only, no GPU memory, <1 s cold start, measured
  in the window; stops cleanly via its cmdStop.
- MATRIX.md gets a short section (the holder is a set member, nothing
  special); `docs/fresh-box.md` lists it.

## Decisions

- **2026-11-04 (owner):** stock stays — the chat-priority compromise
  (a vLLM request mid-image-job fails the job cleanly; box recovers)
  is accepted to get the current work out the door. This task is the
  documented no-fork path if concurrent use ever becomes a real
  problem. The cleaner long-term path is the upstream lease feature
  (above), preferably inside the task-017 PR if that ever ships.

## Blockers

- None technical. The blocker is the trigger: no observed pain from
  concurrent use yet (single-user box; the owner's wife scenario was
  hypothetical).
