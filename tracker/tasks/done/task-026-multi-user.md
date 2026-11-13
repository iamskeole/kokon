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
  `password_hash`, `created_at`); `generations` gains a nullable
  `user_id` (the users.id of the owner; NULL = operator/legacy — the
  stock rows). The session file (`remote-sessions.json`) entries gain an
  optional `user` name — a bare expiry number stays valid (the operator),
  so an existing file reads without migration; the `GenerationRecord`
  gains `user` (the display name; the column carries the id). Zero new
  dependencies (rusqlite + argon2 are already in the tree).
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
  rows whose `user_id` is theirs — anything else is 404 (no existence
  leak). The operator (and the token) sees everything, with an optional
  `?user=` filter: `all` (default), `me` (the NULL rows) or a name. File
  layout: a named user's results land in `<output_dir>/<name>/<date>/`
  (the name is validated to a filesystem-safe charset at creation; the
  subfolder is their "Pictures folder"); the operator keeps
  `<output_dir>/<date>/` exactly as today, and single-user mode's
  layout is untouched. The DB is the source of ownership.
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

## progress

- 2026-11-12 (autonomous run, the owner driving):

## blockers

None.
