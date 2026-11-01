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
- (open until the A/B lands: winner model + quant for qwen)
