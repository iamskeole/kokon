# Incident 2026-11-11 — "HTTP 401 / did not report its capabilities" + Layers lab

Owner-reported before the next-day demo: every generation since ~21:35
failed with "The inference server did not report its capabilities. HTTP
401 Unauthorized"; and the 0.12 Layers lab didn't appear at all on the
phone, while desktop errored "downloading the segmentation models".

## Root cause 1 — the 0.12 settings save wipes the stored gateway key

- Timeline (service log): jobs succeeded at 21:34:15 / 21:35:10; every
  job from 21:35:25 on got 401 at the door. The container had been up
  since Nov 8 (the 0.12 deploy) and the config still carried the
  48-char key (written by hand in the w024 window).
- The wipe: the 0.12 UI's `ExternalEngine` type has **no gateway-key
  field** (only the per-model VLM sub-entries carry one), so any
  settings round-trip — a model switch (`POST /models/switch` rebuilds
  the active `externalEngine` from the picker entry) or a Settings save
  (`PUT /config`) — deserialized the key as `""` (Rust
  `#[serde(default)]`) and saved it. The first settings-touching action
  after the 0.12 upgrade (a model switch at 21:35:2x) wiped the key;
  every request after was keyless, and the one-door gateway answered
  401 before routing (its correct behaviour). The VLM keys survived
  (the UI carries those), which is why only the image jobs died.
- The 21:34 successes prove the door itself was fine all night (key and
  paths good); the flip coincides exactly with the config save.

## Fix

1. **Live (no GPU touched):** key restored into `/config/config.json`
   (value from the w024pre backup, sha-identical to the gateway's
   `gateway.key`), container restarted; door re-verified with a
   no-boot probe (keyed → 404 unknown model / keyless → 401).
2. **Code (commit `793ed0a`, main):** `AppConfig::absorb_secrets(from)`
   — `set_config` now keeps the stored gateway key and each known
   model's VLM key whenever the incoming values are empty (an explicit
   non-empty key always wins). Both wipe paths run through
   `set_config` (`PUT /config`, `POST /models/switch`), so one choke
   point covers both. Unit test
   `settings_save_round_trip_keeps_gateway_and_vlm_keys` (keep /
   explicit / none). Green: qls-core 376 / service 18 / UI 228 /
   mcp 35+3sk / mock-gateway 16/16. **Live regression on the deployed
   image: a `PUT /config` carrying an empty key keeps the stored key**
   (file and memory both).
3. **Deploy:** image `external-engine-0.12.0-793ed0a` built (local-only
   per the box's compose convention — "Not pushed to a registry"),
   `frontends/kokon/compose.yaml` switched (backup
   `compose.yaml.bak-793ed0a`), container recreated, all checks green
   (config load, door, lab models, clean boot log).

## Root cause 2 — Layers lab: not broken, two separate facts

- **Mobile:** 0.12 upstream deliberately excludes the lab from the
  phone UI (`PhoneApp.tsx`: "No Layers lab on a phone … its layers are
  only looked at") — the button never appears in a narrow window. Not a
  fork defect; 0.12 behaviour wins. **Demo: use a wide window.**
- **Desktop "download error":** the lab's click-to-select model
  (MobileSAM, two ONNX files, 44.6 MB total, pinned in `matting.rs`)
  had never been downloaded on this box (w024's manual completion
  covered only the 224 MB BiRefNet "fast sticker" model). The service's
  `/segment` status is file-based, so the UI offered a download nobody
  had completed. Fixed the canonical way:
  `POST /api/v1/segment/download` (hash-verified; both files now in
  `/data/sdcpp/models/segment/`; `/segment` → `installed: true`).

## Residual / watch

- **ghcr push:** this box's `gh` token (scopes incl. `repo`, `workflow`
  — no `packages`) cannot push to ghcr; the deployment is local-image
  so nothing is blocked. If the owner wants the tag on the registry:
  `echo $TOKEN | docker login ghcr.io -u iamskeole --password-stdin &&
  docker push ghcr.io/iamskeole/kokon:external-engine-0.12.0-793ed0a`.
- **openssl on this host:** homebrew openssl is now `@4` (the protocol
  notes say `@3`, which is gone). Cargo needs
  `OPENSSL_DIR=/home/linuxbrew/.linuxbrew/opt/openssl` +
  `PKG_CONFIG_PATH=…/lib/pkgconfig`, and the 0.12 ONNX runtime
  additionally `LD_LIBRARY_PATH=/home/linuxbrew/.linuxbrew/opt/openssl@4/lib`
  (its build script links the brew shared lib).
- **The 48-char box key appears in this night's transcript** (config
  dumps made while working). Optional: rotate it (it lives in the
  gateway yaml + `gateway.key`, the service config, the pi clients and
  Open WebUI).
- The two rebase watch items from the Discussion #52 reply stand:
  idle-unload vs the gateway's TTL in external mode, and his remote
  story vs our reverse-proxy login.

## blockers

None — resolved the same night; the box is demo-safe (no GPU work was
done; vllm never evicted).

## decisions

- The key-wipe defence lives in the service (`set_config`), not in the
  UI: the merged 0.12 settings page stays as-is (0.12 wins on
  conflicts), and a single choke point covers every save path.
- The lab models were completed through the service's own download
  flow (canonical, hash-verified) instead of a manual file copy (the
  w024 workaround): the box has working egress to huggingface.co.
