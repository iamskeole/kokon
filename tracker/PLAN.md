# PLAN — kokon v2: completing the separation

Status: **FINAL** — decisions resolved 2026-11-01 (§6). This file lives in
`tracker/`, which is fork-owned working state: it supersedes `docs/STATUS.md`
on this fork and is **not contributed upstream** (if the PR happens, the PR
branch is built from upstream + the contributed files only).

## 0. Running rules (agreed)

1. **Stock as much as possible.** Every inference backend comes from a public
   upstream (sdcpp, llama.cpp; maybe vllm later, if a quant both vllm supports
   and the 3090 fits). There are no maintained engine forks in the stack. The
   kokon repo (this repo) is the only custom code — and it may disappear if
   merged upstream; the PR/merge/philosophy discussion happens when we're done.
2. **Smart reuse of artifacts.** One PUP (the klein fine-tune) is the candidate
   shared VLM for all image pipelines (klein, qwen-full, qwen-turbo) — adopted
   for a pipeline only if A/B shows it works there; otherwise that pipeline
   keeps the VLM it recommends. One weight family, one mmproj, per-model
   serve scripts, per-model provenance.
3. **Separation of concerns.** The UI/service only ever points at
   **entrypoints, API keys, and prompts**. Model load/unload/co-residency is
   the gateway's (llama-swap's) job. Anyone using this repo can run the
   engines standalone on bare metal — the service doesn't care how.

## 1. Target architecture

```
[UI] ─▶ [kokon service: docker image built from this repo]
            per-pipeline config = { engine entrypoint, VLM entrypoint (optional),
                                    API key, prompt bundle }
            │
            ▼
     [llama-swap gateway]   ← single entrypoint; on-demand load/unload;
                              a "job" is a set of models with one lifetime
            ├─▶ stock sdcpp — klein engine        (DiT + 4B conditioner + VAE)
            ├─▶ stock llama.cpp llama-server      (PUP sidecar: prompt enhancement, vision)
            ├─▶ stock sdcpp — qwen engine         (Qwen-Image 2.1 + turbo)
            └─▶ (later) vllm, if a 3090-fitting quant it supports turns up
```

- The service embeds and builds no engine; it is an HTTP client.
- Engines are stock checkouts built on the host; the gateway starts/stops them.
- Prompt logic ships in this repo as data assets the service reads.

## 2. State: what is already built, what this plan closes

**Built and live (the `flux` branch, image 0.11.2-deb6e00) — kept, never
rebuilt:**
- External-engine mode (the v1 unbundling): the service runs as a docker
  image with no bundled engine; model picker over the config `models` list
  (schema 12), per-model capability memory across restarts,
  `/models/switch` that never touches the GPU, on-demand load through the
  gateway, the status pill as indicator, failure-diagnosis docs, remote
  login behind a reverse proxy.
