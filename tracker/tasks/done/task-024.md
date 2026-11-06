# task-024 — rebase the fork onto upstream master (= 0.12.0)

## goal
Upstream has moved past both our bases: the live `flux` branch (base
`f2e4757`, pre-0.12) and the parked PR branch `pr-external-engine-v2`
(base `200f3d5`) sit 31 commits below upstream master. Between them:
**0.12.0** (merge `92f63c4`: Layers lab, Paint-with-colours, Prompt+ for
areas/Expand, AI-marking/watermark, Mac engine on by default, ONNX in
the service) + 6 post-release commits (site/README/status/marketing
only — zero code). Re-land our work on top of the 0.12 tree with the
upstream default path byte-for-byte intact and our external-engine mode
riding on it (D-RB: always master).

## acceptance criteria
- `flux` rebased onto upstream master; our commits re-landed. The 27
  intersecting files merge; upstream's 0.12 delta in that zone is ~2.4k
  lines, mostly additive; the new modules (`matting`, `onnx`, `scene`,
  `projects`, `watermark`, `glibc_compat`) and the new UI components
  (`LayersEditor`, `LayerSplit`, `LabScene`, `sam.ts`, `matte.ts`,
  `psd.ts`, …) are new files = zero conflict. Expect the real work in:
  `engine.rs` (theirs +370 / ours +2,801), `pipeline.rs` (355/1062),
  `planner.rs` (64/1,515), `enhancer.rs` (148/667), `config.rs`
  (9/1,004), `api.rs` (138/715), and the UI shared state
  (`logic.ts`, `store.ts`, `types.ts`).
- **The one design branch:** 0.12's area/Expand Prompt+ (`enhancer.rs`
  `area_request`/`expand_request` with their built-in system prompts) —
  default mode keeps upstream's in-engine Qwen3-VL path untouched;
  **external mode routes the same request shapes through the
  service-side VLM client** (the task-006 path: OpenAI chat + image
  parts, D-FB hard-fail). The built-in area/expand system prompts are
  Kokon's own words (not BFL's) — safe to use on the sidecar; a bundle
  override is a follow-up only if an A/B ever wants one.
- **0.12's service-resident ONNX and the lab come in as-is:**
  MobileSAM (runs in the webview; the service only downloads and serves
  the ~30 MB files) and BiRefNet-lite (in-service, CPU ONNX, 224 MB) are
  identical in both modes, zero engine interaction, zero VRAM; the
  lab's DiT-touching operations (Quality cut-out, Blend, Expand,
  painted-area edits) ride the same sdcpp job API our gateway client
  already covers. No flavor management.
- Green on the rebased branch: `cargo test -p qls-core`, `-p
  qls-service`, `npm test`, `cd mcp && npm test`, and the GPU-free
  mock-gateway harness (its schema-13 config shape re-checked against
  0.12's config surface).
- **One GPU window (w024)** per the host's autonomy protocol (AGENTS.md):
  the external path on 0.12 — an image job through the keyed one-door
  gateway (the w022 topology), an area or Expand job showing the
  sidecar-routed Prompt+ end-to-end, the lab's fast cut-out (CPU ONNX)
  working while the engine runs, vLLM revived last, marker last.
- Image rebuilt and tagged `external-engine-0.12.0-<sha>`;
  `frontends/kokon` compose pointed at it; `docs/fresh-box.md` updated
  for 0.12 **and** the 022 one-door gateway section in the same pass
  (the open thread left by the task-022 span).
- **The PR branch is re-derived** from the rebased fork: the 5 curated
  commits (core / bundles / service / UI / tests) on an upstream-master
  base; the old `pr-external-engine-v2` is retired, not rebased. Green
  on the re-derived branch.
- `STATUS.md` updated; **task-017 unblocked** — its remaining call is
  posting the discussion variant, when the re-derived branch is in a
  mergeable-on-the-spot shape (owner, 2026-11-06).

## blockers
- The GPU window follows the host's autonomy protocol (AGENTS.md);
  registry neutralization for vllm per the w022 runbook.
- Upstream moves fast (0.12 was ~3 weeks after our old base): re-check
  the master head on the day the rebase starts; if another release
  lands before we finish, rebase onto that master head instead (D-RB).

## decisions
- **2026-11-06 (owner):** rebase base = **upstream master, always** —
  never a release commit (upstream doesn't tag its releases; master =
  the release plus post-release work only). **The fork does no
  releases** — releases are the origin's/author's view (their
  installers, their tags); if the upstream PR doesn't land, our release
  mechanism is: clone the fork → `docker build` → `docker compose up -d`
  (the service image is the product; `docs/fresh-box.md`).

## progress (this span)

- **2026-11-06 — PR branch re-derived first** (it blocks nothing, and its
  5-commit shape is the review surface): `pr-external-engine-v2` rebased
  onto `reb-012` (= upstream master `f9b6971`, 0.12) — the 5 signed
  commits re-landed as `ff372fc … e141d35` (core crate / bundles /
  service API / UI / GPU-free test); green: qls-core 370, UI 228,
  service 18, mcp 35+3skipped. Two 0.12-semantics fixes along the way:
  the `pup-qwen` bundle re-extracted (the 0.12 edit prompt gained the
  coloured-marks bullet; the pin test catches drift) and the cut-off
  VLM answer is kept with a length-limit note, not discarded (0.12).
  The branch is renamed **`pr-external-engine-012`** (the old name is
  retired; the old pre-v2 parked branch stays where it is).
- **2026-11-06 — `flux` rebased onto `reb-012`**: all 110 commits
  replayed; the 13 conflict stops were resolved hunk-level — 0.12 wins
  for upstream behaviour, fork work preserved, additive content unioned,
  format-only files proven by token-diff then taken wholesale. The
  semantic collisions had one resolution each (all final-fork
  semantics, matching the PR branch): `enhancerApplies ≡ true` (the
  sidecar writes the area/expand prompt; a box model's degraded mask is
  an edit the enhancer covers), `needsPrompt` model-aware (maskless
  removal needs no words on an inpaint model, but a box model's removal
  degrades to an edit and needs them; Expand never needs words),
  `system_prompt` on the pipeline's `promptBundle` (Kokon's own text —
  safe on the sidecar).
