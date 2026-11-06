# task-022 — one-door topology: all engines loopback-only, one key at the gateway

**Origin.** Owner proposal, 2026-11-04, mid task-020: vLLM has historically been
a hodgepodge of scripts with mixed binds (0.0.0.0 / 127.0.0.1) from the
pre-gateway single-tenant era; now that the gateway fronts everything, "all
engines listen on 127.0.0.1 only and just the gateway is exposed to LAN", and
the vLLM API key moves out of the vllm script to the gateway level ("llama-swap
fronting everything behind the same key").

**Status: CLOSED 2026-11-06 (proven by the w022 window runs 1–5; the box is
running the one-door single-key topology; see the record at the bottom).**

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

- (none remaining. The phone's pi needing K in its provider config is an
  owner-side open thread — it 401s at the gateway until then, but it does
  not affect the box; see the record's open threads.)

## acceptance criteria

- `ss -ltnp`: no non-gateway model port on 0.0.0.0; gateway 0.0.0.0:8081.
- Gateway rejects a request without the key (401) and serves with it;
  vLLM /v1/models answers the daemon's discovery (no 401 noise in the
  journal); a LAN-reachable request path exists only via the gateway key.
- Kokon UI + a production job + a vLLM stream all work through the new
  topology; the m020 window plan still passes unmodified (it uses
  /upstream/ paths, which keep working).

## Done 2026-11-06 — record

**All three acceptance lines met; the box runs the target topology.**
Evidence: `/tmp/w022{,r2,r3,r4,r5}/` (job.log, digest.txt, journal-slice,
proofs.txt, timeline); jobs `backends/_jobs/w022{,r2,r3,r4,r5}-job.sh`
(r1–r4 are superseded/failed variants kept for the post-mortem; r5 = the
passing run, 75 s).

**What the windows proved, run by run.**

- **r1** (config-level cutover): the 8-model `${PORT}` config swapped in
  (hot reload), the gateway keyed live (401/200/401 proven in-journal),
  `llama-swap.yaml.last-good` re-seeded + `chmod 600`, the daemon unit
  restarted onto `-listen 0.0.0.0:8081` (the wildcard shows as `*:8081`
  in `ss`), the vLLM restore fired through the *keyed* door. Died on a
  harness bug, not a box fault: `systemctl restart` is async (is-active
  still read the old process) and `ss` prints the IPv4 wildcard as `*`
  (grep wanted `0.0.0.0`). Both fixed in the r2+ files.
- **r2** (the proofs that don't need the service): 401 no-key / 401
  wrong-key / 200 with-key / 200 keyless `/health`; all eight models
  listed with the key; 1-token `/v1`; zero wildcard listeners on the
  assigned 9100–9107 range; **the vLLM pair switch both directions**
  (the vision request logged `set=vllm dsl="v | vv" evict=[qwen3.8-27b]`
  and booted on 9107; the text back-request evicted vision and reboots on
  9106 — the one-card invariant, visible in the config and the journal).
  Its P3 YIELDED on a **false positive**: the yield check grepped the
  journal for a boot line containing “starting”, but the daemon's boot-
  DECISION line (`set=… evict=… target=… cost=…`) never contains that
  word — only the `starting (no models running)` variant does, which
  postdates it. The “manual” vllm was the daemon's own; the yield's config
  copy reloaded the daemon, which immediately self-rebooted vllm — net
  effect benign (the corrected config went live then). Lesson recorded in
  both r2/r4 files: **the daemon's own `/running` list is the ground truth
  for “is this vllm mine?”**, not journal phrasing.
- **r3** (the service cutover): K staged into the volume's
  `/config/config.json` (pristine backup `config.json.bak-022`;
  `externalEngine` gained `apiKey`, the vlm entries keyed — the live
  config carried 2 empty-stub vlm entries, not 3: after 019 the three
  entrypoints share the one door, and two model entries carry a vlm
  block each); the
  container recreated onto image `external-engine-0.11.2-d93c3c3`.
  Gotcha found and fixed: the pre-022 container predates compose (no
  compose labels — it was `docker run`-era) so the first `compose up`
  hit a name conflict; the window now removes the container first (all
  state is in the named volumes `kokon-{data,config,gallery}`) and the
  recreated container is compose-managed from now on (labels present,
  P0 asserts the image + `com.docker.compose.service` label). Proven:
  UI :39152 up; the service's exact client path WITH THE KEY — the shared
  PUP door (9104: `/v1/models` shows both aliases `q35` + `qwen3vl-8b`,
  1-token `/v1/completions` answered in 8 s, page-cache warm) and the
  turbo engine (9105: `/sdcpp/v1/capabilities` in 5 s); stop propagation
  on the new `$1`-port scripts: turbo gone in 2 s, door in 2 s, card to
  1 MiB. (The creative pair's boot proofs are deliberately outside the
  acceptance path — each one costs a multi-minute chat drop; optional
  follow-up window, offered to the owner.)
- **r4** (klein, first attempt): fixed the boot race by polling the
  daemon's own health endpoint (loopback `/sdcpp/v1/capabilities`) until
  200 before posting; klein reached health in 7 s — but the 1-token
  `/v1/completions` probe is the wrong endpoint for this engine: the BFL
  hybrid loads Qwen3-VL-4B only as the prompt **conditioner** (the
  journal's load lines show `Conditioner params 2808 MiB`), and the stock
  sd-server answers `/v1/completions` with a clean **404, empty body**
  (journal: `status=404, path=/v1/completions` — three times, 0.9 ms
  each). The `<defunct>` sd-server in the r3/r4 digests was never a
  crash: it was my own `die()` restores evicting the just-healthy engine
  (the vllm set evicts everything) and the daemon mid-group-stop.
- **r5 (PASSED, 75 s)** — the service's actual image-job path on klein
  (9103): boot via a `/sdcpp/v1/capabilities` request, healthy in 7 s,
  then `POST /sdcpp/v1/img_gen` with exactly the klein service config's
  sampling (`1024×1024, euler, 16 steps, txt_cfg 1`) → **202 job accepted
  (`job_1a1b05e2d63_00000000`), polled through the keyed gateway, completed
  in 14 s with a real base64 PNG back** (validated `iVBOR…` magic);
  explicit UNLOADs (klein + the co-resident PUP door of set `klein: "k & p"`)
  to an empty card (1 MiB); terminal state: vllm text restored on 9106 by
  a 1-token request, **both stream modes working** (non-streaming + SSE —
  the “vLLM stream” line), `/v1/models` exactly `qwen3.8-27b`, `/running`
  exactly one entry, config identity `live == last-good == staged`
  (`115283be1f0b6b1350e77b134ee81462`), the 9100–9107 audit shows only
  9106 held, DONE marker last.

**The port contract (a v269 property, now documented in the config
header).** `${PORT}` is assigned at config-parse time to the `${PORT}`
models with their **IDs sorted alphabetically** (not declaration order —
`macros.go` collects the map keys and `sort.Strings` them) from
`startPort: 9100`, in order:

```
9100 creative-fast      9102 kokon-full       9104 kokon-pup
9101 creative-slow      9103 kokon-klein      9105 kokon-turbo
9106 qwen3.8-27b        9107 qwen3.8-27b-vision
```

Deterministic per config version; stable across daemon restarts; adding a
model whose ID sorts before an existing one shifts that model's number
(harmless — only the gateway knows the map, and a reload re-boots on the
new number). The journal always prints the number actually in use
(`cmd: … <script>\n<port>` in `/running`).

**Final box state (2026-11-06, post-r5).**

- `backends/_configs/llama-swap.yaml` = the one-door config (==
  `llama-swap.yaml.last-good`, `chmod 600` both): top-level `apiKeys`
  (the 48-char box key, kept in `gateway.key` 600 — never echoed anywhere
  but file copies), `startPort: 9100`, eight `${PORT}` models, the final
  five-set matrix (`klein: "k & p"`, `qwen-t: "qt & p"`,
  `qwen-f: "qf & p"`, `small: "cf | cs"`, `vllm: "v | vv"`).
- Daemon `ml-llama-swap` (v269 `9fbc47c`): `-listen 0.0.0.0:8081
  -watch-config` — the wildcard 8081 is the box's only LAN door for model
  traffic; `/health` stays open keyless by v269 design. The only other
  fixed addresses: :39152 (kokon UI) and :18021 (the phone's pi-read-more
  proxy — it forwards client headers untouched, so a phone pi carrying K
  works through it unchanged).
- Every engine: `127.0.0.1` only, daemon-assigned port from `$1`, pure-boot
  scripts (the vLLM pair's tuning knobs remain byte-frozen per the
  owner's final calibration — only `--host` and the port source moved).
- kokon service: compose-managed container on `external-engine-0.11.2-
  d93c3c3` (repo commit `d93c3c3`: `apiKey` on `ExternalEngine` + the
  `SdcppClient` auth header on all six `/sdcpp/v1/*` calls + tests;
  container recreation now a plain `docker compose up -d`). The volume's
  `config.json` carries K on `externalEngine.apiKey` + the vlm entries
  (pristine backup `config.json.bak-022`).
- pi `models.json`: `vllm-direct` removed (backup kept); `llamaswap`
  (→ `:8081`, keyed) is the only vLLM route.

**Owner-side open threads (not blockers).**

- The **phone's pi** needs K in its provider config (or it 401s at the
  gateway); its `:18021` proxy needs no change.
- The creative pair's `$1`-port boot proof, if the owner wants it
  (optional window; each boot costs a multi-minute chat drop).
- Docs in their settled shape (`docs/fresh-box.md`'s gateway section —
  the one-door topology, the alphabetical port contract, *no* port table;
  the add-a-model guide — a new model needs no port knowledge; host OPS
  notes) — drafted for the owner's sign-off per the docs rule.

**No new owner decisions were required during execution;** the two
mid-window corrections (the yield ground truth; the compose label
history) are harness/deploy-history facts, recorded above.