- Klein UI: the layout-box editor (draw / describe / place; v2 with
  per-reference tabs, move, and the model's own box language), "show the
  boxes a prompt used" on results and references, the fresh-prompt box
  pill, Paint/Boxes complementarity per model, the model name in the
  metadata bar.
- Klein service: `enhancer.rs` — BFL's official PUP prompts + sampling, the
  FLUX.3 layout-box contract (detect / lock / reconcile the
  prompt-embedded table), the two-layer remove backstop, one retry on a
  degenerate upsampler answer; `bbox.rs` box data; `sdcpp.rs` the sdcpp
  protocol client.

(An older parallel line, `pr-external-engine` — 10 DCO commits, unpushed —
carries the first cut of the external-engine work plus site tweaks. It is
the parked PR candidate, re-baselined when the PR discussion happens; it is
not part of the live path.)

**The gap this plan closes:**
1. Engine entrypoints are not fully config-driven (partially baked into the
   service) → complete per-pipeline config surface.
2. Prompts are code (`enhancer.rs`) → data assets in this repo, overridable.
3. The service has no external-VLM client — the PUP currently lives *inside*
   the klein engine (fork) → PUP becomes a sidecar endpoint.
4. Production engines are not stock: klein = a fork over the kokon project's
   `kokon`-repo branch (1814814) + 3 PUP commits (audited: 100% PUP slot,
   zero DiT/conditioner changes); qwen = a fork over `0a06712` + 21 commits
   (audited: all in-engine LLM text machinery). The canonical upstream
   (**leejet/stable-diffusion.cpp**) has had day-0 FLUX.3 klein support
   since 2026-10-16, so both pipelines have a stock target — and share one
   build.
5. The gateway manages one model per entry, not a set per job.
6. No fresh-box story — the separation only counts when a stranger can stand
   the whole stack up from this repo.

## 3. Host layout (as agreed 2026-11-01)

```
<host>/ml/              the "prod" stack
  backends/
    llama.cpp/          named slot: the personally-tested build (currently the May-21 tree)
    latest/             cutting-edge syncs: llama.cpp/, stable-diffusion.cpp/ (= canonical
                        upstream leejet/stable-diffusion.cpp), HyperQwen/
    llama-swap/         the gateway (upstream mostlygeek/llama-swap), code 100% clean at its pinned tag
    llama-swap-deploy/  (WP0) our deploy artifacts: config, helper scripts, built binary, state
    stable/             (interim, owner-owned) the host's vllm runtime
  frontends/
    kokon/              deploy of THIS repo: compose + settings pointing at the gateway
    openwebui/          stock venv (chat-only frontend)
  models/
    gguf/{author-or-org}/ModelName/   model-<QUANT>.gguf, mmproj-<PRECISION>.gguf,
                                       mtp-<QUANT>.gguf, serve.sh, provenance.md
    huggingface/        HF state dir (a shell pointer file exposes it to non-interactive shells)
    sd-cpp/             DiT-side weights (klein flux3 stack, qwen-image-2.1, upscalers)
  __mess/               the archived pre-cleanup working mess — read-only history, never deleted
  OPS.md                machine-local ops notes (shell/env specifics, GPU protocol, ports)

<host>/code/iamskeole/kokon/   ← this repo; all product work happens here
```

**Rules the layout encodes:**
- **Named slot** = a pinned public-upstream commit that has passed personal
  testing. `latest/` = cutting edge, synced and rebuilt occasionally. A
  `latest/` build is promoted to the named slot only after it passes the
  owner's test.
- **No forked code under backends/.** If an engine genuinely needs a change:
  upstream PR, never a maintained fork.
- **Every model dir carries `provenance.md`**: source (org/repo/URL), quant,
  license, download date, sha256, notes (converter, imatrix, etc.). Filenames
  standardized per the pattern above so scripts can be dumb.

## 4. Work packages (execution order = task numbers in tracker/tasks/)

**WP0 — housekeeping (task-001…003).**
- task-001: llama-swap → pin at v269; deploy artifacts move to the non-git
  sibling `backends/llama-swap-deploy/`; rebuild; one restart window.
- task-002: named sdcpp checkout `backends/stable-diffusion.cpp` pinned at
  **leejet master** (created 2026-11-01 at `290148d`; day-0 klein since
  10/16) — **with the CUDA build phase** (`SD_CUDA=ON`, sm_86, patched ggml
  for SageAttention, cutlass submodule; ~70 min CPU) — one build serves
  both pipelines (klein now, qwen in WP3); deploy dir + serve/stop scripts
  per pipeline.
- task-003: **all** model weights → the models convention (`{org}/ModelName/`,
  standardized names, serve.sh, provenance.md): the whole `models/sd-cpp/
  flux3/` stack (DiT, VAE, 4B conditioner + mmproj, PUP quants + mmproj,
  8B VLM + mmproj) and any non-conforming `models/gguf/` dirs; `models/
  huggingface/` untouched (HF_HOME, owner-owned).
- (Done 2026-11-01: the parked `wip-q35-engine` branch deleted per D-WIP.)

**WP1 — the sidecar + service wiring (task-004…008). Surgical by design:
none of the built klein work is rebuilt; the only klein behavior that changes
is *where the PUP tokens come from*.**
- task-004: the PUP sidecar — llama-server from the named llama.cpp (May-21
  build, proven with the PUP) on :39175 (`--jinja`, mmproj, thinking off) +
  gateway alias; readiness = 1-token completion probe, never `/health`.
- task-005: per-pipeline config surface `{ engine entrypoint, VLM entrypoint
  (optional), API key, prompt bundle }` — everything pointable, nothing
  baked in.
- task-006: the service VLM client (OpenAI chat, image parts, non-streaming
  v1) replaces the in-engine `/sdcpp/v1/text_gen` call inside the unchanged
  `enhancer.rs` orchestration; **VLM down = hard-fail the job** (D-FB).
- task-007: prompts as data assets — the klein BFL set (the four system
  prompts, chat template, tokenizer, preprocessor config) committed to the
  repo, read at runtime, never echoed.
- task-008: image build + `frontends/kokon` deploy + end-to-end smoke
  (owner visual validation).

**WP2 — klein goes stock (task-009…011).**
- task-009: gateway job-set — one klein job stands up `{ klein engine, PUP
  sidecar }` with one lifetime (native v269 config if it can express a set,
  else the serve-script pairing — decided inside the task).
- task-010: the PUP A/B — 8B {Q4_K_M, Q8_0} vs 9B {Q4_K_M, Q6_K, Q8_0} on the
  same runner, the three standing prompts, markers + digest, owner visual
  validation → the production PUP + quant.
- task-011: cutover — production klein = stock engine + sidecar only; the
  forked deploy retires (→ `__mess/`, rollback dir kept); the 3-box i2i acid
  test.

**WP3 — qwen goes stock (task-012…014).**
- task-012: the 21-commit audit over 0a06712 (classify: in-engine text
  machinery = retire vs needed = port upstream).
- task-013: the qwen engine becomes the **same stock leejet master build**
  (0a06712 verified on the leejet line; pin per the named slot); deploy +
  parity check; the 21-commit fork retires.
- task-014: D-QW — A/B the shared PUP for qwen (klein fine-tune vs
  8B Qwen3-VL on qwen's enhancement prompts) → qwen enhancement cuts over to
  the sidecar with the winner.

**WP4 — the matrix + close (task-015…017).**
- task-015: generic N-models-per-job configuration in the gateway (the
  "matrix"); if v269 can't express a set: upstream PR to llama-swap (the
  stock-first rule — never a fork).
- task-016: the fresh-box doc — clone → docker build → `frontends/kokon`
  deploy → stock engine recipes → model downloads per the provenance
  convention → done. Written last; it describes the final state.
- task-017: the upstream PR / merge / philosophy discussion — the owner
  decides what to contribute (the tracker and the AGENTS.md fork preamble
  stay ours unless the owner says otherwise).

## 5. Foot-guns (the machine-specific ones live in the host OPS notes)

1. One GPU, one model at a time; the gateway is the only authority on
   co-residency.
2. The host runs a large chat LLM (vllm, ~23.4GB) on the same GPU, on-demand
   through the gateway, and the operator's tooling depends on it. GPU jobs
   follow the host's proven **autonomy protocol** (full text in the local
   OPS notes): a self-contained setsid job plus a detached backstop watcher,
   launched by a *blocking* shell call that stays pending until the job's
   marker appears — the harness's result injection is what wakes the
   assistant to review and continue without any user nudge. The job (and
   the watcher on the crash path) restores the chat runtime through the
   gateway before it ends; readiness is probed with a real 1-token
   completion, never `/health`.
