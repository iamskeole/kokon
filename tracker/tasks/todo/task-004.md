# task-004 — PUP sidecar: llama-server serve script + gateway entry

## goal
A serve script for the PUP running on the **named** llama.cpp
(`backends/llama.cpp`, the May-21 build — proven to load and run the PUP
with vision on both local llama.cpp trees): `llama-server` on port **39175**
with `--jinja` (the BFL chat template is embedded in the GGUF),
`--mmproj` (the `clip`-arch tower), thinking mode off, and the starting
quant. Registered as a llama-swap alias with on-demand load. Readiness is
probed with a **1-token `/v1/completions`** (200 = ready, 503 = still
loading) — never `/health`.

## acceptance criteria
- Gateway on-demand load brings the sidecar up; 1-token probe returns 200.
- A BFL-t2i request via raw `/v1/chat/completions` (BFL system prompt from
  the assets, an image part where needed) produces a BFL-contract caption
  — re-verifying the archived sidecar-proof result on this new path.
- Serve/stop scripts in the deploy location; vllm reloaded at the end
  (this is a GPU window — full autonomy protocol: blocking call, setsid
  watcher, reload via the gateway).

## blockers
- One GPU window (vllm down during it).

## decisions
- Starting quant = 9B `Q4_K_M` (smallest; task-010 picks the production
  winner — the runner just swaps files).
- Port 39175 (the plan's allocation; scratch :9999 stays free).
