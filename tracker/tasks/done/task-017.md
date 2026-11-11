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
  - **Fork tidied (same day, on the owner's "it's confusing" prompt):**
    the default branch renamed **`flux` → `main`** (the old name was a
    historical feature-branch name — a misnomer for the whole fork),
    and the public branch list shrank from **10 branches to 2**
    (`main` + `pr-external-engine-012`): the two pre-v2 lines preserved
    as annotated tags **`parked/pre-v2-pr`** (`7958672`) /
    **`parked/pre-v2-line`** (`d318018`) — visible in the tag list, out
    of the branch list, still on the fork; the 7 upstream-mirror
    branches deleted (six byte-identical to the author's own branches;
    the fork's stale `master` snapshot fully contained in `main`).
- **The joint call made (2026-11-06, same day):** the **discussion
  route**; the v2 body (the personal opening + the full production
  scope, in `pr-upstream-body.md`) **finalized as-is** (owner: "thats
  brilliant chief, wont change a thing"); the owner opens it in the
  browser, category **Ideas** (`DIC_kwDOVBkBuM4DHe3H`).
- **Sole precondition:** the posting account needs **write** on the
  author's repo — the `iamskeole` login is read-only there (see the
  permissions wall above), so either the author grants `iamskeole`
  collaborator write (one click: repo → Settings → Collaborators) or a
  different account posts the same text.
- **It is up — 2026-11-06 17:39 UTC, Discussion #52, category Ideas:**
  https://github.com/GabrieleLozupone/kokon/discussions/52 — posted from
  `iamskeole` via the web UI. The "wall" described above turned out to be
  **API-side only**: that repo allows any logged-in user to *start* a
  discussion (no collaborator grant was needed after all); the account
  still has no push on the author's repo. The body is verified verbatim
  = the final v2 (the corrected same-repo diff link included); 0 comments
  at verification time.
- **G8 closed; this file moves to `done/`.** What the author's reply
  decides: accepted ⇒ the fork re-baselines onto the merged upstream and
  shrinks to whatever diff remains (the mission: keep getting smaller);
  rejected on the one-model vision ⇒ the fork is the product (D-RB: clone
  → `docker build` → `docker compose up -d`), and this discussion is the
  paper trail either way. Nothing else on the queue blocks (task-023
  stays parked on the owner's trigger).

## The reply — 2026-11-10 (the branch the fork had planned for)

The author replied 2026-11-10 15:49 UTC:

- Thanks, and "asking before opening a 17,500-line PR was the right
  call." The discussion-first strategy is validated by him, not just by
  us.
- **The external-engine mode is not taken upstream.** Two reasons, in
  his words: (1) the one-model vision — "several model families, a
  separate prompt VLM and replaceable prompt bundles go the other way";
  (2) maintainability — "a change that size would also be more than I
  can keep tested release after release."
- **The kind line, quoted because it matters:** "Your fork is the right
  home for that setup, and it's good to see it exists for people with a
  box like yours." — an explicit author acknowledgment of the fork's
  existence.
- He points out the *access* half is already upstream: Settings →
  Other devices / `kokon-service --remote` headless / the Docker image
  — the full interface in a browser with a password, box stays headless.
- **Announced for the next release: idle unload** — Kokon will unload
  the model after a period of inactivity *even with a window or browser
  tab open*, and reload on the next request ("that should let it sit
  next to your other models on the same card without an external
  gateway in between").

**Outcome per the decision framework — the rejection branch, as
planned:** the fork is the product (D-RB: clone → `docker build` →
`docker compose up -d`); the fork keeps the `kokon` name (owner). There
is no merged upstream to re-baseline onto; the `pr-external-engine-012`
branch stays on the fork as the record of how the feature was built
(the fork's `main` carries the same feature in fork form — nothing
dies). The discussion (with his reply) is the permanent paper trail.

**Two watch items for the next rebase (his next release = idle-unload):**
1. **Idle unload vs the gateway:** his change governs the *in-engine*
   path. In external mode the model's lifetime is the gateway's job
   (v269's 120 s TTL) — the service must not start managing a model it
   doesn't own. Before adopting his release, check the service's idle
   timer against the gateway's lifecycle in external mode (expect: the
   service-side timer simply doesn't apply; verify it can't double-unload
   or fight the gateway).
2. **"Other devices" / `--remote`:** compare his remote story with our
   reverse-proxy remote-login work on the fork; keep whichever is the
   better mechanism, note the overlap.