3. BFL contract strings (special tokens, system prompts) are never echoed
   into logs, chat, or docs — they live in the repo's asset files and are
   read silently.
4. Host scripting hazards: dash (no `\|` in grep, no arrays); setsid for
   host processes that must outlive the shell; kill by PID, never
   `pkill -f` (self-matches); same-tag nested heredocs collide; a failing
   multi-edit script writes nothing; SSE routes need `--max-time`;
   **always `ls -a`** (venvs, config, and shell-pointer files are
   dot-hidden); no cargo fmt on this host; host cargo needs the OPENSSL env
   pair; container restarts kill web sessions; service tokens are read at
   runtime, never written down.
5. `__mess/` is read-only history. Nothing there gets resurrected into the
   live stack without an explicit decision.

## 6. Decisions (resolved 2026-11-01)

- **D-K1** — klein engine pin = **leejet/stable-diffusion.cpp master**
  (canonical upstream; day-0 FLUX.3 Image Klein / Klein Base since
  2026-10-16). Initially proposed the kokon project's `kokon`-branch pin
  (1814814) because the audit clone's `master` was stale (10-11, pre-day-0);
  the owner corrected this — the branch is now moot. One named build serves
  both klein and qwen. **Open item (tracked in STATUS):** pin + build-verify
  the specific master commit in task-002; keep the pin stable (leejet
  master moves daily) and re-verify on deliberate syncs.
- **D-LS** — the llama-swap cleanup runs as WP0 (task-001): pin v269,
  deploy artifacts to the non-git sibling.
- **D-QW** — the shared PUP for qwen: **A/B first** (task-014).
- **D-FB** — VLM down mid-job = **hard-fail**. No raw-prompt fallback:
  making the VLM work is our job, so this shouldn't happen.
- **D-WIP** — the in-engine Qwen3.5 port branch is **deleted** (was
  ff69b0d; a rabbit hole we should not follow anywhere). Done 2026-11-01.
- **D-ST** — the host's `stable/` (vllm) is **owner-owned** and stays as-is
  for now.

## 7. How this plan is used (the tracker convention)

- `tracker/STATUS.md` = one-line items, `[ ]` open / `[x]` done; updated at
  the end of every task.
- A task is `tracker/tasks/todo/task-NNN.md` → moved to `tracker/tasks/doing/` when
  picked up → `tracker/tasks/done/` when complete. Task shape: `# goal` /
  `## acceptance criteria` / `## blockers` / `## decisions`. The **oldest
  file in todo/ is always the current task** — that is how fresh sessions
  stay focused.
- A fresh session: pi auto-injects `AGENTS.md` (its fork preamble points
  here) → read `STATUS.md` → the oldest todo task → work it.
- `tracker/` is fork-owned and is never included in an upstream PR.
