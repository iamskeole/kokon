# Task-026 (PARKED — owner-triggered) — multi-user: per-user accounts + per-user galleries on the existing single-user service

**Created:** 2026-11-11 — the owner, anticipating demo questions: the
service is deliberately single-user (one shared login; all users share
one gallery and see each other's work in the client UI). He wants users
unbundled: auth + user-related assets per user, in SQLite (Postgres
"eventually", though for two users SQLite should be enough).

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

## blockers

None. Parked on the owner's trigger.
