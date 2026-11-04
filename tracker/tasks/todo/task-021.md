# Task-021 — the vision vLLM variant (qwen3.8-27b-vision on :18022) + the "add a model / model combo to the gateway" guide

**Created:** 2026-11-04 — the natural next step after task-020 closed on
`config.yaml.stock-016`: the box has the pure-orphan vLLM main model
(`qwen3.8-27b`, :18020) and the owner wants the **vision variant**
served the same way, as a second pure-orphan vLLM model
(`qwen3.8-27b-vision`, :18022) — same weights dir, different served
name / flags — plus a **written guide** in `MATRIX.md` for adding any
new model or model combo to the gateway, so the next person (human or
agent) never has to rediscover the topology.

## Goal

1. **A second vLLM instance on :18022** serving the vision-enabled
   model as `qwen3.8-27b-vision`, launched by a thin
   `vllm-serve-vision.sh` wrapper with the **exact same contract** as
   `vllm-serve.sh` (task-020): pure orphan, no out-of-band kills,
   direct-parent `--no-scope` launch, adopt/wait/launch branches,
   90 s concurrent-boot grace, persistent self-log
   (`/tmp/vllm-serve-vision.log`), `gpu-empty.sh` last-resort only.
   The two vLLM instances share the GPU: 27B-W4A16 at
   `--gpu-memory-utilization 0.97` fits only one at a time, so both
   models stay **pure orphans** — a request for either evicts the
   other (and anything else) through the router; they never co-reside
   (24 GB card, measured).
2. **The launcher stays one source of truth**: `single-user/start_qwen.sh`
   gains an env-gated `SERVED_NAME` / `VISION` path (stock
   `--language-model-only` vs the vision flags) so both instances come
   from the same script tree; `serve-vision.sh` (already in the
   stable dir) is the :18022 front. No new copy of the weight-loading
   logic.
3. **The guide**: a `MATRIX.md` section "Adding a model / model
   combo to the gateway" — the five pieces every new model needs
   (a model entry: proxy/cmd/cmdStop/ttl; the set membership if it
   co-resides, or none for an orphan; the wrapper contract for
   anything the daemon can't supervise directly; the versioned
   config + validate + window protocol; the journal lines that prove
   it works), with the vision variant as the worked example.
   Reflection in `docs/fresh-box.md` (the model list gains the 10th
   model).

## Blockers

- The owner's two **task-022** decisions are independent but should
  be settled in the same session: the vision instance's bind/key
  follows whichever one-door topology the owner picks (loopback
  keyless vs shared key), so build the launcher to take the key from
  the same single source the 022 decision names.
- One 24 GB GPU shared with the session's own vLLM: the verification
  window evicts :18020 while :18022 is up (and vice versa) — the
  session's chat route goes quiet during the window (marker-last
  protocol; the owner is the judge).

## Decisions

- (pending) Which vLLM flags the vision variant runs with (owner
  spec or the stable-dir `start_qwen.sh` vision defaults) — confirm
  at start; engine parameters otherwise stay unchanged (task-020
  constraint).
- (pending) :18022 port is free today (verified pre-task-020
  preflight); keep it.

## Acceptance criteria

- `config.yaml.stock-017` (next versioned config): `qwen3.8-27b-vision`
  entry (proxy :18022, `vllm-serve-vision.sh` cmd, `cmdStop` = the
  vision stop), in **no set**; the six kokon models unchanged from
  stock-016; validates; window proves: (a) a vision request boots the
  vision instance and answers; (b) a main-vLLM request mid-vision-job
  evicts it (orphan-vs-orphan) and vice versa; (c) a kokon job still
  evicts both vLLMs (the sets are unchanged); (d) after the window,
  cutover to stock-017 with the box serving this session.
- `MATRIX.md` carries the guide section; `docs/fresh-box.md` lists the
  10th model with its provenance line.
- No out-of-band process kills anywhere (the task-020 invariant
  extended to the second vLLM); self-logs land in `/tmp/`; window
  ends in the stock config with the marker last.
