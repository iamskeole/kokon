# task-010 — PUP A/B: 8B {Q4_K_M, Q8_0} vs 9B {Q4_K_M, Q6_K, Q8_0} on the sidecar

## goal
Pick the production PUP + quant. Six candidates, all on the same sidecar
runner (llama-server, :39175, `--mmproj`, thinking off, `--jinja`), each
evaluated on the three standing prompts carried over from the A/B rig:
C1 kitchen t2i, C2 desk edit, C3 lantern+robot compose. The BFL system
prompt comes from the repo assets (task-007).

## acceptance criteria
- Per candidate: a digest file with the three outputs (captions, verbatim
  is fine — they are model *answers*, not BFL template strings), token
  counts + wall times, and a server-log tail (proves which model/quant was
  loaded each run).
- `DONE`/`FAIL` markers per run; every run is a self-contained GPU window
  (autonomy protocol: setsid job + watcher + blocking call + vllm reload
  at the end of each window).
- The owner visually validates the renders (the captions alone are not the
  test — the rendered images are).
- Winner recorded in `STATUS.md` (D-FB note: quant also bounded by memory
  headroom with the DiT co-resident on the 24GB card — Q8 9B + DiT Q8 ≈
  22.7GB; Q4/Q6 are the safe band; record the measured fit).

## blockers
- task-003 (files in place), task-004 (runner), task-007 (prompt assets).

## decisions
- Batch policy: one candidate per GPU window, or several back-to-back in
  one window (time vs risk; decide at execution — 6 candidates × ~2–3 min
  each suggests two windows of three, but confirm).
- The 8B baseline exists (its in-engine A/B already showed Q8_0 >
  Q4_K_M); re-run on the sidecar anyway so both models are compared on
  identical infrastructure.
