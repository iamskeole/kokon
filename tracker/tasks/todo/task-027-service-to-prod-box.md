# Task-027 (PARKED — owner-triggered) — move the kokon service (+ MCP + its data) to the stable prod box, behind nginx; the accelerator keeps only the gateway + engines

**Created:** 2026-11-11 — the owner moved Open WebUI to the more stable
"prod" box (nginx front, talks to the accelerator's gateway on the
one-door `:8081` with an OpenAI-compatible API) so that the accelerator
can be taken down for OS/CUDA/driver work. He asked whether moving the
kokon frontend/service the same way has footguns.

## Trigger (when this stops being parked)

The owner says go, and names the prod box (host/IP, its nginx, and
whether it terminates TLS — self-signed or real).

**The setup is already written up:** `docs/service-on-prod-box.md`
(handed over 2026-11-11 with the owner's request for a handover
prompt) — the paste-able brief, the bind-mounted compose (data dirs
next to the yaml, chown 1000:1000), the config seed (gateway + vlm
entrypoint → the accelerator's IP), the nginx values that are
load-bearing (SSE buffering off, ≥660 s timeouts, 64 m bodies, the
X-Forwarded headers), the verification checklist, and the receiving
side of the data migration. Executing the move = this task: pack the
three volumes on the accelerator (service stopped), ship them, run
the brief on the prod box, verify, retire the accelerator's volumes
only when the owner is happy.

## Why the move is architecturally clean (not a hack)

- The service container already runs `kokon-service --persistent
  --remote`: it never self-terminates on idle, and the remote port
  (39152, password + session cookies) is the browser door. The token
  port (39151) stays 127.0.0.1-only by design.
- **0.12 is explicitly reverse-proxy-aware** (`remote.rs`): a private
  peer's `X-Forwarded-For` is believed for the per-IP attempt limiter,
  and `X-Forwarded-Proto: https` from a private peer lets the session
  cookie carry `Secure`. nginx in front is the intended usage.
- The only cross-box traffic becomes service→gateway
  (`http://<accelerator>:8081/upstream/…`) — the same one-door,
  single-key pattern the prod box already carries for owui's
  OpenAI traffic. After the move the accelerator holds **no user
  data, no UI, no MCP** — pure muscle: gateway, engines, models.
  OS/CUDA/driver reinstalls then have nothing personal to break.
- The gateway does **not** health-check the service (verified in the
  live yaml: no per-model healthCheck entries; the kokon service is a
  client of the gateway, not a registry member) — nothing on the
  accelerator depends on the service being there.

## Footguns (the checklist for the actual move)

1. **nginx must be configured for this API, not just pointed at it:**
   - `/events` is a long-lived **SSE** stream: `proxy_buffering off`
     and a long `proxy_read_timeout` on the kokon location.
   - Job waits run for minutes (the service's own client timeout is
     600 s): `proxy_read_timeout >= 600s`, else nginx 502s mid-job.
   - Edit references are base64-in-JSON: nginx's default
     `client_max_body_size` is **1 MB** → 413 on any real photo.
     Set 32–64 m.
   - No WebSockets involved (SSE + fetch only) — no Upgrade handling.
2. **The token port must not go LAN.** nginx proxies to the *remote*
   port (39152) only. **The MCP server moves to the prod box with the
   service** (it speaks the token port over loopback; it cannot reach
   another box's loopback, and the token port should never be LAN-
   exposed). Whatever agent sessions rely on that MCP must move too.
3. **Gateway URL in the service config:** `127.0.0.1:8081` → the
   accelerator's **IP** (not a hostname: OS reinstalls can change
   names; the IP survives). Same 48-char box key (already
   multi-homed: owui, pi clients, the service).
4. **Data move:** three docker volumes (`kokon_kokon-{data,config,
   gallery}` on the accelerator; the service's real data — `gallery.db`
   + `inputs`/`originals`, `config.json` + backups, dated gallery
   folders). Safest: stop the service, tar/copy the volume contents,
   restore on the prod box, start, verify the DB opens and the
   **service token** is intact (the MCP depends on it; it lives in
   the data volume). The `~/Pictures/Kokon` folder on the accelerator
   is the *desktop app's* gallery (a second, separate Kokon on the
   box), not the service's — it stays with the desktop install.
5. **Prod compose differences:** drop the `nvidia` device reservation
   (the service needs no GPU in external-engine mode — every
   inference is at the gateway; without an nvidia runtime on the
   prod box the reservation would refuse to start). Image is 237 MB
   compressed. Ports 39151/39152 free on the prod box.
6. **Failure semantics (the point of the move):** accelerator down ⇒
   the prod box keeps serving the UI and login; job requests fail
   fast (3 s connect timeout) into the visible "engine unreachable"
   error state; when the accelerator returns, the next start
   re-adopts the gateway (this death/recovery path is the 16/16
   mock-gateway harness, sections 6–7). Cost: in-flight jobs during
   the outage are lost (re-submit works), and the first job after
   recovery pays one cold model load (same as the current 10-min idle
   unload — not new).
7. **Cosmetic/records:** the gateway yaml comment calls `:39152`
   "the only other fixed address on the box" — stale after the move;
   update it in the next config edit.
8. **Opportunity, not just a footgun:** if the prod nginx terminates
   TLS (self-signed is enough on a trusted LAN), browser clients
   gain a **secure context ⇒ WebGPU**, so the Layers lab's
   click-to-select actually runs on the client's GPU instead of the
   slow single-thread WASM fallback it silently degrades to today
   over plain-HTTP LAN. The service already honours
   `X-Forwarded-Proto` for the `Secure` cookie.

## Upstream relation

Stock 0.12 runs the service on the machine that owns the GPU (one
box, one person). A split service/engines deployment is fork territory
by nature (the external-engine mode *is* the fork) — nothing to
contribute, all additive on top of stock's own reverse-proxy support.

## blockers

None. Parked on the owner's trigger (needs: prod box identity, nginx
layout, TLS or not).
