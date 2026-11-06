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
