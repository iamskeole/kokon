# task-017 draft — the upstream contribution (NOT submitted; owner review required)

Status: **everything prepared, nothing pushed, nothing submitted.** The
branch `pr-external-engine-012` exists only in this clone (re-derived onto
the 0.12 base 2026-11-06; the old `pr-external-engine-v2` name is retired).
Submitting it — or even pushing it to our public fork — makes the BFL
prompt bundle (commit 2) public for the first time; that is decision
point (a) below and happens only on your word.

Base: `upstream/master` @ `f9b6971` (0.12.0 + the 6 zero-code
post-release commits). Contents: 5 DCO commits (ff372fc core / 9255632
bundles / 595d76f service / fbcd114 UI / e141d35 tests+docs) = the
external-engine feature in final form, on 0.12. The re-derivation folded
in two 0.12-semantics fixes (the `pup-qwen` bundle re-extracted against
0.12's built-in prompts; the cut-off VLM answer kept with a length-limit
note, not discarded). Verified on the branch 2026-11-06: qls-core
370/370, qls-service 18/18, vitest 228/228 (tsc clean), mcp 35 passed +
3 skipped, plus the GPU-free mock-gateway harness (16 assertions,
loopback).

## Decision points (yours)

**(a) The BFL bundle in or out?** Commit 2 (`9255632`, the bundles) carries
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
> Tests: `cargo test -p qls-core` (370), `-p qls-service` (18),
> `npm test` (228), the MCP protocol suite (35), and the mock-gateway
> harness above, all green on the branch; no reformatting of upstream
> files — the diff is limited to the files the feature touches.

## Discussion variant (if (c) = discussion-first)

v2, 2026-11-06 (owner direction: more personal + full scope): the personal
opening (the wife / headless-box story), the production scope (both image
families by name, the one shared 9B PUP serving both, the co-resident
text-chat pair, the single-keyed-port gateway as our own deployment), the
vision acknowledgment, and the measured diff size (61 files, +15,416 /
-2,060 against `f9b6971` = ~17,500 changed lines). Deliberately not
included: the gateway internals (v269 matrix, the port map — our
deployment, not his repo) and MOSS (parked research, not built).

> **Title:** Proposal: run Kokon against your own model gateway
> (external-engine mode)
>
> First, a personal word: Kokon is a masterclass in UI, and a brilliant
> piece of engineering — in that order. The moment I found it I knew it
> would solve my problem. Happy wife, happy life: the only thing my wife
> hated me showing her more than Excel pivot tables was ComfyUI nodes. She
> needs image generation and editing in her workflows, and until Kokon,
> everything was a no for her.
>
> The complication in our setup: we have exactly one machine powerful
> enough to run models — a headless Linux box with a 3090 — and it is not
> where she works. So the question became: how do I run Kokon where the
> GPU lives, and let her use it from her own machine? This branch is that
> answer: Kokon, unbundled from its engine.
>
> **What it is.** The service can point at a small local gateway that
> loads models on demand (one GPU, several models, evict-on-demand — the
> llama-swap `/upstream/<model>` proxy is the reference deployment), and
> Kokon stops being an engine owner: it adopts whatever the gateway loads
> and never starts, stops or downloads a model. "No model loaded" is its
> resting state, not an error.
>
> **What we have been running with it.** In production on that box since
> 11-02: two image-model families — the Qwen-Image 2.1 line (Turbo and
> full) and the BFL FLUX.3 Klein line — plus one shared 9B
> prompt-enhancement VLM that serves both families from the same 24 GB
> card, with a text-chat model pair co-resident and evicting cleanly. (The
> gateway itself is our own deployment, out of scope for the repo: it
> runs that whole set on the one card, evict-on-demand, behind a single
> keyed port.) She uses it from her own machine; the box stays headless. I
> suspect a number of users have a similar setup — one capable box,
> several machines to use it from — and would benefit from the same
> unbundling of UI and engine management.
>
> **The shape of the change in the repo** is smaller than it sounds: a
> config surface (per-pipeline gateway / VLM / prompt-bundle; old configs
> read unchanged), one extra path segment in the engine client, the
> prompt-enhancement call moved into the service as a plain chat-API
> client, and the prompts becoming replaceable data. The in-engine mode —
> the default you ship — is untouched.
>
> **Why a discussion first.** I understand and respect the product vision
> this repo is built on — one carefully chosen model, nothing to tune —
> and I'd rather have this conversation than throw a ~17,500-line diff at
> you and hope for the best. So: is "Kokon as a thin layer over your own
> gateway" a direction you'd accept upstream, what would you want
> different (naming, config shape, the VLM hard-fail policy, the bundle
> layout), and is there overlap with anything you're planning on the
> server side?
>
> It is built on 0.12 as released, so it is ready to review as-is:
>
> - Branch: https://github.com/iamskeole/kokon/tree/pr-external-engine-012
> - Diff against `upstream/master` @ `f9b6971`: https://github.com/GabrieleLozupone/kokon/compare/f9b6971...iamskeole:kokon@pr-external-engine-012
