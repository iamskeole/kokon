# Task-026 — multi-user: per-user accounts + per-user galleries on the existing single-user service

**Created:** 2026-11-11 — the owner, anticipating demo questions: the
service is deliberately single-user (one shared login; all users share
one gallery and see each other's work in the client UI). He wants users
unbundled: auth + user-related assets per user, in SQLite (Postgres
"eventually", though for two users SQLite should be enough).

**Trigger fired 2026-11-12** ("do task 26 pls" — the owner is driving
the day; autonomous run, no GPU window: the acceptance is unit tests +
the mock-gateway harness + API-level checks on the box).

## Trigger (when this stops being parked)

The owner says go — plausibly the day after the demo, if the "what if
two people use it?" question lands as expected.

## Where things stand (verified 2026-11-11, 0.12 code — the base is deeper than it looks)

- **Two identities today:** (1) the Bearer token (`service.json`) = the
  machine itself (desktop app, MCP, scripts) — accepted everywhere;
  (2) browsers: **one shared password** (Argon2id hash,
  `remote-auth.json` in the data folder) → a persisted session cookie
  (`SameSite=Strict`, 90-day TTL, sessions file in the data folder,
  per-IP attempt limiter; a password change ends all sessions).
- **The gallery is already SQLite:** `gallery.db` (rusqlite, bundled
  in the tree): a `generations` table (id, created_at, prompt, mode,
  favorite, file_path, record) plus content-addressed `inputs`. Image
  files live in the gallery directory (`/gallery` volume here; the
  user's Pictures on a stock install).
- So unbundling = **adding a user dimension to what exists**, not a
  new storage stack. Zero new dependencies (rusqlite + argon2 are
  already in the tree).

## Design sketch (final decisions at start)

1. **Users** — a `users` table (id, name, Argon2id hash, created) in
   the existing data folder. The current shared password becomes the
   **operator** credential: it keeps working exactly as today (and
   stock behaviour stays byte-identical while the feature is off),
   and from an operator session named accounts are created.
   (Alternative: first account = admin — decide at start.)
2. **Login** — the existing `POST /auth/login` stays password-only
   (operator); adding a `username` selects a named account. Sessions
   gain a user association in the existing persisted session store.
3. **Gallery scoping** — `generations.user_id` nullable (NULL =
   legacy/shared/operator). Named users see and save only their own
   rows; the operator sees everything (with a filter). Files: per-user
   subfolders under the gallery directory in multi-user mode (each
   person's "Pictures folder"); the DB is the source of ownership.
   Single-user mode's layout is untouched (the stock path stays intact).
4. **Shared by design:** settings (models, engine, gateway key), the
   lab models, upscalers — machine-level, not per-user.
5. **Jobs** — per-user visibility of the transient queue is a
   follow-on; the gallery is the durable record.
6. **DB backend** — SQLite now (bundled; one file in the existing
   volume, no migration pain); a repository trait for a later Postgres
   only if it ever earns its keep — not a commitment for two users.
7. **MCP / scripts / token** — unchanged: the token stays the
   machine/operator identity.

## Upstream relation + the demo answer

Single-user is the upstream vision; this is **fork-only additive**
(config-gated, off by default; the stock path byte-identical).
Upstream won't take it — D-RB, the fork is the product.

**Demo wording:** "Today it's one shared password and one shared
gallery — deliberately single-user, like the product. But the gallery
is already a SQLite database with hashed logins behind it, so
per-user accounts with their own galleries is a planned fork feature
that sits on what's already there, not a rewrite."

## Effort

Stage 1 (users + login + gallery scoping + the UI listing): a few
days; no GPU window needed (pure service + UI logic — unit tests, the
mock-gateway harness, and a live two-browser check on the box).
Stage 2 (per-user jobs/history, the MCP mapping). Stage 3, only if
ever wanted: per-user settings, Postgres behind the trait.

## Reopened 2026-11-13 — the projects leak

The owner's live report: a logged-in (named) user sees other people's
projects even though they see none of the images. The cause is the design,
not one place: D-MU8 deferred per-user projects to Stage 2 and left
`<data>/projects.json` machine-level, so a named account's browser lists
and writes the one shared project list (on the box: 9 projects, the
operator's own work) while D-MU5/6 404 every image inside it. Titles
without images is worse than nothing — and a named account's `import` /
`put` could merge into another's project and, through a stuffed `gen:`
reference in its items, read that person's records from
`GET /projects/{id}/records`. D-MU13 closes it (per-user projects, the
gallery's semantics). The task stays open until the fix is built, proven
and on the box.

## decisions (2026-11-12, at start)

- **D-MU1 — Three identities.** (1) the Bearer token = `Local`: the
  machine itself (desktop app, MCP, scripts) — the operator, full API,
  unchanged; (2) a browser signed in with the shared password (no name)
  = `Operator` — sees every gallery row and job, manages the named
  accounts; (3) a browser signed in *with a name* = `User(name)` — sees
  and changes only its own rows and jobs. One session cookie either way
  (the existing store, extended).
- **D-MU2 — Storage: the existing SQLite, extended.** A `users` table in
  `gallery.db` (`id` UUID, `name` unique case-insensitively, Argon2id
  `password_hash`, `created_at`); `generations` gains a nullable `user`
  column (the owner's *name*; NULL = operator/legacy — the stock rows).
  The name, not the UUID, is the ownership key everywhere (sessions,
  records, the column): there is no rename (D-MU7), so the name is
  stable, and it keeps one identity instead of two. The session file
  (`remote-sessions.json`) entries gain an optional `user` name — a bare
  expiry number stays valid (the operator), so an existing file reads
  without migration; the `GenerationRecord` gains `user` (the same name;
  it lands in the PNG provenance too). Zero new dependencies (rusqlite +
  argon2 are already in the tree).
- **D-MU3 — The gate.** A config field `multiUser` (schema 15,
  additive, default false). Off: behaviour is exactly today's (a login
  that carries a name is refused with a clear error; no scoping; the
  `/users` endpoints do not exist). It is set only by
  `POST /users/enable` (operator); every config save preserves the
  stored value (`Engine::set_config`), so a UI round-trip cannot wipe it
  (the 2026-11-11 incident pattern).
- **D-MU4 — Login.** `POST /auth/login` keeps `{password}` for the
  operator; `{password, username}` picks a named account. A blank or
  absent name = operator (the stock path, byte-identical). With a name:
  unknown name → "Unknown name" (names are not secret on a LAN and the
  per-IP limiter caps enumeration at five strikes per lockout); wrong
  password → same per-IP limiter, "Wrong password". One session created
  either way.
- **D-MU5 — Gallery scoping.** Named users list/get/patch/delete only
  rows whose `user` is their own name — anything else is 404 (no
  existence leak). The operator (and the token) sees everything, with an
  optional `?user=` filter: `all` (default), `me` (the NULL rows) or a
  name. File layout: a named user's results land in
  `<output_dir>/<name>/<date>/` (the name is validated to a
  filesystem-safe charset at creation; the subfolder is their "Pictures
  folder"); the operator keeps `<output_dir>/<date>/` exactly as today,
  and single-user mode's layout is untouched. The DB is the source of
  ownership.
- **D-MU6 — Jobs.** `Job`/`JobView` gain `user` (the owner's name; null
  = operator/token job). In multi-user mode a named user lists/
  gets/waits/cancels only its own jobs (others 404) and the `/events`
  stream drops other people's `job` events (gallery events pass
  through — the UI refetches its own scoped list; the job event is what
  drives the result reveal, and that is theirs). This closes the sketch
  item 5 (per-user transient queue) inside Stage 1, because the UI
  cannot show a result it may not read.
- **D-MU7 — User management (operator or token only).**
  `GET /users`; `POST /users {name,password}` (name 1–20 chars of
  letters/digits/space/`.`/`-`/`_`, unique case-insensitively — the same
  charset is filesystem-safe for the subfolder; password ≥ 8 chars,
  Argon2id, same minimum as the shared password); `DELETE /users/{id}`
  (cascades: the user's gallery rows + their files + their sessions);
  `POST /users/{id}/password {password}` (reset; ends the user's
  sessions); `POST /users/enable {on}` (the D-MU3 gate). No rename
  (delete + recreate — the sessions store the name, so a rename would
  orphan them).
- **D-MU8 — What a named user may not do.** Manage users; anything
  outside the existing `browser_allowed` list (shutdown, path-taking
  endpoints, model scanning, ...). `PUT /config` stays allowed exactly
  as today: settings are machine-level and shared by design (sketch
  item 4) — the same `keep_computer_settings`/`absorb_secrets`
  restrictions that already hold a browser today apply unchanged. Jobs
  from a named user may reference `gen:` results of their own (the
  engine checks at submit) but not others' (404-style refusal);
  `input:` uploads stay shared (content-addressed, unguessable) —
  per-user inputs are a Stage 2 item, like per-user projects.
- **D-MU9 — Session semantics.** Changing the shared password ends
  every session, named users' included (today's behaviour, unchanged);
  resetting a user's password ends that user's sessions; deleting a
  user ends theirs. The 90-day TTL, the 200-session cap and the per-IP
  limiter apply as today.
- **D-MU10 — MCP / scripts / token: unchanged** (the token is the
  machine/operator identity; per-user MCP mapping is Stage 2).
- **D-MU11 — UI.** The login screen shows an optional name field only
  when the service reports multi-user on (the stock screen is
  byte-identical); the gallery gives the operator a Who filter
  (Everyone / Me / each user) and a small owner tag on rows that have
  one; Settings gains a People tab (visible when a remote password is
  set — without one no one else could sign in; hidden from named
  users). Named users get the settings screen as today except People.
- **D-MU12 — Proof without a GPU.** The `tests/external-engine`
  harness grows a multi-user run (the real service + the mock gateway +
  two browser identities through cookie jars: login both ways, a job
  each, gallery + file scoping, cross-user 404s, the job filter); on
  the box the check is API-level (logins, users CRUD, seeded rows,
  scoping, the UI page) with no engine job — the owner is driving and
  vllm is resident, so the day's live proof stops short of a
  generation (recorded here as the residual).
- **D-MU13 — Per-user projects (2026-11-13; D-MU8's Stage-2 item, pulled
  forward because the leak is live).** `projects.json` gains an `owner`
  map (project id → named account; absent = the machine's shared project,
  the pre-multi-user list; the field is skipped while empty, so a
  single-user file stays byte-identical). Ownership is decided by the
  service from the caller only — never from a request field — and is
  preserved across every merge. A named account sees, creates, edits and
  deletes only projects whose owner is its name; anything else (shared,
  another's, absent) is the stock 404 (no existence leak, D-MU5). The
  operator and the token see everything — the shared list is exactly
  today's for them, and the single-user path (no named account can exist
  while the gate is off) is byte-identical. `Project`'s wire shape is
  unchanged (the owner never travels in it) — the UI is caller-transparent,
  no client change. `GET /projects/{id}/records` gives a named caller only
  the rows it owns (a project's items are a client-controlled list of
  references — a stuffed `gen:` id must not surface another's record; the
  inputs stay shared, D-MU8; the UI already skips versions whose record is
  gone). `Event::Projects` carries the owner, and a named account hears
  only its own projects' changes (as with its jobs, D-MU6), so its own
  second devices still live-update. The first-launch import (a browser's
  pre-0.11 local list) is namespaced by the caller: a named import
  merges/creates only its own, and ids it may not see are skipped,
  untouched. `POST /examples` (the first-launch tour) seeds its gallery
  rows with the caller's owner — a named tour is private, a fresh phone
  gets its own samples; already-present rows (idempotent) keep their
  owner. Proof: the 50-assertion harness grows a project block (per-user
  create; cross-user 404s on get/put/delete/records; the
  stuffed-reference filter; the import namespacing; the event filter),
  and the box check does the same against its 9 shared projects.

## progress

- 2026-11-13 (reopened, the owner driving): **the projects leak fixed per
  D-MU13 — done the same day, all-CPU.** Commits `631eee8…7c3cb37` (the
  store's owner map + scoped put/get/delete/import + the account sweep;
  the scoped records + the event's owner + the per-caller examples; the
  caller-scoped handlers + the stream filter + the users-delete sweep +
  the new `projects_are_per_user` security test; the harness's projects
  section). Green: qls-core **406** (incl. the new ownership tests, among
  them that a single-user file never gains the owner key), service **20**
  (incl. the new projects test), UI **231** unchanged (the wire shape
  moved nothing), mcp green with the host's openssl env, the multi-user
  harness **67/67** (17 new: per-user create/list, the cross-user 404s
  on get/put/delete/records, the stuffed-reference filter, the import
  namespacing, the stream filter, the account-deletion sweep), the
  single-user harness **16/16** (the stock path). Image
  `multiuser-7c3cb37` on the box in 150 s (job `/tmp/mu7c3cb37/`,
  rollback `compose.yaml.bak-7c3cb37` = the 1899739 compose): the 44 D-MU12
  checks + the 15 live project checks passed **59/59** against the real
  state as found (the owner's account `p`, the gate on, the 9 shared
  projects) — a fresh named account now sees **no** projects (the
  complaint), sees and creates only its own, the shared list and
  another's are the identical 404, a stuffed reference gets no record
  for its author, the operator sees everything, and deleting the test
  accounts swept their projects with the owner key gone from
  `projects.json` again; vllm never evicted (same pid/RAM, the engine
  doors idle as found; one `check26-p` tombstone stays in the file's
  deleted map — the design, same as any single-user delete).

- 2026-11-12 (autonomous run, the owner driving):
  - **Core (all D-MU decided and done):** `store.rs` — `users` table +
    user CRUD (create/list/by-name/by-id/verify/set-password/delete,
    `delete_user_rows` cascade) + `generations.user` column +
    `OwnerFilter`/`ListQuery.user` scoping; `remote.rs` — `SessionEntry`
    (bare `u64` operator / `{user,exp}` named) + `owner_of`/`remove_user`/
    `clear_named`; `config.rs` — `multi_user` (schema 15, preserved on
    every save); `engine.rs` — `JobView.user`, submit/`preview_plan`
    share `check_source_ownership`, `apply_lut`/`save_layers` take the
    owner, per-user output folder `<output_dir>/<name>/<date>/`,
    `set_multi_user`.
  - **Service:** `Caller { Local, Operator, User(name) }` in the auth
    middleware; login handles named users (gate check, distinct
    "Unknown name."/"Wrong password.", the per-IP limiter counts both);
    `/auth/session` reports `user` + `multi_user`; the user-management
    endpoints (D-MU7) guarded by `require_operator`;
    **job scoping (D-MU6): `list_jobs`/`get_job`/`wait_job`/`cancel_job`
    filter for named callers (another's job = "Unknown job.")** in
    addition to the `/events` job-event filter; every gallery handler
    (`list/get/patch/delete/image/original/thumb/preview/export`,
    `save_layers`, `luts apply`) runs `ensure_sees` (a wrong-owner row is
    the stock "That image is no longer in the gallery.").
  - **Tests:** qls-core 404 ok; qls-service 19 ok (new
    `named_accounts_see_only_their_own_rows`: both named logins, the
    session's identity, the distinct login errors, per-owner gallery
    scoping with fake rows, cross-user 404s (row/thumb/image/patch/plan),
    the operator's `?user=` filters, the 403 on user management, the
    delete cascade (rows + session), the gate off (named sessions end,
    named logins refused, the operator's survives)); UI `vitest` 231 ok.
  - **UI:** `Login.tsx` (the name field only when `multi_user` — the
    stock screen unchanged); `Gallery.tsx` (the Who filter for the
    operator/desktop — Everyone/Me/each name — and the small owner tag
    on tagged rows; named users get neither); `Settings.tsx` + new
    `People.tsx` (the People tab: visible when a password is set and
    the viewer is not a named account — the on/off switch, create,
    new-password, delete); `api.ts` (the users endpoints, the login
    name, the `?user=` gallery param, `session()` type) + `UserRecord`
    in `types.ts`.
  - **Proof (D-MU12, no GPU):** the new
    `tests/external-engine/run_multiuser_test.sh` — real service + mock
    gateway + two cookie-jar identities — **50/50 pass** (both logins,
    one job each, per-user job list/get/cancel/wait 404s, the SSE stream
    carrying own and dropping the other's job events, gallery + file
    scoping to the owner's folder, cross-user 404s, the `?user=`
    filters, the 403 on user management, the delete cascade including
    files off disk, the gate off). The original single-user external
    harness still passes 16/16 (its config bumped to schema 15).
  - **Box check (D-MU12, done the same day, no GPU):** the box took the
    image `ghcr.io/iamskeole/kokon:multiuser-e59cb70` (rollback in
    `compose.yaml.bak-e59cb70`) and **40/40** passed against the live
    service: the real 88-row gallery migrated to schema 15 untouched
    (every row ownerless, every file present), gate off → named logins
    refused, gate on → two test accounts signing in (with the distinct
    wrong/unknown errors and the named account refused user management),
    a seeded row readable by its owner (row + thumb) and 404 to the
    other account, the operator's `?user=` filters, and cleanup
    restoring the exact baseline (rows, files, users, gate, sessions).
  - **Field fix (2026-11-13, `b1f2251`, after the owner's first live
    look from a phone):** "Add did nothing" = the button was genuinely
    disabled (`!newName.trim() || newPassword.length < 8`) and the UI
    never said the 8-character rule; "greyed out" = the app-wide
    `button:disabled { opacity: 0.4 }`. The exact browser replay (login
    by shared password → cookie + `x-kokon-csrf: 1` → POST
    `/users/enable` → POST `/users`) passed end-to-end on a scratch
    instance, so the request path was never the problem. The People
    form now stacks its text and fields at every width, the subtitle and
    the placeholder state the minimum, the per-user rows stack under
    560px, and the new-password button holds 8 (was a stray 6) to match
    the server. The box runs `multiuser-b1f2251` (rollback
    `compose.yaml.bak-b1f2251`); the 40-check suite re-passed (38/40;
    the two deltas: the owner's gate was on, so "starts off" and
    "named login refused" read differently — and the empty users table
    confirmed the owner's own add attempts had created no account).
  - **CSS field fix (2026-11-13, `1899739`, the phone's second look):** the
    field fix's own comment block in `src/styles.css` was opened with `/*`
    and never closed, so the CSS minifier read it as one comment running to
    the next `*/` (the overlays heading) and dropped everything between —
    the `.people-form` / `.people-row` stacking plus the block's `.input`,
    `.btn`, `.muted`, `.switch` and `.seg` bases. No build error: to the
    parser it was just a comment, so the image built and the 40-checks
    passed (they are API-level); only the phone's render showed it (one
    word per line, browser-default controls).
  - **Image closeout (same day, autonomous job `mu-1899739`, all CPU —
    vllm never touched, digest in `/tmp/mu1899739/`):** the box now runs
    `ghcr.io/iamskeole/kokon:multiuser-1899739` (rollback
    `compose.yaml.bak-1899739` = the b1f2251 compose). Proof, all at the
    bytes a browser receives: the served CSS of the new image carries
    `people-form` ×2 + `people-row-fields` + the `.muted` / `.switch`
    bases, while the same probe against the old b1f2251 image (a scratch
    container on scratch ports) shows `people-form` = 0 — the regression,
    then the fix. The gate-aware 44-check box suite (the 40-check D-MU12
    suite adjusted to the owner's gate-on state + 4 served-CSS checks)
    passed **44/44 with zero deltas** — it restores the gate to as-found
    instead of forcing it off, so the owner's sessions were never at risk.
    The 50-assertion multi-user harness re-passed **50/50** against the mock
    gateway (host debug build; the job had to supply the linuxbrew
    openssl@4 env — `PKG_CONFIG_PATH` + `LD_LIBRARY_PATH`, what
    `brew shellenv` in `.profile` gives the interactive shells — because
    the debug binary links against `libssl.so.4`; the pre-existing 09:50
    debug binary is linked the same way).

## blockers

None.
