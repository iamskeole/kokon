# task-022 — one-door topology: all engines loopback-only, one key at the gateway

**Origin.** Owner proposal, 2026-11-04, mid task-020: vLLM has historically been
a hodgepodge of scripts with mixed binds (0.0.0.0 / 127.0.0.1) from the
pre-gateway single-tenant era; now that the gateway fronts everything, "all
engines listen on 127.0.0.1 only and just the gateway is exposed to LAN", and
the vLLM API key moves out of the vllm script to the gateway level ("llama-swap
fronting everything behind the same key").

**Status: parked** (owner input: the two open decision points below; execution
needs the owner's vLLM to be restarted at a convenient moment — the pi agent
session rides on it). Lands after task-020 (its P7 cutover) and before
task-021, so the vision variant enters the final pattern and the task-021
guide documents it.

## The target topology

| Component | Today | Target |
|---|---|---|
| vLLM (:18020) | 0.0.0.0, `VLLM_API_KEY` from two drifting sources (canonical `api_key.txt` vs daemon config `env:` — the source of the m020-a3 401 discovery artifact) | **127.0.0.1 only, no `VLLM_API_KEY`** |
| llama-swap gateway (:8081) | 127.0.0.1, **no client key** | **0.0.0.0 (the only LAN door) + `apiKeys: [key]`** (stock v269 field) |
| creative ×2 (:39176/77), sd-server ×2 (:39160/70), pup (:39175) | per-launch binds (llama.cpp/sdcpp defaults vary) | 127.0.0.1 in every launch line |
| qwen-fastani (:18021) | 0.0.0.0 | 127.0.0.1 |
| kokon service | API already 127.0.0.1:39151; UI 0.0.0.0:39152 (token-protected) | decision point (2) |

## Why this works on stock v269 (verified in source, `9fbc47c`)

The gateway's upstream proxy is a plain `httputil.ReverseProxy`
(`internal/process/process_command.go:481`) with **no Director rewriting
`Authorization`**: the client's key is forwarded to the upstream as-is, and
the v269 config has **no per-upstream key field**. So a single gateway key
works iff the upstream does not reject it — i.e. vLLM runs **keyless**
(when `VLLM_API_KEY` is unset, vLLM accepts any/no Bearer and the forwarded
gateway key is ignored). The alternative — vLLM keeping its own distinct key
— is **not possible** on stock v269 (would need per-model auth-header
injection; a small upstream PR, task-017-era candidate, not needed on a
single-user box).

## Scope

1. `serve.sh` / `serve-vision.sh` (stable dir): `--host 127.0.0.1`;
   `VLLM_API_KEY="${VLLM_API_KEY:-$(cat api_key.txt)}"` (respect a key the
   daemon passes; canonical path falls back to the file); drop
   `api_key.txt` (single source: the gateway key file both paths read —
   or truly keyless: the gateway key is the only one that exists).
   Engine parameters (ctx, memory, speculative, …) unchanged — the owner's
   explicit constraint.
2. Every engine launch line: 127.0.0.1 binds (creative, sd-server ×2, pup).
3. Gateway unit: `-listen 0.0.0.0:8081`; config: top-level `apiKeys:`.
   Client updates: kokon service gateway client (add the key to its config),
   Open WebUI endpoint key, qwen-fastani (its `Authorization` env), job
   scripts, MCP if it fronts the gateway.
4. Fresh-box doc (task-016) updated to the one-door pattern; task-021's
   "adding a model" guide written against it.
5. vLLM restart (the owner's agent session drops and reconnects — do it as
   the last step, announced).

## decisions (owner)

- (1) vLLM keyless-on-loopback (recommended) vs shared-key (vLLM enforces
  the SAME value as the gateway — one secret, two checkpoints; costs the
  401-drift class only if the two copies are kept in sync).
- (2) kokon UI :39152: keep 0.0.0.0 (token-protected; Tauri + LAN browsing)
  or loopback-only (UI reachable only on the box).

## blockers

- The owner's call on (1) and (2).
- A convenient vLLM-restart moment (the agent session rides on vLLM).

## acceptance criteria

- `ss -ltnp`: no non-gateway model port on 0.0.0.0; gateway 0.0.0.0:8081.
- Gateway rejects a request without the key (401) and serves with it;
  vLLM /v1/models answers the daemon's discovery (no 401 noise in the
  journal); a LAN-reachable request path exists only via the gateway key.
- Kokon UI + a production job + a vLLM stream all work through the new
  topology; the m020 window plan still passes unmodified (it uses
  /upstream/ paths, which keep working).
