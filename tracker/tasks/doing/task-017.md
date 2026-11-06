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

## re-derived on the 0.12 base — 2026-11-06 (with task-024)

- **The branch is `pr-external-engine-012`** (local only): rebased onto
  `upstream/master` @ `f9b6971` (0.12.0 + the 6 zero-code post-release
  commits) — the 5 signed commits replayed as `ff372fc` (core) / `9255632`
  (bundles) / `595d76f` (service) / `fbcd114` (UI) / `e141d35` (tests+
  docs); the old `pr-external-engine-v2` name is retired. The re-
  derivation folded in two 0.12-semantics fixes (the `pup-qwen` bundle
  re-extracted against 0.12's built-in prompts; the cut-off VLM answer
  kept with a length-limit note, not discarded). Verified on the branch
  2026-11-06: qls-core 370/370, qls-service 18/18, vitest 228/228 (tsc
  clean), mcp 35 passed + 3 skipped, GPU-free harness 16/16.
- **Exclusions after the re-derivation:** the pre-v2 note above stands
  for `tracker/`, the AGENTS preamble, `docs/fresh-box.md` and the
  harness run-artifact dirs; the 10 support files' rewrapping is gone —
  every format-only file was proven by token-diff and taken wholesale
  from 0.12, so the diff against upstream master carries no formatter
  noise.
- **Decisions (owner, 2026-11-06):** (c) = **the discussion variant, not
  the PR** — posted on the author's repo only when the branch is in a
  mergeable-on-the-spot shape (it is, per the verification above); the
  expected outcome is rejection on the one-model vision, and the fork is
  the product either way (D-RB). (a) and (b) ride the post: pushing the
  branch to the public fork (required for "mergeable on the spot") makes
  the BFL bundle (`assets/pup/`, commit `9255632`) public for the first
  time — that is the owner's call; the discussion text itself carries no
  BFL bytes. (d) = parked until the go-ahead.
- **The draft is finalized in [`pr-upstream-body.md`](../pr-upstream-body.md)**
  (base/hashes/counts updated to the re-derived branch; the discussion
  variant now states the 0.12 base and names the branch).
- **Awaiting the owner's go-ahead** to: (1) push `pr-external-engine-012`
  to the public fork (with or without the BFL bundle commit), and (2)
  post the discussion on the author's repo (GitHub Discussions, linked
  from or in reply to #35). G8 closes when the decision is recorded.

## executed — 2026-11-06 (owner go-ahead "go with a" + "everything on the fork")

- **Pushed (all DCO-signed, no force on any new ref):**
  - `flux` → fork default branch, force-updated to `1ab6a2c` (proven
    first: the remote tip `71df3d9` and every commit it added were
    contained in the rebased local flux — no work overwritten).
  - `pr-external-engine-012` → **new** fork branch at `e141d35`, the
    complete proposal **with the BFL bundle** (owner's "go with a":
    "i dont want us to lose work … our fork becomes the source of
    truth"). The bundle is public on the fork from this commit on.
  - `pr-external-engine` → **new** fork branch at `7958672`: the old
    pre-v2 parked line (10 commits, never pushed before) — the last
    work not on the fork. The fork's other old branches
    (`external-engine` 15 commits; `master`, `macos-optimization`,
    `prompt-enhancer`, `release-0.9.4-docs`, `server-0.11`,
    `cla-signatures`, `mac-engine-0.1.1` = upstream mirrors, two of
    them byte-identical to upstream) already existed.
  - Fork audit: no stashes; untracked = one generated test log; every
    local branch is now on the fork. **Nothing worked on is outside
    the fork.**
- **The publish is deliberately NOT done yet (owner):** "the pr assume
  mergeable on the spot, but dont publish the pr or a discussion yet,
  thats the last item on our agenda and we need to hash that out
  together." The branch is in that shape (verified above); both the PR
  and the discussion post stay unmade until the joint decision.
- **The permissions wall (found while preparing the post):** the `iamskeole`
  account has **read-only** on the author's repo (`permissions`
  `pull: true, push/triage: false`), and GitHub's Discussions API is
  web-session-only (no `addDiscussion` on api.github.com, REST 404) —
  so I cannot create the discussion with this login, whatever the
  decision. The paths, for the joint call:
  1. The author grants `iamskeole` write (collaborator) on the repo →
     I retry the post via the web-session GraphQL/REST endpoint.
  2. The owner opens it in the browser with the prepared title + body
     (in `pr-upstream-body.md`), category **Ideas**
     (`DIC_kwDOVBkBuM4DHe3H`), linked from / in reply to #35.
  3. An **Issue** instead (any logged-in user may open issues there) —
     deviates from the "discussion variant" instruction; only if chosen.
- G8 stays open: it closes when the joint decision (channel, account,
  with-or-without a companion PR, and the BFL-bundle-already-public note
  in the body if relevant) is made and the post is up.
