# task-017 draft — the upstream contribution (NOT submitted; owner review required)

Status: **everything prepared, nothing pushed, nothing submitted.** The
branch `pr-external-engine-v2` exists only in this clone. Submitting it —
or even pushing it to our public fork — makes the BFL prompt bundle
(commit 2) public for the first time; that is decision point (a) below and
happens only on your word.

Base: `upstream/master` @ `200f3d5` (fetched 2026-11-04; its 3 new commits
are all `engine-mac/` M5 work, zero overlap). Contents: 5 DCO commits
(a1e4c6f core / 9a12945 bundles / 7a9ad05 service / d775a94 UI /
8469b67 tests+docs) = the external-engine feature in its final v2 form.
Verified on the branch: qls-core 327/327, qls-service 17/17, vitest
160/160, plus the GPU-free mock-gateway harness (16 assertions, loopback).

## Decision points (yours)

**(a) The BFL bundle in or out?** Commit 2 carries
`crates/qls-core/assets/pup/` — the klein-pup upsampler's prompt assets
(Apache-2.0; source + per-file hashes in its LICENSE.md; the code and
tests reference keys/hashes only, never the text). It has never been
pushed anywhere, including our public fork. In it: the upstream klein
pipeline gets the full sidecar-PUP path out of the box. Out (drop the
commit): the BFL bytes stay on our host only, and the upstream klein
pipeline runs its in-engine conditioner instead (its pre-v2 behavior).
*My read: in — it's licensed, hashed, self-describing, and without it the
"prompts as data" half of the feature doesn't exist upstream. But it's
your relationship with that maker, so it's yours to call.*

**(b) The box editor in or out?** Commit 4's UI commit carries the
layout-box editor/overlays alongside the external-engine picker/status,
because they share files (Composer/Stage/logic) and the klein pipeline's
prompt-embedded box table is the feature the editor feeds. Splitting them
means per-hunk surgery across 6 files with no formatter allowed on this
host. *My read: keep — it's what makes the klein contract end-to-end, and
it's tested (160 vitest assertions incl. the table round-trip).*

**(c) Discussion first, or straight PR?** The repo's own rule (AGENTS)
routes *model* changes through Discussion #35; this is architecture, not
a model swap, but the maintainer may still want the conversation before a
~18k-line diff lands. *My read: post the discussion (text below) and link
the branch; it costs a day and buys the philosophy conversation the task
file wants at the end.*

**(d) Push/submit when?** The parked-branch instruction says "pushed or
parked per the owner's call." Default here: parked in this clone until
you say otherwise.

## Draft PR body

> **Title:** External engine mode: Kokon as the thin layer over your own
> model gateway
>
> **Body:**
>
> Kokon today spawns and manages its own engine process. This PR adds a
> second way to run: point the service at an on-demand model gateway
> (any OpenAI-shape proxy that loads models on request — the
> llama-swap `/upstream/<model>` proxy is the reference deployment) and
> Kokon stops being an engine owner. The service *adopts* whatever the
> gateway loads: it never starts, stops, downloads or upgrades the
> model, and "no model loaded" is its resting state, not an error.
>
> What it changes
>
> - **Config (schema 13).** `externalEngine` gains a gateway URL and the
>   new `models` list (label + per-pipeline gateway/VLM/prompt-bundle
>   overrides). Everything is pointable; nothing is baked in. Old configs
>   read unchanged.
> - **The engine client** (`sdcpp.rs`) gains the gateway path:
>   `/upstream/<model>/sdcpp/v1/…` — the same async job API, one extra
>   path segment. Capability, job and cancel calls all travel the prefix.
> - **The VLM moves to the service** (`vlm.rs`). Prompt enhancement is an
>   OpenAI chat call with image parts, per pipeline, to whatever
>   `vlm` endpoint the config names. A down VLM **hard-fails the job**
>   with an actionable error instead of silently sending a raw prompt —
>   the enhancer is part of the quality contract.
> - **Prompt bundles as data** (`pup.rs`, `assets/`). A pipeline's VLM
>   prompts load from a named bundle folder at runtime; a dropped-in
>   replacement applies to the next job. Two families: the bundled
>   klein-pup upsampler set (Apache-2.0, source + file hashes in
>   `assets/pup/LICENSE.md`, referenced by keys only in code and tests)
>   and Kokon's own Qwen set (missing → built-in defaults).
> - **The picker** lists the config's models, not flavors; switching
>   (`POST /models/switch`) changes config only — the next job triggers
>   the gateway's swap. The status pill is an indicator, not a loader:
>   visiting the UI preloads nothing.
> - **The v2 UI** the mode ships on: the layout-box editor/overlays the
>   klein pipeline reads as a prompt-embedded table (the box
>   keep/move/new/remove contract), plus the settings card that tells
>   the user the engine is managed outside Kokon.
> - **A GPU-free integration test** (`tests/external-engine/`): a mock of
>   the gateway's proxy shape drives a real service through rest / load /
>   job / switch / evict-swap / gateway-down / recovery — 16 assertions,
>   all loopback, no GPU, generates its own config.
>
> What it deliberately doesn't do: no bundled engine, no model
> downloads, no host deployment layout (that's the deploying machine's
> business), no Mac changes. The in-engine path (Kokon-owned sd-server)
> remains the default and is untouched; this mode is opt-in per config.
>
> Tests: `cargo test -p qls-core` (327), `-p qls-service` (17),
> `npm test` (160), and the mock-gateway harness above, all green on the
> branch. A few long-lived files carry incidental reformatting from a
> rustfmt pass during development — a maintainer fmt pass will absorb
> it; I've kept the diff to the files the feature touches.

## Discussion variant (if (c) = discussion-first)

> **Title:** Proposal: run Kokon against your own model gateway
> (external-engine mode)
>
> We've been running a configuration where Kokon doesn't own the engine
> at all: the service points at a small local gateway that loads models
> on demand (one GPU, several models, evict-on-demand), and Kokon
> adopts whatever it finds. It's been in production on our box since 11-02 across two model families, and the shape of the change in the
> repo is smaller than it sounds: a config surface (per-pipeline
> gateway/VLM/prompt-bundle), an extra path segment in the engine
> client, the prompt-enhancement call moved into the service as a plain
> chat API client, and the prompts becoming replaceable data. The
> in-engine mode is untouched and stays the default. Before we open a
> PR we'd like the conversation this repo asks for: is "Kokon as a thin
> layer over your own gateway" a direction you'd accept upstream, what
> would you want different (naming, config shape, the VLM hard-fail
> policy, the bundle layout), and is there overlap with anything you're
> planning on the server side? Our working branch and a full diff are
> available on request.
