# AGENTS.md

**Fork mission (iamskeole fork — read before anything else).** This fork
unbundles Kokon from shipping built-in engines: the service in this repo is
the only custom layer; every inference engine is **stock upstream**
(stable-diffusion.cpp, llama.cpp — the host's `backends/` named slots)
scheduled by the host's gateway (llama-swap); the service points only at
**entrypoints, API keys, and prompts**. Where this file (upstream text below)
and the fork direction disagree, **the fork direction wins** — notably,
"Kokon's own build of sd-server" in the architecture below is the upstream
bundled design; here engines are external and stock. The rest of this file
(build/test, UI conventions, contracts, the model-change quality policy)
stays in force for our work.

**Working state.** The fork's plan is [tracker/PLAN.md](tracker/PLAN.md); what
is in progress on this fork is [tracker/STATUS.md](tracker/STATUS.md), which
supersedes `docs/STATUS.md` here. **The current task is always the oldest
file in [`tracker/tasks/todo/`](tracker/tasks/todo/)** — when asked to "pick
the next task": move that file to `tasks/doing/`, work it to its
acceptance criteria, move it to `tasks/done/`, and update `STATUS.md`.
Each task carries `## blockers` and `## decisions` — surface blockers
before guessing, and record new decisions in the task file. `tracker/`
is fork-owned and is never contributed upstream.

**Commit discipline (this host, this repo).** DCO-signed commits
(`git commit -s` — no GPG key here); **never run `cargo fmt`** (no
rustfmt.toml on this host — hand-format only); one small commit per
logical unit; tracker changes (task file moved, `STATUS.md` line)
committed with the work they describe; BFL prompt/template strings never
appear in chat-visible diffs, logs, or repo docs — they live in assets
and are referenced, not inlined.

**Autonomy protocol (long / GPU jobs — engaged when the owner kicks one
off; the full version lives in the host's OPS notes, `/home/p/ml/OPS.md`).**
One 24GB GPU is shared with the operator runtime (vllm); evicting it kills
it, and the revival is the reload. A long GPU job:
1. is a self-contained **POSIX-dash** job script (no arrays or `[[` — dash
   parses incrementally, so a bash array makes it run the clean prefix and
   then die mid-script with no marker; check with `sh -n`, not just
   `bash -n`): frees the GPU, starts the model under
   test, waits on a **1-token `/v1/completions` probe** for true
   readiness (200 = ready, 503 = still loading — `/health` lies), runs
   the tests, writes a digest, stops the model, **revives vllm
   canonically** (its `serve.sh start --no-wait` — never a gateway
   on-demand curl, that trigger drops), **and writes the `DONE`/`FAIL`
   marker last** (the marker means "the runtime is back");
2. is launched with `setsid sh /path/job.sh < /dev/null > /path/job.log
   2>&1 &` (a surviving host process — never `setsid sh -c '…'`);
3. gets a second setsid watcher as backstop (hard cap ≤3h; fallback
   digest + revival if the job dies);
4. ends the agent's turn in a **blocking wait that is the last step of the
   LAUNCH tool call itself** — the same bash invocation that setsid-launches
   the job and the watcher then runs
   `sh /home/p/ml/wait-marker.sh /tmp/<jobdir> <cap>`; bash tool timeout =
   cap + 120 s, where **cap exceeds the job's worst case (the watcher cap
   + revival margin), not its expected runtime** — a timeout firing while
   vllm is down kills the session for good, and a turn ended with no pending
   wait never resumes (both observed 2026-11-03; four consecutive windows
   that ended their launch turn before the wait, the last leaving the owner
   mid-crisis — a launch not immediately followed by the wait in the same
   call is a process failure, not a judgment call). While it blocks the
   agent is suspended, not polling; the
   harness's result injection — marker + digest + log tails + live state
   in one payload — is the nudge that continues the work without the owner;
5. tolerates the agent dying mid-job (the job and the watcher are host
   processes; the revival revives the runtime regardless);
6. neutralizes in the gateway registry any model the window must keep down
   (config swap), or vets it in a loop: the daemon brings dead registry
   models back by re-running their cmd (three vLLM resurrections inside the
   2026-11-03 pair window; the same day's no-registry window was clean),
   and a gateway unload cannot stop a model started outside the daemon.
   **When removal is in effect the job's defense is a YIELD, never a
   kill-veto**: with vllm out of the registry the daemon cannot bring it
   back, so any vllm that appears mid-window is a human reclaiming the GPU
   — the job dies cleanly and the restore leaves a healthy vllm alone.
   Killing a human's deliberate boots is the protocol's worst failure mode
   (2026-11-03 qwabl: three manual boots killed in a row, 20:52-20:55, ~21
   min of owner chat lost). The owner's sessions stay on qwen-direct for the
   window's duration (verified 2026-11-03: qwen-direct is truly direct,
   18020/v1 — it cannot trigger loads, but its turns fail while the model
   is down).
It exists to kill the proven failure modes: poll loops, "background it
and hope the owner asks", false-DONE on `/health`, crashed jobs
stranding the GPU, a job that dies on a parse error before any marker
(interpreter/dialect mismatch — the watcher's silence fallback recovers
the box; kleenq-fail1, 2026-11-03), a script created without its exec bit
(the daemon's fork/exec fails `permission denied` at info level while the
job's waits just 500 — preflights exec-audit every cmd/cmdStop script;
2026-11-03 qwabl, 13 min), premature markers, the job killing the
owner's manual boots (yield rule, item 6), and a turn that ends idle after
a launch (the wait is part of the launch call, item 4).

The entry point for anyone working on Kokon: people and coding agents (Claude Code, Codex, Cursor and
others). Read this first, then the fork working state above for what is in progress on this fork.

## What Kokon is

Kokon is the simplest local photo editor: **edit photos by describing the change**. One installer sets up an
open image model on the user's own GPU (Windows, macOS, Linux), and the user brushes an area (or not) and
types what they want.

- **We pick the best open model, the user picks nothing.** Today that is Qwen-Image 2.1 Turbo (8 steps, CFG 1)
  with three quality presets. No settings to tune: sizes, memory and presets are decided by the code.
- **Expert controls are opt-in only**, in Settings, off by default, never next to the prompt box.
- **A request fits if it helps someone who has never used ComfyUI.** Power-user options go to the bottom of the list.
- **Changing the default model** needs a model that beats the current one overall on text-to-image, edit,
  multi-image edit and memory use, on the same test set; the old model stays selectable, with a rollback.
  Proposals: [Discussion #35](https://github.com/GabrieleLozupone/kokon/discussions/35).

## Architecture in one minute

```
Kokon app (Tauri 2 + React, src-tauri/ + src/)    kokon-mcp (mcp/, stdio)    scripts
                    └──── loopback HTTP + token ────┬────────────────────────┘
                                                    ▼
             kokon-service (crates/qls-service): stable API on 127.0.0.1, job queue, gallery
                         all logic in crates/qls-core (planner, pipeline, imaging, catalog…)
                                                    │  /sdcpp/v1/*
                                                    ▼
             sd-server (stable-diffusion.cpp, Kokon's own build) on 127.0.0.1
```

- `crates/qls-core`: all the logic. `sdcpp.rs` is the only file that knows sd-server's request format;
  `catalog.rs` pins every download (URL, size, SHA-256); `planner.rs` decides sizes and memory.
- `crates/qls-service`: thin HTTP service. `src-tauri/`: thin desktop shell. `src/`: React UI, logic in
  `src/lib/logic.ts`. `mcp/`: MCP server (TypeScript, official SDK).
- Engine: Kokon's build of stable-diffusion.cpp from the fork `GabrieleLozupone/stable-diffusion.cpp`
  (branch `kokon`); `engine.json` on master ships engine updates without an app release.
- `site/`: kokon.app (static, deploys on push to master). `marketing/`: Remotion videos, showcase images,
  release notes.

Module map and the measured facts behind each default: [docs/engineering-notes.md](docs/engineering-notes.md).

Working on the Mac engine (`engine-mac/`): read [engine-mac/AGENTS.md](engine-mac/AGENTS.md) first: optimize for
your own Mac without changing other chips, how to measure, and the report every engine PR carries.

## Build and test

Setup, prerequisites and the full commands are in [CONTRIBUTING.md](CONTRIBUTING.md). In short:

| | Run in development | Release build |
|---|---|---|
| Windows | `npm install`, `npx tauri dev` | `scripts\build-release.ps1` |
| macOS (Apple Silicon, 15+) | `bash scripts/dev/mac-dev.sh` | `bash scripts/build-release-mac.sh` |
| Linux (Ubuntu 22.04 / WSL, rustup cargo) | see [docs/linux.md](docs/linux.md) | `bash scripts/build-release-linux.sh` |

Tests, before every commit that touches code:

```bash
cargo test -p qls-core
npm test                 # UI logic (vitest)
cd mcp && npm test       # MCP server (the protocol test needs a built kokon-service)
```

Most changes (planner, geometry, UI logic, MCP) are testable without a GPU. Anything that runs the model for
minutes (real inference, benchmarks, release builds) is run by the maintainer: hand over the exact command.

## Conventions

- Logic in `qls-core` (or `src/lib/logic.ts`), where it can be unit-tested; service, shell and MCP stay thin.
- **Measured defaults carry their source.** If you change a size, memory budget or backend flag, say in the
  comment what you measured it on (GPU, OS, bench). Comments explain *why*.
- Add or update a test for behaviour you change. Tests never touch real user folders.
- UI text and docs: plain, short English. Follow the style of the file you edit (`cargo fmt`, strict TypeScript).
- Methods must generalize: no constants tuned on one image; every step falls back safely when it doesn't apply.
- Don't commit model files or large binaries. Code is AGPL-3.0-only; contributors sign the [CLA](CLA.md).

## Where the docs live

| Doc | What |
|---|---|
| [README.md](README.md) | the product for users: install, requirements, FAQ, short roadmap |
| [docs/STATUS.md](docs/STATUS.md) | **what is in progress, next, known issues, help wanted** |
| [CONTRIBUTING.md](CONTRIBUTING.md) | build, tests, code style, licences |
| [docs/engineering-notes.md](docs/engineering-notes.md) | architecture, module map, verified backend facts, engine updates |
| [docs/platforms.md](docs/platforms.md), [docs/linux.md](docs/linux.md) | per-platform notes; Linux plan and the 0.11 server design |
| [docs/releasing.md](docs/releasing.md) | publishing a version (maintainer) |
| [docs/prompting.md](docs/prompting.md), [docs/showcase-prompts.md](docs/showcase-prompts.md) | prompt guide and examples |
| [marketing/](marketing/) | videos, showcase images, release notes per version |

## Keeping docs/STATUS.md current

At the end of every task that changes what users get or what is planned:

1. Move what you finished from "In progress" to "Recently shipped" (or "Done on master, not released yet"),
   with the PR or issue link, and change the "Updated" date at the top.
2. Add new known issues or help-wanted items with their issue link; remove the ones that are closed.
3. Check the items against GitHub (`gh issue list`, `gh pr list`, releases) rather than memory: STATUS must
   never claim something the repo doesn't show.
4. Keep the README roadmap short and in step with it.

`node scripts/dev/agent-context.mjs` prints the STATUS pages; agents can run it at session start (Claude Code:
a SessionStart hook in `.claude/settings.local.json`).

If a `private/` folder exists, read `private/AGENTS.md` next (maintainer only).
