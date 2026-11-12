# The service on the stable box (engines stay on the accelerator)

The split deployment: **this** box runs the kokon service, the browser
UI, and (optionally) the MCP — no GPU, no models. All inference runs on
the **accelerator** box through its one-door gateway (`:8081`, keyed,
OpenAI-style door for other clients). This is the setup that lets the
accelerator go down for OS/CUDA/driver work while the UI keeps serving.
See also [fresh-box.md](fresh-box.md) (the whole stack on one machine)
and the tracker's task-027 (the move from the current deployment).

## The prompt to hand over on the stable box

Copy from the next fence, fill in the angle-bracket values, and paste it
to the agent on that box:

```text
Set up the Kokon image service on this box. The full reference is in
docs/service-on-prod-box.md of the repo in step 2 — read it; the values
below override anything ambiguous in it.

Owner-supplied values:
- This box: <name> (<IP>)
- Accelerator (the model gateway) IP: <IP>. Its gateway listens on
  :8081 — the box's only model door, every request needs the key.
- Gateway key: a file at <path on this box>, mode 600. Read it for the
  config. Never print it — not in your reply, logs, diffs, or commits.
- What people type for the UI: <name or this box's IP>, port <n>;
  TLS: <yes — self-signed cert at <path> | no, plain HTTP>.
- Data location: <path> (default: the directories next to the compose
  file). Fresh start, or: migrate my existing data (tarball at <path>).
- Default image model: <kokon-qwen-image-2.1-turbo | kokon-qwen-image-2.1-full | kokon-flux-image-3-klein>.

Do, in order:
1. Preflight: docker + compose plugin present; ports 39151 and 39152
   free; ~5 GB of disk; NO nvidia runtime needed (and none must be
   reserved — this service uses no GPU in this mode).
2. git clone https://github.com/iamskeole/kokon.git <dir> — branch
   main. The build is self-contained (its own Rust/npm toolchain in
   the image); allow it up to 30 minutes cold.
3. Build the image: docker build -f docker/Dockerfile -t
   ghcr.io/iamskeole/kokon:external-engine-0.12.0-<short-commit> .
   (or pull that tag from ghcr instead, if the owner says it is pushed
   and hands you a registry token).
4. Create <data>/data, <data>/config, <data>/gallery and chown them
   1000:1000 (the in-container user). Writing them next to the compose
   file is the point: the owner must be able to see and back them up.
   If migrating: unpack the owner's tarballs into those directories
   first (they carry the gallery, the config, the service token and
   the login password); keep the owner's copy untouched.
5. Write <data>/config/config.json per the reference in this doc:
   gateway and vlm entrypoint at http://<accelerator-IP>:8081/upstream
   (the vlm one with /kokon-pup appended), apiKey from the key file,
   the owner's default model, the rest copied from the accelerator's
   current config when migrating (or the doc's template when fresh).
6. docker compose up -d with the compose from the doc. If this is a
   fresh start, set the browser password:
   docker exec -it kokon kokon-service --set-password
   (a migrating setup keeps the existing password in its data).
7. Install the nginx server block from the doc (the exact values are
   load-bearing: SSE buffering off, timeouts, body size, the two
   X-Forwarded headers) and reload nginx.
8. Verify with the doc's checklist — including one real generated
   image (this needs the accelerator up; ask the owner if it is).
9. Report back: what you did, where the service token file is (do not
   print the token itself), any deviation you had to make, and the one
   thing the owner should open from a second device to believe it.

Guardrails:
- Never print the gateway key or the service token (first 4 chars
  max, masked).
- This box stores no models and installs no engine; the only traffic
  you initiate toward the accelerator is through the gateway door on
  :8081 with the key.
- If the box's reality conflicts with this brief (port, path, nginx
  layout), stop and ask before adapting anything.
```

## Reference the agent works from

### Layout

```
<data>/
  compose.yaml      # next to the data, on purpose
  data/             # -> /data    gallery.db, service token, lab models, logs
  config/           # -> /config  config.json (gateway address + key)
  gallery/          # -> /gallery saved images, dated folders
```

Bind mounts (not named volumes) so the data is a directory the owner
can see, back up and move — the whole point. Create the directories
before first start and `chown 1000:1000` them (the container runs as
uid 1000; a directory docker creates on your behalf comes out
root-owned and the service cannot write it).

### compose.yaml

```yaml
# Kokon service on the stable box. All inference runs on the
# accelerator through its gateway (:8081, keyed) — nothing in this
# container touches a GPU, so there is no nvidia reservation.
#
#   docker compose up -d          # start / recreate
#   docker compose logs -f kokon  # service log
#
# Ports: 39152 = web UI (password; all interfaces — put nginx in
# front), 39151 = token API (127.0.0.1 only: the MCP on this box).
services:
  kokon:
    image: ghcr.io/iamskeole/kokon:external-engine-0.12.0-<short-commit>
    container_name: kokon
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./data:/data
      - ./config:/config
      - ./gallery:/gallery
```

The container's entrypoint is `kokon-service --persistent --remote`:
it stays up when idle and opens the browser door. `--set-password`
(one-off, same command form) sets/changes the browser password
without a restart; it is stored as an Argon2id hash in
`data/remote-auth.json` (sessions: `data/remote-sessions.json`).

### config/config.json (seed)

