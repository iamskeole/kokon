# task-014 — qwen: A/B the shared PUP, then cut over (D-QW)

## goal
Per D-QW: A/B the qwen-side VLM — the klein-fine-tuned PUP (9B, the
task-010 winner quant) vs the 8B Qwen3-VL (qwen's current VLM) — judged
on **qwen's own** enhancement prompts. Cut qwen's enhancement over to
the sidecar with the winner; the in-engine PUP slot turns off for the
qwen engine (both pipelines then share one VLM artifact, per the reuse
rule).

## acceptance criteria
- Digest: both candidates over a standing qwen prompt set (t2i + edit),
  using qwen's prompt bundle (which becomes a repo asset in the
  task-007 pattern — BFL silence rules apply to the BFL set; qwen's own
  prompts are the model's and are recorded as assets too).
- The owner judges enhancement quality vs current qwen production —
  the bar is *not worse*, not *different*.
- Winner wired as the qwen pipeline's `vlm` entry (task-005 config);
  qwen jobs run end-to-end through the sidecar; the in-engine PUP
  flags are off in the qwen serve script.
- Winner recorded in `STATUS.md` (the D-QW open item closes).

## blockers
- task-013 (qwen on stock, same sidecar runner), task-006 (the client),
  task-007 (assets pattern). GPU windows for the A/B runs.

## decisions
- **Goal reinterpretation (post-task-013):** the file predates the
  stock cutover — there is no in-engine PUP slot to turn off anymore;
  the qwen sidecar IS qwen's PUP (task-013, owner's Option A). The A/B
  is therefore **9B vs 8B on the same sidecar runner**: the current
  qwen door (8B Qwen3-VL Instruct Q4_K_M) vs the 9B BFL klein-pup (a
  Qwen3.5-9B fine-tune, the task-010 Q8 winner on klein's sidecar)
  run with klein's production recipe. Same alias (qwen3vl-8b) so the
  service config is identical between legs — only the gateway door
  entry's cmd differs.
- **Repo side done (2026-11-03, commits a528f0a + 610ccb3):** the
  qwen prompt set is now a repo asset — `crates/qls-core/assets/pup-qwen/`
  (system_prompts.json: t2i/edit/compose/compose_label, extracted
  verbatim from the built-in enhancer constants; a test pins asset and
  constants together). `pup.rs` is the two-family loader (BFL family:
  all six keys + companion mandatory; Qwen family: t2i mandatory, the
  rest optional with built-in fallback — the Qwen path never
  hard-fails on its bundle). `enhancer.rs`/`engine.rs`: the Qwen path
  uses the pipeline's own `promptBundle` (schema-13 per-model field,
  previously BFL-only) — Turbo/Full now point at `pup-qwen`; a broken
  bundle is a job note, not a job failure; the BFL branch takes the
  unwrapped BflPrompts form (a BFL pipeline pointed at a non-BFL
  bundle fails the job cleanly). 327/327 qls-core + 160/160 UI.
  Deployed: image external-engine-0.11.2-610ccb3, container recreated,
  bundle in the data volume at /data/assets/pup-qwen, service config
  has the per-model `promptBundle: "pup-qwen"` on Turbo/Full only.
- **Window design (`jobs/qwabl-*`, W=/tmp/qwabl; launched
  2026-11-03 after the file-side work):** the standing qwen set =
  3 t2i (seeds 11/22/33, 768², the kleenq A/B prompts) + 1 edit
  (seed 44, "a watercolour painting of this scene", ref = newest
  gallery PNG). vLLM out of the registry for the whole window
  (window-014a/014b = window-013 lineage minus vLLM); leg A =
  window-014a (8B door = production); mid-window atomic swap to
  window-014b (turbo door cmd → `deploy/qwen/qwen-pup-serve-9.sh` —
  9B Q8, production klein recipe, same port 39176 + pidfile + alias,
  so `qwen-pup-stop.sh t` tears it down); leg B = same 4 jobs, same
  seeds. Per-leg gates: pid stability across the 4 jobs (engine.pid
  + door pidfile — a churn regression dies the window), leg-B door pid
  must differ from leg-A's, 1-token probe per door, VRAM recorded
  (turbo engine ~10.3 GB + 9B door ~11 GB ≈ 21.2 GB — tight but fits,
  like the quality pair). Final: active back to klein, atomic swap to
  stock-013, canonical vLLM revival last (stop-before-start on an
  unhealthy holder), 1-token gateway adoption check, digest + DONE
  last. The 9B door cold load (9.5 GB, first page-cache fill) is the
  long pole — leg B door wait budget 15 min. Budgets: job self-
  deadline 45 min; watcher 60-min hard cap + 45-min secondary silence
  (pid-aware, qwenp pattern); wait-marker 80-min cap. `sh -n` clean
  on all four scripts (POSIX dash).
- (open until the A/B lands: winner model + quant for qwen)