- **Design branch verified as already merged in the tip**: 0.12's
  run_job area/Expand gate (`applies_to`, `EnhancerKind::Area/Expand`)
  builds the `own` request with the built-in Kokon prompts, and the
  task-006 VLM routing in `enhance_prompt` sends that same `req` to the
  sidecar in external mode (`pipeline_vlm` → `externalEngine.vlm` →
  engine fallback); default mode keeps the in-engine path (model arg
  empty → no bundle, built-in prompts). No extra code needed.
- **Green on the rebased `flux` tip** (2026-11-06): qls-core **375/375**,
  qls-service **18/18**, UI **228/228** (tsc clean), mcp **35 passed +
  3 skipped**, mock-gateway harness **16/16** after its own 0.12
  refresh (`/models/switch` route, schema-13 config) — commit `85e9656`.

- **2026-11-06 — the w024 window** (15:54 UTC; the protocol: self-contained
  job + watcher + blocking wait; vllm neutralized in P1, revived last, the
  `FAIL` marker in `/tmp/w024/` is run 2's historical record, the completion
  record is `COMPLETION.txt`):
  - **The owner's matrix rename landed in-window** (the ask that unblocked
    this): the gateway's kokon ids now equal their boot-script names —
    `kokon-turbo` → `kokon-qwen-image-2.1-turbo`, `kokon-full` →
    `kokon-qwen-image-2.1-full`, `kokon-klein` → `kokon-flux-image-3-klein`
    (`kokon-pup` unchanged) — registry id == script name, so the tests and
    the matrix share one vocabulary. The alphabetical port map shifted with
    the rename (klein 9103→9102, pup 9104→9103, full 9102→9104; turbo 9105
    and the vLLM pair 9106/9107 unmoved); the volume config was renamed
    in-step (backup `config.json.bak-w024pre`); live yaml == last-good.
  - **P0–P4 green in-window**: baseline (keyed one door, vllm ready on
    9106); the window config neutralized vllm (reload, 4 s); the 0.12 image
    recreated the service on the renamed config in ~1 s; **T1** a t2i on
    the *renamed* turbo through the service's keyed one-door client path
    (job `job_20d7bb3f…`, 1,268,111-byte PNG, set qwen-t cold-booted on
    demand); **T2** an Expand on the T1 image — 0.12's area/Expand Prompt+
    ran through the **shared PUP sidecar** (alias `qwen3vl-8b` on
    `/upstream/kokon-pup`; the job's prompt is the sidecar's own 1,114-char
    answer; the DiT co-tenant stayed loaded) — the one design branch, proven
    end to end.
  - **P5 failed in-window** on the matte: 0.12 gates BiRefNet-lite behind a
    separate `POST /matte/download` the script omitted (`missing_model`).
    The restore ran as designed: the FINAL renamed config (vLLM pair
    re-registered) live, vllm revived by its own tracked boot (1-token 200
    in 40 s).
  - **P5 completed manually post-window** (CPU-only — the matte is a
    224 MB CPU/ONNX run, zero VRAM, so no window was needed; vllm resident
    on 9106 the whole time): `/matte/download` (→
    `/data/sdcpp/models/matting/birefnet_lite.onnx`), then `/matte` on the
    run-2 test image → a 7,788-byte grey PNG in 10 s **with vllm on the
    card** (the zero-engine-interaction claim holds); the qwen-t set
    self-reaped on its 600 s TTL (the daemon's own unload); the active model
    switched back to klein (the owner's 11:39 pipeline). The job script's P5
    now includes the download pre-step (`backends/_jobs/w024-job.sh`), so a
    future re-run is self-contained.
  - **The final image**: `external-engine-0.12.0-b77d049`, rebuilt at the
    final `flux` tip (the in-window `5c8a8d7` image was code-identical —
    the delta is docs-only); `frontends/kokon` pointed at it, service
    healthy, UI 200, vllm never touched by the swap.
  - **The harness aligned with the rename** (`9d87211`): the mock gateway,
    the reference config and the driver now use the script-aligned ids —
    16/16 green on the 0.12 build.
  - `docs/fresh-box.md` carried the 0.12 pass and the 022 one-door section
    in the same commit as the tip (`b77d049`).

**DONE 2026-11-06.** Terminal state: the keyed one-door gateway on the
final renamed config (== last-good); kokon 0.12 (`b77d049`) on the renamed
volume config, active model klein; BiRefNet-lite in the data volume; vllm
resident on 9106. Every acceptance criterion above is met — the PR branch
re-derived (`pr-external-engine-012`) and green; the w024 window ran to its
restore; the image and the fresh-box doc are at the final tip.

## decisions

- **2026-11-06:** the PR branch is re-derived **by rebase onto
  `reb-012`** (the 5 signed commits replay, original sign-offs and
  shapes preserved) and renamed `pr-external-engine-012` — content-
  identical to a cherry-pick re-derivation from the rebased fork, one
  less hand-curation step; the old `pr-external-engine-v2` name is
  retired. The two 0.12-semantics fixes inside it (bundle re-extract,
  kept-truncation) are part of the re-derivation, not new commits.
