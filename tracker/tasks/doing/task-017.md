# task-017 — upstream PR / merge / philosophy (the last task)

## goal
Re-baseline the parked `pr-external-engine` branch (10 DCO commits,
never pushed) against upstream + the v2 work. The owner then decides
what we contribute upstream (the unbundling, external-engine mode, the
picker, possibly the Qwen3.5 engine support) vs what stays ours
(`tracker/`, the AGENTS fork preamble, the host layout). The
philosophy conversation (fork vs upstream PR — why it is shaped the way
it is) happens here, at the end, with the whole picture in hand.

## acceptance criteria
- The PR branch rebuilt: upstream + contributed files only (no
  `tracker/`, no fork preamble); proposal/discussion text drafted; the
  branch pushed or parked per the owner's call.
- If anything merges upstream: this repo re-baselines onto it, and the
  fork shrinks to whatever diff remains (the mission is to keep getting
  smaller).
- The decision recorded here and in `STATUS.md` (open item G8 closes).

## blockers
- task-016 (the final state first — you describe what works before you
  propose it).
- user approval required, never autonomous.

## decisions
- (deferred to the owner — their explicit "when we're done" item)

## prepared — 2026-11-04 (autonomy span; nothing pushed, nothing submitted)

- **The branch is `pr-external-engine-v2`** (local only, on this clone):
  base = `upstream/master` at `200f3d5` (fetched fresh; 3 new upstream
  commits, all `engine-mac/` M5 work — zero file overlap with us) +
  **5 DCO commits** that are the external-engine feature in its final v2
  form (the parked branch's 10 commits were the pre-v2 schema-12 design
  and are superseded, not rebased):
  1. `a1e4c6f` the core crate — schema-13 config surface (per-pipeline
     gateway / VLM / promptBundle), the gateway engine client
     (`/upstream/<model>/sdcpp/v1/…`, adopted-never-owned), the
     service-side VLM client (OpenAI chat + image parts; down VLM
     hard-fails, D-FB), the runtime two-family bundle loader, and the
     planner/pipeline/gallery/imaging changes those rest on
  2. `9a12945` the prompt bundles (`assets/pup/` BFL Apache-2.0 with
     LICENSE + hashes; `assets/pup-qwen/` Kokon's own Qwen set)
  3. `7a9ad05` the service API (status/switch/jobs over the gateway,
     `/models/switch`, per-image model name, reverse-proxy login,
     security tests)
  4. `d775a94` the UI (model picker, status pill, settings card, plus
     the v2 layout-box editor/overlays the klein table contract rides on)
  5. `8469b67` the GPU-free mock-gateway integration test (16
     assertions) + the engineering-notes section
- **Verified on the branch**: `cargo test -p qls-core` 327/327,
  `-p qls-service` 17/17, `npm test` 160/160 (host note: local builds
  need `OPENSSL_DIR=$(brew --prefix openssl@3)` — in OPS.md).
- **Excluded by design** (the AC's "no tracker/, no fork preamble"):
  `tracker/`, the AGENTS fork preamble, `docs/fresh-box.md` (fork host
  layout), the test harness's run-artifact dirs (`tests/external-engine/
  config/` + `data/` — host-specific paths incl. a `kokon-fork`
  reference; the harness regenerates its own), and
  `examples/inpaint_preview.rs` (provably format-only vs upstream —
  kept upstream's). The 10 other support files (lora/lut/hdr/heif/
  catalog/store/geometry/wic/speed/imaging) carry real changes plus
  rustfmt-style rewrapping that can't be split without running a
  formatter (banned on this host); the maintainer's fmt pass absorbs it.
- **The full draft lives in [`pr-upstream-body.md`](../pr-upstream-body.md)**
  (PR body + discussion variant + the four decision points), per the
  owner's directive: everything prepared, **nothing pushed or submitted**
  — the branch is local, and note that pushing it anywhere (even the
  public fork) would first make the BFL bundle public, which has never
  been pushed anywhere to date.
- **Owner decision points** (see the draft): (a) BFL bundle in/out of
  the PR; (b) box-editor entanglement in/out; (c) discussion-first vs
  direct PR; (d) when to push/submit.
- G8 stays open until that call; the task file moves to `done/` with the
  recorded decision.
