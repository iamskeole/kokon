# task-004 — PUP sidecar: llama-server serve script + gateway entry

## goal
A serve script for the PUP running on the **named** llama.cpp
(`backends/llama.cpp`, the May-21 build — proven to load and run the PUP
with vision on both local llama.cpp trees): `llama-server` on port **39175**
with `--jinja` (the BFL chat template is embedded in the GGUF),
`--mmproj` (the `clip`-arch tower), thinking mode off, and the starting
quant. Registered as a llama-swap alias with on-demand load. Readiness is
probed with a **1-token `/v1/completions`** (200 = ready, 503 = still
loading) — never `/health`.

## acceptance criteria
- Gateway on-demand load brings the sidecar up; 1-token probe returns 200.
- A BFL-t2i request via raw `/v1/chat/completions` (BFL system prompt from
  the assets, an image part where needed) produces a BFL-contract caption
  — re-verifying the archived sidecar-proof result on this new path.
- Serve/stop scripts in the deploy location; vllm reloaded at the end
  (this is a GPU window — full autonomy protocol: blocking call, setsid
  watcher, reload via the gateway).

## blockers
- One GPU window (vllm down during it).

## decisions
- Starting quant = 9B `Q4_K_M` (smallest; task-010 picks the production
  winner — the runner just swaps files).
- Port 39175 (the plan's allocation; scratch :9999 stays free).

---

## Done (2026-11-02)

## acceptance evidence

- **Serve/stop scripts in the deploy location:** `backends/pup-serve.sh`
  (the gateway's `cmd`; evict-gpu first, then `exec llama-server` from the
  named llama.cpp May-21 build — `--jinja`, mmproj, thinking off via
  `--chat-template-kwargs '{"enable_thinking":false}'`, ctx 8192, full GPU
  offload, image token window 1024–4096, temp 0) + `backends/pup-stop.sh`
  (pidfile first, port-resolved pid as fallback — kill by PID). Registered
  as the gateway alias **`kokon-pup`** (proxy :39175, checkEndpoint
  /health, ttl 120) in the llama-swap config; port 39175 added to
  evict-gpu's kill list; config validates 7/7 and the daemon hot-reloaded.
- **Gateway on-demand load brings the sidecar up; 1-token probe 200:**
  run 2026-11-02 11:04 — trigger `GET /upstream/kokon-pup/health` (200),
  first 200 on the 1-token `/v1/completions` **through the gateway proxy**
  at t+217s. (Cold page cache after hours of vllm residency: 6.5 GB of
  weights + CUDA-graph capture; the archived proof measured ~4s on a warm
  cache. Both are the realistic job-set budgets — see task-009.)
- **BFL-t2i request via raw `/v1/chat/completions`:** the BFL t2i system
  prompt (read at runtime from the archived `system_prompts.json` asset,
  never printed) + the kitchen prompt → a 1634-char structured caption,
  343 completion tokens — **token-identical to the archived sidecar proof**
  (same prompt, temp 0, May build): the archived result re-verified on the
  new path, now behind the gateway. Vision leg also re-verified: the
  sidecar described the task-002 run's own mug JPEG (1047 prompt tokens,
  76 out, 1.9s) — image part in, description out, through 8081.
- **GPU window, full protocol:** vllm down at start, sidecar test,
  sidecar killed + GPU verified empty, **vllm reloaded through the
  gateway (51s, warm) — the box self-healed, zero owner intervention.**

## findings (input for task-009/010/011)

- **v269 retry-storm on a broken `cmd` (the 10:33 incident).** First run:
  `pup-serve.sh` was 644 (the write tool's default) → the gateway's
  doSpawn got `fork/exec: permission denied` → **500 on every request to
  the alias, with the gateway retrying the spawn on each one, indefinitely**
  — that storm is what locked the service and needed a manual restart
  (and killed the vllm it had spawned, leaving the box idle). Operational
  rule: before registering any alias, verify the cmd script is executable
  AND its preconditions (model files, binary) exist — the gateway will
  hammer a broken cmd on every request, forever.
- **setsid wrapper pids are dead pids (re-confirmed, watcher side).**
  Launching the job via `setsid` and passing the pgrep'd pid to the
  watcher chased a 20-second-lived wrapper: false fallback at t+20s
  (it did the right thing — freed the GPU, reloaded vllm — but the real
  job kept polling until the owner's gateway restart). Fixed: the job now
  self-reports its true pid to `pup-sidecar.job.pid`; the watcher trusts
  that file (grace 60s) and re-reads it every cycle.
- **May-build llama-server only opens its listener after the model has
  fully loaded** (raw /health = 000 for 208s, then up). So the gateway's
  checkEndpoint /health cannot answer early in this build — but the
  1-token `/v1/completions` probe remains the readiness gate, per the task
  (the old-build early-200 failure mode is why the rule exists).
- **Load budget (measured, for the task-009 job-set):** PUP Q4_K_M +
  mmproj = 7706 MiB resident; ~217s first load after a long vllm residency
  (cold cache) vs ~4-10s warm; vision 1.9s; t2i 4.8s (343 tokens @ ~78 t/s).
  A klein job = engine (~10s ready) + sidecar (≤217s worst case) sharing
  the window — the sidecar load dominates; task-009 decides whether the
  two loads run in parallel.
- **`enable_thinking:false` works** (no thought tags in the caption) and
  the PUP GGUF's embedded BFL chat template renders under `--jinja` in
  this build — the sidecar needs no external template/tokenizer files.
- v269 note for task-010: the `/upstream/<m>/...` request while the model
  is loading is a 500/retry (not a queue) in this build — the job gates on
  the probe, never on the trigger response.

## decisions (this task)

- **Alias name `kokon-pup`** (the kokon-* family; the service's VLM
  client in task-006 will address `http://127.0.0.1:8081/upstream/kokon-pup/...`
  — the service stays a plain gateway client).
- **Starting quant Q4_K_M** (the task's decision, unchanged): 7.7 GB
  resident — leaves ~16 GB headroom for a shared-window klein engine.
  task-010 A/Bs Q6/Q8 and the 8B baseline on this exact runner (swap the
  two file paths).
- Sidecar scripts live in `backends/` next to the other kokon engine
  scripts (gateway `cmd`s); the model dir's serve.sh is now a pointer to
  them.
