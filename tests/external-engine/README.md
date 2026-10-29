# External engine integration test (deployment-local)

> **Not for upstream.** This harness targets *this deployment's* gateway
> (llama-swap's `/upstream/<model>` proxy) with *its* model ids; the Rust-side
> behavior it exercises is covered generically by the unit tests in
> `crates/qls-core/src/process.rs`.

End-to-end check of the `externalEngine` mode: a `kokon-service` configured with an
external gateway (a mock of llama-swap's `/upstream/<model>` proxy) instead of a
locally spawned sd-server.

What it proves (16 assertions):

1. **rest** — a bare service start (what visiting the UI does) preloads nothing:
   zero requests reach the gateway, state is `stopped`.
2. **load** — `POST /api/v1/backend/start` asks the gateway to load the model;
   the service adopts the running engine as `owned: false`.
3. **job** — a real generation job travels
   `{gateway}/{model}/sdcpp/v1/img_gen` → `jobs/{id}` and completes.
4. **switch** — `POST /api/v1/models/flavor` re-points the service at a different
   flavor: only the config changes, the gateway is untouched.
5. **generation** — the next job triggers the swap: the gateway evicts the old
   tenant, loads the new model, the job completes and the service reports it.
6. **gateway down** — with the gateway killed, a start request lands in the
   visible error state (no local spawn, `owned` stays false).
7. **recovery** — the gateway comes back; an explicit start re-adopts it.

Run (Linux, after `cargo build -p qls-service`):

```sh
QLS_BIN=$PWD/target/debug/kokon-service ./tests/external-engine/run_test.sh
```

The mock listens on 127.0.0.1:39990, the service on 127.0.0.1:39902; both are
killed on exit. Everything happens on loopback in a throwaway folder
(`/tmp/qls-external-test` by default, override with `QLS_EXT_TEST_DIR`).