```json
{
  "schemaVersion": 13,
  "servicePort": 39151,
  "setupComplete": true,
  "externalEngine": {
    "gateway": "http://<accelerator-IP>:8081/upstream",
    "apiKey": "<the box key — from the key file, never typed in chat>",
    "model": "<the owner's default model id>",
    "modelName": "<that model's gguf name>",
    "sample": { "cfg": 1, "clear_sigmas": true, "sampler": "euler", "steps": 40 },
    "vlm": {
      "entrypoint": "http://<accelerator-IP>:8081/upstream/kokon-pup",
      "model": "qwen3vl-8b",
      "apiKey": "<same key>"
    },
    "promptBundle": "pup-qwen"
  }
}
```

When migrating, copy the accelerator's current values verbatim
(`model`, `modelName`, `sample`, `promptBundle`) and change only the
two `127.0.0.1` addresses to the accelerator's IP. Model ids and
gguf names as the accelerator's gateway knows them:

| model id | modelName |
|---|---|
| kokon-qwen-image-2.1-turbo | qwen-image-2.1-turbo-Q4_K_M.gguf |
| kokon-qwen-image-2.1-full | model-Q8_0.gguf |
| kokon-flux-image-3-klein | model-Q8_0.gguf |

(For klein the `promptBundle` is `pup`, not `pup-qwen` — follow the
accelerator's config, which is the source of truth.)

### nginx (the load-bearing values)

```nginx
# /etc/nginx/conf.d/kokon.conf  (or a location inside an existing vhost)
#
# Three things make or break this proxy:
#  1. /api/v1/events is a long-lived SSE stream — buffering must be OFF.
#  2. job waits and the events stream stay open for minutes — the
#     read/send timeouts must beat the service's own 600 s job timeout.
#  3. edit references travel as base64 in JSON — nginx's default 1 MB
#     body limit rejects real photos (413).
server {
    listen 443 ssl;              # or listen 80 if this LAN stays plain HTTP
    server_name <ui-name>;
    ssl_certificate     <cert>;  # self-signed is fine on a trusted LAN
    ssl_certificate_key <key>;

    client_max_body_size 64m;
    proxy_http_version 1.1;
    proxy_buffering off;          # SSE: stream straight through
    proxy_read_timeout 660s;
    proxy_send_timeout 660s;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;   # lets the session cookie carry Secure

    location / {
        proxy_pass http://127.0.0.1:39152;
    }
}
```

The service trusts `X-Forwarded-For` / `X-Forwarded-Proto` **only from
private peers** (nginx on the same box qualifies; a direct client's
claims are ignored), so no trust hole is opened. Point nginx at the
*remote* port (39152, the password door) — never at 39151 (the
token door stays 127.0.0.1-only, for the MCP).

### Verification checklist

1. `docker compose up -d` → container Up; log shows the "Open in a
   browser" banner and no errors.
2. `data/service.json` exists and holds the service token (the MCP's
   credential) — note its location, don't print it.
3. Door, from this box: `curl -o /dev/null -w '%{http_code}' -H
   "Authorization: Bearer <key>" http://<accelerator-IP>:8081/upstream/no-such-model/sdcpp/v1/capabilities`
   → **404** (key accepted, model unknown). Without the key → **401**.
   (A 401 with the key means the key in the config is wrong.)
4. The lab models: the service's `/api/v1/segment` and `/api/v1/matte`
   report `installed: true` (they arrive with a migrated data dir;
   fresh starts download them on first lab use — ~45 MB and ~224 MB).
5. The UI: open `https://<ui-name>` from **another device** on the
   LAN → sign in with the password → the gallery loads.
6. One real generated image (accelerator up): a short text-to-image
   job completes and lands in `gallery/` with a dated folder. This
   proves the whole path: browser → nginx → service → gateway over
   the LAN → engine → back.
7. While that job runs, the UI shows live progress (the SSE stream
   through nginx — if progress only appears after the job finishes,
   `proxy_buffering` is still on somewhere).

### Migrating the existing data (receiving side)

The accelerator-side agent packs (with the service stopped there):

```sh
tar czf kokon-move/data.tar.gz   -C /var/lib/docker/volumes/kokon_kokon-data/_data   .
tar czf kokon-move/config.tar.gz -C /var/lib/docker/volumes/kokon_kokon-config/_data .
tar czf kokon-move/gallery.tar.gz -C /var/lib/docker/volumes/kokon_kokon-gallery/_data .
```

(`kokon_*` is the compose project prefix on that box.) Unpack into
`<data>/{data,config,gallery}`, `chown -R 1000:1000`, start. The
tarballs carry the gallery database, the config, the service token
and the login password — after a successful migration the
accelerator's volumes are the rollback: leave them in place until the
owner is happy, then delete them.

### The MCP

If an agent session on this box uses the Kokon MCP: install it from
the repo's `mcp/` directory (official TypeScript SDK, stdio); its
credential is the service token from `data/service.json`. A client
that runs elsewhere cannot use the MCP across boxes (the token port
is loopback-only by design) — the MCP lives where the service lives.

### Day-to-day

- **Accelerator down** (OS/CUDA/driver work): the UI and logins keep
  working here; job requests fail fast (~3 s) into the visible
  "engine unreachable" state; when the accelerator is back, the next
  job re-adopts the gateway. In-flight jobs during the outage are
  lost — re-submitting works. First job after recovery pays one cold
  model load (same as any 10-minute idle unload).
- **Updating the service**: clone main, rebuild, change the tag in
  the compose, `docker compose up -d`. The data dirs are untouched.
- **Password change**: `docker exec -it kokon kokon-service
  --set-password` — no restart; every browser signs in again.
- **Two users today share one password and one gallery.** Per-user
  accounts and galleries is tracker task-026 — it slots into exactly
  this topology when it happens.
