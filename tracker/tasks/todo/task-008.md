# task-008 — service image build + `frontends/kokon` deploy + e2e smoke

## goal
Build the service image (quiet window — a container restart kills web
sessions) and deploy it into the host's `frontends/kokon/` (compose + env
pointing at the gateway; the dir is currently empty). End-to-end smoke of
the new wiring: UI → service → **sidecar PUP** → **stock engine**.

## acceptance criteria
- Image built + tagged (tag convention confirmed at execution), container
  running from `frontends/kokon/`.
- One klein t2i end-to-end: the caption path provably went through the
  sidecar (service log / job metadata), the render path through the engine;
  the owner validates the image visually.
- Status pill + metadata bar show the real values (the earlier fix
  `47c87a9` stays in force).
- The service token is read at runtime from the container (`/data/
  service-token`) — never written to any file or log.

## blockers
- task-005, task-006, task-007 (the wiring this image ships).
- A quiet window (no active owner web sessions).

## decisions
- Image tag convention (e.g. `0.12.0-dev.N`) confirmed at execution.
