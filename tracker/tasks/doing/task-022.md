# task-022 — one-door topology: all engines loopback-only, one key at the gateway

**Origin.** Owner proposal, 2026-11-04, mid task-020: vLLM has historically been
a hodgepodge of scripts with mixed binds (0.0.0.0 / 127.0.0.1) from the
pre-gateway single-tenant era; now that the gateway fronts everything, "all
engines listen on 127.0.0.1 only and just the gateway is exposed to LAN", and
the vLLM API key moves out of the vllm script to the gateway level ("llama-swap
fronting everything behind the same key").

**Status: CURRENT (2026-11-05, scope finalized with the owner in this span's
discussion; see decisions below).**

## The target topology (final, 2026-11-05)

| Component | Before | After |
|---|---|---|
| vLLM text+vision (:18020) | 0.0.0.0, keyless in practice (no `VLLM_API_KEY` anywhere, no `api_key.txt` — the 020-era drift source is simply gone) | **127.0.0.1, keyless, auto port** (`${PORT}`) — the owner's final calibration untouched: only the bind line and the port source change, no tuning knob |
| creative ×2, kokon-turbo/full, kokon-klein, kokon-pup | fixed ports 9100/9101 (auto) / 39160 / 39170 / 39175 | **all `${PORT}` auto-assigned** (9100+ in declaration order) — zero fixed engine ports in the config |
| llama-swap gateway (:8081) | 127.0.0.1, keyless | **0.0.0.0 (the only LAN door) + `apiKeys: [K]`** where K = the 48-char key already shared by all pi providers (the one on :18021, which the owner pointed at) |
| kokon service | no key on its gateway client (the VLM path had an `apiKey` field, the engine path had none) | `apiKey` added to `ExternalEngine` (code change) and set, together with the three `vlm.apiKey` entries, to K |
| pi `vllm-direct` provider | `:18020` direct | **removed** (duplicate of `llamaswap` once vLLM has no stable direct port); `models.json` backed up first. The pi-read-more proxy (:18021, phone UI) already proxies the gateway and forwards client headers untouched — no change |
| vLLM boot script's attach branch | probes fixed :18020 (the human-boot convention) | probes the **assigned** port (`$1`): the human-boot convention is retired (the gateway is the only front), but the same config always assigns the same number, so after a daemon restart the probe still finds a surviving process and re-attaches — the orphan self-heals, no convention needed |

### The matrix (final sets)

```
vars:  k=kokon-klein  p=kokon-pup  qt=kokon-turbo  qf=kokon-full
       v=qwen3.8-27b  vv=qwen3.8-27b-vision  cf=creative-fast  cs=creative-slow
sets:  klein: "k & p"     qwen-t: "qt & p"     qwen-f: "qf & p"
       small: "cf | cs"  vllm: "v | vv"
```

`vllm: "v | vv"` is the owner's explicit encoding of the one-card invariant:
verified in source (`internal/matrix/program.go` — `nodeOr` evaluates each
branch as its own state; there is no state containing both disjuncts), a
request for one variant evicts the other (and everything else), identical
to today's orphan behavior but now visible in the config and in the
journal (`set=vllm dsl="v | vv"` instead of `set= dsl=""`). The qwen pair
is already expressed the same way (two sets, no set contains both).

### Why auto-assignment is safe for the two-identity pairs (settled 2026-11-05)

The router is the sole eviction authority and it serializes: orphans evict
everything; different sets evict each other; the matrix completes an
eviction before booting. So at most one process of each pair ever exists,
regardless of ports — the old shared fixed ports (18020, 39160) were a
redundant second lock from the pre-matrix one-at-a-time-group era (the
task-013 fix). `${PORT}` is assigned at config-parse time in declaration
order (`macros.go`, `nextPort++`) — deterministic per config version,
stable across restarts; only a config edit moves a number, and the journal
always shows which is which. The daemon-restart procedure (unload first)
stays as the standing habit.

## Scope (execution order, 2026-11-05)

1. **Kokon code** (repo): `apiKey` on `ExternalEngine` (serde default,
camelCase, doc-matched to `VlmEndpoint.apiKey`); `SdcppClient` gains the
header (sent on all six `/sdcpp/v1/*` calls when non-empty); the
construction site `engine.rs` passes it; tests (config default/parse,
header present/absent); cargo + vitest + mcp green; DCO commit.
2. **Image rebuild** from the commit (docker/Dockerfile, tag convention
`external-engine-0.11.2-<short>`); `frontends/kokon/compose.yaml` updated;
container recreated; the volume's `config.json` gets K on `externalEngine.apiKey`
+ all three `vlm.apiKey` (old image tolerates the unknown field —
no `deny_unknown_fields` — so it can be staged before the recreate).
3. **Host**: `_configs/gateway.key` (600, the single secret, copied from
the pi provider — never echoed); vLLM scripts `--host 127.0.0.1` + port
from `$1`; klein/pup/qwen×2 scripts take `$1`; daemon yaml: six entries to
`${PORT}` cmd+proxy, the `vllm: "v | vv"` set + `vv` var, top-level
`apiKeys: [K]`, comments rewritten, `llama-swap.yaml.last-good` re-seeded,
file 600; unit `-listen 0.0.0.0:8081`; `_jobs/` scripts get the key header
and gateway-based probes (no port checks); `models.json` loses
`vllm-direct` (backup kept).
4. **Cutover window** (autonomy protocol, marker last; the box chat drops
~1 minute — vLLM restart — and the phone's pi needs K in its provider
config or it 401s): preflight → config write (hot reload drops vLLM)
→ daemon restart (new listen) → vLLM 1-token boot via the gateway (warm
~45-60 s) → proofs: 401 without / wrong key, 200 with, `/v1/models`
with key, `ss` (only :8081 on 0.0.0.0 among model ports), `/v1`
chat 1-token, `/upstream/kokon-pup` with key (door boots on its assigned
port — journal shows it) → explicit UNLOADs (door, klein) → final
identity (live == last-good) + vLLM resident as the terminal state →
DONE.
5. **Docs**: `docs/fresh-box.md` gateway section (the one-door topology,
the five-set matrix, *no* port table — ports are daemon-internal from
`startPort`; the only fixed addresses are :8081 and :39152), the
add-a-model guide (a new model needs no port knowledge), the host OPS
notes. **Close** with the record.

## decisions (owner)

- **2026-11-05:** the key for the gateway = the 48-char key already set
  on the pi provider for :18021 ("the qwen-fastani pi provider"); all
  three pi providers already share it, so no client on the box changes
  key values.
- **2026-11-05:** the vLLM scripts may be touched for the bind (`--host
  127.0.0.1`) and the port source, **never** for the model's tuning
  knobs (kv-cache, speculative, ctx, batch — the owner's final
  calibration).
- **2026-11-05:** no custom ports left for the kokon engines — the UI
  (service) points at the gateway, nothing external needs a number; the
  goal is to minimize moving pieces and ad-hoc conventions ("future me
  won't remember them").
- **2026-11-05:** the vLLM pair gets an explicit mutual-exclusion set
  (`vllm: "v | vv"`) so the matrix visibly cannot have both live — the
  human-boot/adopt convention predates the box-behind-gateway and is
  retired; the attach code stays only as a daemon-restart self-heal on
  the same assigned number.
- **2026-11-05 (correction accepted):** the earlier "keep 18020/39160 as
  shared fixed ports" proposal was rejected — the router's
  evict-then-boot already enforces one-process-per-card for both
  pairs; the ports were a redundant pre-matrix lock.

## blockers

- The phone's pi instance needs K in its provider config (owner's side;
  401 at the gateway until then).

## acceptance criteria

- `ss -ltnp`: no non-gateway model port on 0.0.0.0; gateway 0.0.0.0:8081.
- Gateway rejects a request without the key (401) and serves with it;
  vLLM /v1/models answers the daemon's discovery (no 401 noise in the
  journal); a LAN-reachable request path exists only via the gateway key.
- Kokon UI + a production job + a vLLM stream all work through the new
  topology; the m020 window plan still passes unmodified (it uses
  /upstream/ paths, which keep working).
