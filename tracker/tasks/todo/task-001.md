# task-001 — llama-swap: pin v269, deploy artifacts to the sibling

## goal
`backends/llama-swap` = 100% clean upstream at a pinned tag (current upstream
head is **v269**; the tree is at v267, 0 ahead / 17 behind). All of our
deploy artifacts — currently untracked files inside the repo tree
(`config.yaml` + `.bak`, `creative-fast.sh`, `creative-slow.sh`,
`evict-gpu.sh`, `vllm-serve.sh`, `serve.sh`, the test scripts, `state/`,
`logs/`, the built `llama-swap` binary) — move to the non-git sibling
`backends/llama-swap-deploy/`. The gateway is rebuilt from the pinned tree
and comes back up with every managed model working.

## acceptance criteria
- `git status` in `backends/llama-swap` is clean at the upstream v269 tag.
- Deploy artifacts live in `backends/llama-swap-deploy/` and are referenced
  correctly by whatever starts them (no dangling old paths).
- The binary is rebuilt from the pinned tree and serves `:8081`.
- On-demand load verified for every managed alias: vllm 27B, both creative
  models, both engine aliases (gateway health curl per alias).
- vllm healthy at the end — this is the one restart window in WP0 and uses
  the full autonomy protocol (the gateway supervises the operator runtime).

## blockers
- One gateway-restart window (vllm down during it; standard protocol).
- Our `config.yaml` was written for v267 — validate it against v269's
  `config-schema.json` / `config.example.yaml` before rebuilding (17
  commits moved; schema drift is the main risk).

## decisions
- Sibling dir name = `backends/llama-swap-deploy/` (confirm at execution).
- Pin = v269 (current upstream head at extraction time).
