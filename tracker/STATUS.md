# STATUS — kokon v2 separation

Updated 2026-11-12 (**task-026 multi-user implemented** — named accounts + per-user galleries on the existing single-user service: the core (`users` table, session owners, the `multiUser` gate at schema 15, per-user job ownership and output folders), the service (the `Caller` identity, named logins with distinct errors, the user-management endpoints, per-user scoping of gallery + jobs + the SSE stream), and the UI (login name field, the gallery's Who filter + owner tag, Settings → People) — all fork-only, config-gated, off by default (the stock path stays byte-identical); green: qls-core **404** / service **19** (new `named_accounts_see_only_their_own_rows`) / UI **231** / the new **50-assertion** mock-gateway multi-user harness + the original 16; **box check 40/40** on the deployed image `multiuser-e59cb70`, no GPU, gallery back to baseline; 2026-11-13 field fix (the People form: the password rule made visible, the mobile squeeze gone) out as `multiuser-b1f2251`); **task-028 ADOPTED — the native PE doors are live (owner verdict ADOPT-NATIVE-ON); the schema-14 cutover is applied and the one-step rollback is staged**: step 5 (`ae971a8…fedbcb5`, six commits; image `external-engine-0.12.0-fedbcb5` live, box behaviour unchanged until the cutover): `vlmVision` endpoint on the engine + each model (**schema 14** — 13 loads as-is, saves as 14), the mode-routed PUP client (t2i jobs → the pipeline's `vlm` door; every image-grounded mode → `vlmVision`), the native `pup-qwen-native` bundle (both fine-tunes' `system_prompt.txt` + Qwen `LICENSE` in the data dir — zero prompt bytes in the image or committed bundles; the native system prompt wins over a saved custom one, D-PE11), the JSON-envelope decode of the PE doors' `{rewritten_prompt, wh_ratio | ratio_follow}` replies (one warm retry, then raw-keep-with-note, D-PE9), one-shot native compose (D-PE13), and the UI's quiet "Suggested shape" hint (D-PE12); green: qls-core **399** / UI 231 / mcp 35+3sk / mock-gateway 16/16. Step 6 (windows **w028r3** 317 s + **w028r3b** 150 s, ~7 min of GPU; the box restored byte-identical after each — gateway, vllm on 9108, service config): **7 A/B jobs, same prompt + seed 1234 per mode** — arm A (BFL door + `pup-qwen`, the box as it stood) vs arm B (native PE doors + native bundle, **thinking on**: t2i 1303 tok/21.2 s, 31 % longer prompt, ratio **3:2**; edit 1701 tok; compose **one call** with 2 images vs arm A's three — D-PE6 decided on evidence) vs B-off (**thinking off**, scratch boot: 441 tok/6.2 s, same 3:2) — 7/7 in-contract on the first call, zero retries, zero think-leaks, zero D-PE9 notes; the pixel-identical duplicate run (D-AD10) proved the images' differences are purely prompt-driven, so the verdict reads as prompt quality. **Adoption applied (same-day evening):** the live config is the staged schema-14 native wiring (md5 `4baa5379…` — turbo/full on the native doors + `pup-qwen-native`, klein unchanged), the pre-adoption bytes preserved as `config.json.pre028` — **rollback = one `cp` back + `docker restart kokon` (the window's restore path, demonstrated twice, D-PE8).** **The verdict (a vision session):** t2i b > boff > a, edit a ≈ b, compose b > a → ADOPT-NATIVE-ON (D-PE18) — thinking **on** stays the default. Same evening on the host (outside this repo): the vision vllm door's encoder OOM (17:23/17:25 fatals) fixed per D-PE19 — `longest_edge` 4194304 → 1048576 (1 MP) in `serve-qwen3.8-27b-vision.sh`, KV budget untouched; proven with a 39k-token + 3-image request (200, no fatal). Earlier the same day: **step 4** — the two native PE doors in the gateway (registry now **10 models**: `-t2i` **9105** text-only / `-i2i` **9104** mmproj-bf16; qwen sets widened to the mode-exclusive group `qt & (p|pt|pi)` / `qf & (p|pt|pi)`; window w028r2e landed step 2's remaining proofs in 225 s — boots, mmproj-less T2I image-rejection, the thinking switch both ways, the JSON contract on both doors — and its four checker-side failures are the D-AD1..D-AD5 window-builder lessons) and **step 3** — the shared PUP door is now `kokon-pup-flux-image-3-klein` (rename window w028r1d in 150 s; legacy `q35`/`qwen3vl-8b` survive as extra aliases). Step 1: both Qwen PE Q4_K_M GGUFs + the I2I mmproj pinned with provenance (plus, landed with step 4, the PE contract bundle — `system_prompt.txt`, `generation_config.json`, `LICENSE` — pinned to the model dirs, referenced never inlined). **Also 2026-11-12 (earlier): dev-box relayout executed + Prompt+ button fixed**: the service deployment moved to the bind-mounted shape at `/home/p/ml/frontends/kokon/` (compose + `data/`/`config/`/`gallery/` beside it; old named volumes kept as rollback; gallery fresh — owner dropped the test assets; carried state verified: old token, door, lab models, clean boot); the owner then found the **Prompt+ button missing** — root cause was the morning's model switch, not the relayout: `POST /models/switch` replaced the active engine wholesale and the klein picker entry carried neither a VLM nor a bundle, so that pipeline had no enhancer at all, and `/status.text_generation` (engine's own caps — stock engines have none) reported false and hid the switch (the button had only been visible on models with no cached caps, via the browser's remembered value). Fixed in `9f4eafb`: `/status` reports the enhancer as available whenever the active pipeline has a VLM (the same two sources as the job routing), `absorb_secrets` now inherits a stored VLM across a vlm-less switch and keeps a stored same-family bundle, and the live config gives all three pipelines the PUP VLM + their family bundle; image `external-engine-0.12.0-9f4eafb` live on this box, all checks green (record: `done/incident-2026-11-12-promptplus-hidden.md`); **second glitch the same morning — a replacement run on qwen-full then the same on klein came back with metadata reading "full" for both**: the pipeline was right (gateway journal: the matrix evicted the qwen set and job two ran on klein in 14 s) but the record's `model` field copies the engine's self-reported name, and both Q8 tenants serve a file named `model-Q8_0.gguf`, so the metadata was identical and the UI's name-matched label read both as Full. Fixed in `a907258` (current build): records also carry the gateway tenant the job was sent to, the parameters text and the details panel / viewer label resolve from it (reported name kept as before), and this morning's two images were back-filled with their actual tenants from the journal. **Task-027 prod-box half still PENDING on owner access to the stable box** (the doc's build step now names `a907258` as the current one). Earlier 2026-11-11: the 0.12 settings-round-trip key wipe (HTTP 401) root-caused and fixed in `793ed0a`; the lab's MobileSAM pair installed; the box was demo-safe otherwise; task-024 CLOSED — the 0.12 rebase is DONE on both branches and green; the w024 window ran with the owner's matrix rename in it (P0–P4 green in-window; the matte's 0.12 download-gate miss completed post-window CPU-only with vllm resident); the box was on the final image `external-engine-0.12.0-b77d049` with the renamed config, and the harness is aligned to the rename; the Discussion #52 reply stands (2026-11-10: the mode declined on the one-model vision, the fork acknowledged as the product); two rebase watch items noted (idle-unload interaction, his remote story)**: `flux` (110 commits, 13 conflict stops, all final-fork semantics — `enhancerApplies ≡ true` for the sidecar's area/Expand prompts, model-aware `needsPrompt`, bundle-routed `system_prompt`) and the PR branch re-derived onto upstream master `f9b6971` as **`pr-external-engine-012`** (the 5 signed commits `ff372fc…e141d35`; old name retired) — green on both: qls-core 375 / service 18 / UI 228 / mcp 35+3sk / mock-gateway 16/16 (harness refreshed to `/models/switch` + schema 13); w024 window done, final image and fresh-box pass done — and the last gap closed too: **the proposal is out as [Discussion #52](https://github.com/GabrieleLozupone/kokon/discussions/52) (Ideas) on the author's repo, awaiting the author's reply**; `todo/` now holds only task-023, parked on the owner's trigger. Also 2026-11-12: **task-028 created** from the owner's question on Qwen's native PE pups (`Qwen-Image-2.1-PE-T2I` / `-PE-I2I`, fine-tuned Qwen3.5-VL 9B, released 2026-09-20) — investigation done: Q4_K_M GGUFs exist (prithivMLmods; I2I with mmproj, T2I text-only), the Oct-14 llama.cpp tree already carries `qwen35.cpp` and serves BFL's Qwen3.5-9B PUP, and the design is two qwen-native mode-exclusive PUP doors (t2i jobs → PE-T2I, image-grounded jobs → PE-I2I, both flavors) with the native `system_prompt.txt` as a third-party bundle, JSON `rewritten_prompt` parsing, and klein untouched on its BFL door. **task-022 stays CLOSED** — the one-door, single-key topology is live and proven: the gateway (wildcard `:8081`) is the box's only LAN door for model traffic, keyed with the 48-char box key (no-key 401 / with-key 200 / wrong-key 401; `/health` open keyless by v269 design); **zero fixed engine ports** — all eight models on `${PORT}` daemon-assigned (the v269 contract: assigned by *alphabetically sorted* model ID from `startPort: 9100` — 9100 creative-fast … 9107 qwen3.8-27b-vision), every bind `127.0.0.1`; the vLLM pair carries the explicit mutual-exclusion set `vllm: "v | vv"` (proven both directions: vision evicts text onto 9107, text evicts vision back onto 9106); the kokon service then ran the compose-managed image `external-engine-0.11.2-d93c3c3` (`apiKey` on `ExternalEngine` + the vlm entries; pristine config backup in the volume) with its exact client path proven — door 9104 (both aliases), turbo 9105 (capabilities), klein 9103 (a real 1024×1024 image job, 14 s, PNG back); vLLM resident on 9106 as the terminal state with both stream modes working. The `w022` window ran five times, each one teaching something: r1 did the config swap + keying + daemon onto the wildcard (died on an async-`systemctl`/`ss`-format harness bug), r2 proved the keying and the pair switch, r3 the service cutover (the pre-compose container's missing labels forced a remove-then-create; stops 2 s) + measured stops, r4 exposed that the BFL hybrid has **no** `/v1/completions` (its LLM loads as the prompt conditioner only — clean 404s, never a crash), r5 proved the real image-job path and the terminal state in 75 s. Evidence `/tmp/w022{,r2,r3,r4,r5}/`. **Earlier this span**: tasks 020/021 (the stock-cleanup re-land) and task-019 (one shared PUP door) stand closed; `docs/fresh-box.md` was rewritten in the post-019 shape **before** 022 — its gateway section carried the one-door update with the 0.12 pass (at tip `b77d049`). **Current work = task-017's discussion variant (the owner's go-ahead)**; `todo/` holds only task-023 (parked on the owner's trigger) — the `## Current` section is the pointer.) `[ ]` open, `[x]` done. See [PLAN.md](PLAN.md) for the map.
Keep this file current at the end of every task.

## Decisions (resolved 2026-11-01 — details in PLAN.md §6)
- [x] D-K1 — klein engine = **leejet/stable-diffusion.cpp master** (canonical upstream; day-0 FLUX.3 support since 2026-10-16; one build serves klein + qwen)
- [x] OPEN ITEM — pin + build-verify the specific leejet master commit for the named sdcpp checkout (task-002: `290148d`, done 11-02)
- [x] D-LS — llama-swap pinned at v269, deploy artifacts moved to a non-git sibling (task-001)
- [x] D-QW — shared PUP for qwen: **winner = 9B BFL klein-pup** (A/B done, task-014; turbo door Q8 / full door Q4, `config.yaml.stock-014` live; 8B one step back at stock-013)
- [x] D-FB — VLM down mid-job = hard-fail (making the VLM work is our job)
- [x] D-WIP — in-engine Qwen3.5 port branch deleted (was ff69b0d; rabbit hole, do not follow)
- [x] D-ST — host's `stable/` (vllm) is owner-owned; stays as-is
- [x] D-RB — (owner, 2026-11-06) rebases always target **upstream master**, never a release commit (upstream doesn't tag its releases; master = the release plus post-release work only). **The fork does no releases** — releases are the origin's/author's view (their installers, their tags); if the upstream PR doesn't land, our release mechanism is: clone the fork → `docker build` → `docker compose up -d` (the service image is the product; `docs/fresh-box.md`).

## Gaps (PLAN.md §2)
- [x] G1 per-pipeline config surface: entrypoints + key + prompt bundle, all pointable (task-005)
- [x] G2 prompts as repo data assets — klein BFL set (task-007) + qwen set (task-014, `assets/pup-qwen/`)
- [x] G3 service VLM client (OpenAI chat + image parts) replaces the in-engine `/text_gen` call (task-006)
- [x] G4 production klein = stock sdcpp + PUP sidecar (task-002, 004, 009, 010, 011)
- [x] G5 production qwen = stock build + PUP sidecar (task-012, 013, 014 — stock pin 290148d per the audit; PUP = 9B, task-014)
- [x] G6 gateway: generic N-models-per-job, the "matrix" (task-015 — **v269's native `matrix` router, no upstream PR needed**: `config.yaml.stock-015` live 11-04, sets klein=k&p&v / qwen-t=qt&pq / qwen-f=qf&pqf / small=(cf|cs)&v, vLLM shared across sets; 4,608-case replay 0 violations / all deltas strict supersets; live window proved it in 255 s (protect-vllm kept under the matrix, qwen evicts vLLM, vLLM reclaims the resident qwen pair — the nudge-404 class fixed at the router); one latent race found and fixed in vllm-serve.sh (never double-launch an in-flight boot); deploy-dir doc `MATRIX.md` = how to define a new job's set)
- [x] G7 fresh-box doc (task-016 — `docs/fresh-box.md`: the whole stack from a new machine — host layout, verified prerequisites, the final-state model list with the provenance convention, the stock engine recipes, the v269 gateway with the full matrix routing block, the service image + schema-13 compose, verification checklist, ops rules; no BFL prompt bytes, no fork references)
- [x] G8 upstream PR / merge / philosophy discussion — **closed 2026-11-06** (task-017 done; the proposal is out as [Discussion #52](https://github.com/GabrieleLozupone/kokon/discussions/52), Ideas — awaiting the author's reply): branch `pr-external-engine-012` = upstream master `f9b6971` (0.12) + the 5 DCO commits of the feature in final form (schema-13 config, gateway client, service-side VLM, runtime bundles, picker + box UI, GPU-free mock-gateway test); green on the branch (qls-core 370 / UI 228 / service 18 / mcp 35+3sk); the draft PR body + discussion variant + 4 decision points in `tracker/pr-upstream-body.md` — **executed 2026-11-06 (owner's "go with a")**: the **complete** branch (BFL bundle in — public on the fork from this point; the first time it exists outside this box) is pushed as fork branch `pr-external-engine-012`, and `main` (renamed from `flux` that day) is the fork's **default branch** (everything we have ever worked on is on the fork; the branch list tidied to just `main` + `pr-external-engine-012`, the pre-v2 lines preserved as `parked/*` tags); the publish happened the same day: **Discussion #52 (category Ideas) is live** — posted from `iamskeole` via the web UI (that repo allows any logged-in user to *start* a discussion — the earlier "wall" was API-side only, no collaborator grant needed); body verified verbatim (the corrected same-repo diff link). Awaiting the author's reply: accepted ⇒ the fork re-baselines onto the merged upstream and shrinks to the remainder; rejected on the one-model vision ⇒ the fork is the product (D-RB). **Reply 2026-11-10: declined, exactly as planned — with the author's words "your fork is the right home for that setup"; his next release adds an idle-unload (watch item for the next rebase — in external mode the gateway, not the service, owns the model's lifetime, so the fork keeps its reason to exist). CLOSED — the last gap.**)

## Work packages (PLAN.md §4)
- [x] WP0 housekeeping — task-001 done 11-02 (v269 pin + `llama-swap-deploy/` sibling) / task-002 done 11-02 (`290148d` pinned + built + stock klein verified, findings logged) / task-003 done 11-02 (`models/gguf`+`models/sd-cpp` reorg per the provenance convention, all reference paths updated, config validates 6/6; klein DiT conversion source unresolved — owner question, non-blocking)
- [x] WP1 sidecar + service wiring — task-004 done 11-02 (PUP sidecar: named llama.cpp May build, alias `kokon-pup` on :39175, 1-token readiness, vision + BFL t2i re-verified through the gateway — vllm self-healed after; retry-storm + setsid-pitfall findings logged for 009/010) / task-005 done 11-02 (schema 13: per-pipeline gateway override + VLM endpoint + prompt-bundle ref — nothing hardcoded, fresh deploy is config-only; cargo 302/302, vitest 160/160) / task-006 done 11-02 (service-side VLM client, OpenAI chat, non-streaming v1; D-FB hard-fail proven live + unit-tested; t2i + box-edit through the sidecar in a co-resident window; cargo 308/308, vitest 160/160, mcp 35/35) / task-007 done 11-02 (BFL prompt bundle as committed data in `assets/pup/` + LICENSE with source commit and file hashes; runtime loader with hash tests; four constants and both request-line formats out of the binary — `strings` on the service: zero BFL fragments, logs clean; cargo 313/313, vitest 160/160, mcp 35/35) / task-008 done 11-02 (rebuilt image `external-engine-0.11.2-4193733` deployed from `frontends/kokon/` — compose + README written; **stock-klein cutover absorbed from 011**: live gateway alias → stock serve/stop with adopt path, `.orig` = post-cutover state; e2e live proof: t2i sidecar-enhanced → stock engine (service + engine logs), box-edit preserving the locked bbox table, D-FB hard-fail `upsampler` with the sidecar down; the `starting_backend` freeze fixed (bounded nvidia-smi + `BoundedCache`, `4193733`) — two consecutive windows then passed in ~105 s / ~90 s; v269 monitor + eviction-loop semantics measured, window cleanup unloads the klein alias, vLLM self-heals; owner visual image: `jobs/klein-smoke-0.11.2-t2i.png`)
- [x] WP2 klein goes stock — task-009 done 11-03 (the live proof: `groups.kokon = {swap:false, members:[kokon-klein, kokon-pup]}` in config.yaml + joint supervisor as the klein cmd — one request raised both members (40 s, page-cached; 240 s startup grace covers cold), job A t2i (16 s) + job B box-edit (20 s, locked bbox table survived the sidecar verbatim + extended by 3 of its own locked rows), 120 s TTL took both down together, second request revived the pair (20 s), vLLM back through the gateway's own trigger with its wrapper watching; the 21:15 reboot incident, the 22:26 broken-probe gate, and all four overnight window failures are post-mortemed in the task file, and the v269 semantics are corrected — there is NO autonomous monitor (revival is request-driven EnsureReady), API unload = Stop-only (no removal route exists), cmd start/stop are Debugf-invisible at `logLevel: info`; my own harness was the "nudge" that kept reviving vLLM mid-window — windows now stop pi-debug-proxy for their duration; **verdict for task-015: Path A + B fused — native `groups` = gateway authority/eviction/co-residence, joint supervisor cmd = shared lifetime**, v269 alone cannot start two processes from one request) / task-010 done 11-03 (PUP A/B, 4 windows: **decision — production PUP = BFL `flux-3-image-klein-pup` Q8_0** (9.5 GB sidecar, ~20 GB pair, tightest fit, best quality ceiling; 8B baseline stays config-selectable). The fine-tune lifts t2i + compose over stock Qwen3-VL-8B and is clean at all three quants on the production path; window C's C2 "loop" was the ad-hoc driver's simple scaffold, not the model (window D, the production request path, was clean at all quants). Model-identity correction logged (the earlier "9B" candidates were unsloth stock, not the PUP), BFL maker-spec audit logged (sidecar sampler + pixel budget + JPEG q92; DiT 16/euler/no-CFG; request body beats server flags; config = startup snapshot), evidence in `llama-swap-deploy/jobs/pupab-{A,B,C,D}-digest.txt`) / task-011 done 11-03 (stock klein as a first-class gateway tenant; file-side cutover: Q8 sidecar default, fork retired to `__mess/`, `KLEIN-ROLLBACK.md`; windows E+F: five production jobs incl. the 3-box i2i acid test, all clean, owner visual verdict fine; **window G rollback drill complete in both directions** — fork in + job `job_3b0d05ce1914` OK (attempt 1, 12:00–12:09, 8.5-min fork cold start measured), forward + stock job `job_cd7fe63c378d` OK (attempt 2, 12:48–13:03, Q8 re-confirmed, vLLM revived in ~55 s); both trigger-loss modes, the veto/no-op evict failure, and the owner-traffic episode are post-mortemed in the task file)
- [x] WP3 qwen goes stock — task-012 done 11-03 (audit of the fork's 20 commits: **17 retire / 3 moot / 0 keep-ported** — the qwen image pipeline is safe on the stock pin `290148d` with zero of them, no exception list, no upstream PR candidates; table in `tracker/audits/qwen-fork-audit.md`) / task-013 done 11-03 (A/B window 16:32–16:37: 4 min, 7/7 jobs, vetoes 0 — cutover live: stock engine :39160 + sidecar PUP; the stock leg's per-request churn root-caused in the llama-swap source (ungrouped models land in the default one-at-a-time group) and fixed with per-flavor exclusive groups + one sidecar door per taste (two-door layout, owner-approved) and heavy-tenant gates in both supervisors; two-door verification window 18:08–18:12: all 3 checks PASS — churn gone, flavor swap clean; owner verdict "not worse, not different" — **fork archived to `__mess/` with the `QWEN-ROLLBACK.md` one-step rollback**) / task-014 done 11-03 (qwen prompt set committed as the `assets/pup-qwen/` bundle + two-family loader, image 610ccb3 deployed with per-model `promptBundle`; **D-QW A/B verdict: 9B BFL klein-pup wins** — round 1 turbo (owner: "9b is better"), the canary hard edit, and the full-flavor standing set (9B-Q4 after the Q8 OOM data point, ~21.3 GB pair); cutover live as `config.yaml.stock-014` — turbo door Q8 / full door Q4, one-step 8B rollback in QWEN-ROLLBACK.md; owner visually confirmed 11-04 (renders “virtually indistinguishable between pup quants”, prompts pass) — **9B klein-pup locked for qwen generation** (and the klein sidecar); the 8B rollback (stock-013) stays an escape hatch. Five window failures post-mortemed in the task file — all the agent's own (exec bit, a filename, an alias, a silent no-op edit, a moved switch), the daemon and the box behaved exactly as designed every time)
- [x] WP4 matrix + close — task-015 done 11-04 (v269 native matrix router, stock-015 live, MATRIX.md written; three window attempts: attempt 1 found + fixed the vllm-serve.sh booting race, attempt 2 died on my own dropped-paren transcription, attempt 3 DONE 255 s with all six phases passing) / task-016 done 11-04 (`docs/fresh-box.md` — the fresh-box doc: the final state as a stranger can stand it up, per the goal's list; ACs all met, see the task file) / task-017 done 11-10 (proposal posted 11-06 as Discussion #52; the author's reply: declined on the one-model vision, the fork acknowledged as the right home — the D-RB branch: the fork is the product; two rebase watch items recorded) / task-018 resolved 11-04 as the duplicate of task-015 (its written comparison: `tracker/audits/matrix-vs-groups-018.md`). **WP4 closed — all work packages done; the only remaining queue item is task-023, parked on the owner's trigger.**
- [x] WP5 sidecar efficiency — task-019 parked 11-04, **re-scoped 11-05, DONE 11-05** (one shared Q4_K_M PUP door as a member of all three image sets, serving both client names as aliases; the lifetime precondition died with the re-land; **w019: run 1 failed on the window's own client stream evicting the door (a vLLM orphan boot), run 2 passed in 75 s under the registry-neutralization design** — 0 sidecar evictions across two switches, stop propagation 2 s; record: `done/task-019.md`)
[x] WP6 stock-cleanup — task-020 done 11-04, reopened 11-05, **re-land COMPLETE and closed this span** — the 11-04 record (stock-016 live, deferral proven in both directions, chat-priority the product fact) stands as the provenance of the invariants; the 11-05 re-land: layout surgery (pure stock trees; `_scripts`/`_configs`/`_jobs` homes; 10 pure-boot scripts; **no `cmdStop` anywhere**; `.last-good` + reload semantics; `logToStdout: both`), the owner's handover to the daemon-owned vllm, the pre-window daemon restart (journal mirror engaged; the P3 bind-handover race found + gated), and the **w020r window PASSED 223 s** (vllm 4-s group stop with `EngineCore` in the journal, llama.cpp 2-s, every set eviction per the stock-017 sets, vllm restored last, DONE marker last). Record: `done/task-020.md`.

## Current
**task-026 multi-user — DONE on master (2026-11-12, autonomous run; the owner driving the day, no GPU window).**
Per-user accounts on the deliberately single-user service: a `users` table
(name + Argon2id, in the existing `gallery.db`) and a `user` column on
`generations`; the session file's bare expiry stays the operator, an object
entry is a named account (D-MU1/2); a `multiUser` config gate (schema 15,
default off — stock behaviour byte-identical, set only by `POST
/users/enable`) (D-MU3); login gains an optional name (distinct
"Unknown name." / "Wrong password.", one per-IP limiter) (D-MU4); named
accounts see only their own gallery rows (anything else is the stock
404 — no existence leak) and their files live in
`<gallery>/<name>/<date>/`, the operator keeps the root and filters with
`?user=` (D-MU5); `JobView` carries the owner — a named account
lists/gets/waits/cancels only its own jobs and its `/events` stream drops
the others' (D-MU6); `GET/POST/DELETE /users`, `POST /users/{id}/password`,
`POST /users/enable` are operator/token-only (delete cascades rows + files
+ sessions; the gate off ends named sessions) (D-MU7/9); settings stay
machine-level (D-MU8/10). UI: the login screen's name field (only when the
service reports multi-user on), the gallery's Who filter + small owner tag
(operator/desktop only), Settings → People (visible where a password is
set, hidden from named accounts) (D-MU11). Proof without a GPU (D-MU12):
qls-core 404, service 19 (new named-accounts security test), UI 231, and
`tests/external-engine/run_multiuser_test.sh` — real service + mock gateway
+ two cookie-jar identities — **50/50** (logins, one job each, job scoping
404s, the SSE job filter, gallery + file scoping, cross-user 404s, the
`?user=` filters, the 403s, the delete cascade, the gate off); the original
single-user harness still 16/16. The box got the image
`multiuser-e59cb70` (the old one in `compose.yaml.bak-e59cb70`) and the
API-level check (D-MU12) passed **40/40** on it: the real 88-row gallery
migrated to schema 15 untouched (all rows ownerless, every file present),
the gate off → named logins refused, on → two test accounts signing in
with distinct errors, a seeded row readable by its owner and 404 to the
other, the `?user=` filters, the named account refused user management, and
cleanup restoring the exact baseline; the vllm runtime was never evicted
(no job was submitted). **Field fix (2026-11-13, the owner's first live
look, from a phone):** the People "Add" button read greyed-out and a tap
did nothing — it was genuinely *disabled* (the 8-character password
minimum the subtitle never said; an exact replay of the cookie+CSRF
browser flow proves the request path works) — and its label was squeezed
into a near-zero column (a `.setting-card` flex row with fixed-width
inputs). The form now stacks its text and fields at every width, states
the minimum in the subtitle and the placeholder, the per-user rows stack
on phones, and the new-password field holds 8 instead of a stray six;
the box runs `multiuser-b1f2251` (rollback `compose.yaml.bak-b1f2251`),
re-checked 40/40 (its two deltas were the owner's own gate being on), and
the owner's gate left on.

**Incident 2026-11-11 (the "401 / did not report its capabilities" + Layers lab) — FIXED the same night; the box is demo-safe.**
Two separate facts: (1) every generation from 21:35 tonight failed at the
gateway door with 401 because the 0.12 UI's `ExternalEngine` type has no
gateway-key field, so the first settings round-trip after the 0.12
upgrade (a model switch — `POST /models/switch` rebuilds the active
`externalEngine` from the picker entry; `PUT /config` does the same for
Settings saves) deserialized the stored 48-char key as `""` and saved
it away; the key was restored live (no-boot probe re-verified the door),
and the fix `793ed0a` (`AppConfig::absorb_secrets` in `set_config`: an
empty incoming key keeps the stored one; an explicit one always wins;
covers both paths; unit-tested + live regression on the deployed image)
shipped as image `external-engine-0.12.0-793ed0a` (compose switched,
backup `compose.yaml.bak-793ed0a`). (2) The Layers lab on the phone is
absent **by 0.12 design** (`PhoneApp.tsx` excludes it — demo on a wide
window); the desktop "downloading the segmentation models" error was the
lab's MobileSAM pair (two ONNX files, 44.6 MB, pinned in `matting.rs`)
having never been downloaded on this box (w024 covered only the 224 MB
BiRefNet model) — now installed through the service's own hash-verified
`/segment/download`; `/segment` and `/matte` both report installed.
No GPU work was done (vllm never evicted). Residuals recorded in the
task file: the ghcr push needs the owner's token (`packages` scope) —
the deployment is local-image so nothing is blocked — and this host's
homebrew openssl is now `@4` (the `@3` protocol line is stale). Full
record: `tasks/done/incident-2026-11-11-401-and-layers.md.**

**task-017 — CLOSED with the reply (2026-11-10).** Discussion #52
(https://github.com/GabrieleLozupone/kokon/discussions/52, Ideas) drew
the author's answer: the external-engine mode is **declined upstream**
(one-model vision + "more than I can keep tested release after
release"), but with an explicit acknowledgment — *"your fork is the
right home for that setup, and it's good to see it exists for people
with a box like yours."* Outcome per the recorded framework: **the
fork is the product (D-RB)**; the proposal branch stays on the fork as
the record of the build. He also (a) notes the access half already
exists upstream (Settings → Other devices / `--remote` / Docker) and
(b) announces **idle-unload in the next release** (the model unloads
after inactivity even with a tab open). **Two watch items for the next
rebase:** the service's idle timer vs the gateway's TTL in external
mode (the service must not manage a model it doesn't own), and his
remote story vs our reverse-proxy login. The queue is empty except
task-023, parked on the owner's trigger.**
- **task-024 — rebase onto upstream master (= 0.12.0) — DONE 2026-11-06.**
  Both branches re-landed on upstream master `f9b6971`: `flux` (110 commits,
  13 conflict stops resolved hunk-level — 0.12 wins for upstream behaviour,
  fork work preserved, additive unioned) and the PR branch, re-derived as
  **`pr-external-engine-012`** (the 5 signed commits `ff372fc…e141d35`; the
  old `pr-external-engine-v2` name retired). The one design branch is in:
  0.12's area/Expand Prompt+ keeps its in-engine path in default mode and
  routes the same `own` request through the service-side VLM (the task-006
  client) in external mode — the built-in area/expand prompts are Kokon's
  own words, safe on the sidecar. 0.12's service-resident ONNX (MobileSAM in
  the webview, BiRefNet-lite CPU) and the lab come in as-is, identical in
  both modes. Green on the `flux` tip: qls-core 375 / service 18 / UI 228 /
  mcp 35+3sk / mock-gateway harness 16/16 (harness refreshed to
  `/models/switch` + schema 13, `85e9656`). **The w024 window ran
  (15:54 UTC)** with the owner's matrix rename in it (registry ids ==
  boot-script names; the alphabetical port map shifted klein→9102,
  pup→9103, full→9104, turbo and the vLLM pair unmoved): P0–P4 green
  in-window (the renamed turbo's t2i through the service's keyed one-door
  client path; the 0.12 Expand Prompt+ proven end to end through the shared
  PUP sidecar, alias `qwen3vl-8b`), P5's matte missed 0.12's separate
  `/matte/download` gate and was completed post-window CPU-only with vllm
  resident (BiRefNet-lite now in the data volume; the job script's P5 fixed
  for self-contained re-runs). The final image `external-engine-0.12.0-b77d049`
  is deployed (`frontends/kokon`), `docs/fresh-box.md` carries 0.12 + the
  022 one-door section at the tip, and the mock-gateway harness now uses the
  renamed ids (16/16; `9d87211`). Full record: `done/task-024.md`.
- **task-017 — upstream PR — the publish is the last item; the joint call
  happened the same day (2026-11-06): discussion route, v2 body as-is,
  the owner opens it in the browser (category Ideas).** The branch = upstream master + 5 DCO commits of the
  final-form feature (schema-13 config, gateway client, service-side VLM,
  runtime bundles, picker + box UI, GPU-free mock-gateway test); green on
  the branch (qls-core 370 / UI 228 / service 18 / mcp 35+3sk) =
  **mergeable on the spot**. What happened since (the owner's "go with a"):
  the **complete** branch — BFL bundle in (`9255632`), public on the fork
  for the first time — is pushed as fork branch `pr-external-engine-012`
  (`e141d35`); the fork's **default branch** (renamed `flux` → `main` that
day) contains everything, and the branch list was tidied to just `main`
  + `pr-external-engine-012` (pre-v2 lines preserved as the annotated tags
  `parked/pre-v2-pr` `7958672` / `parked/pre-v2-line` `d318018`; the 7
  upstream-mirror branches deleted — six byte-identical to the author's
  own, the stale `master` snapshot fully contained in `main`). **The fork
  is safe and clean.** **The post is up: Discussion #52 (Ideas), 2026-11-06 17:39
  UTC, from `iamskeole` via the web UI — that repo allows any logged-in
  user to *start* a discussion, so the precondition above was API-side
  only. Body verified verbatim (the corrected diff link); 0 comments at
  verification. The ball is in the author's court; the follow-through
  (re-baseline on accept, or the fork-as-product on reject per D-RB) is
  on the task file, now in `done/`.**
- **task-022 closed this span** (one-door, single-key topology — the full
  record: the target table, the v269 alphabetical `\${PORT}` contract, the
  five-run window history and each lesson it taught, the final box state,
  the owner-side open threads — is in `done/task-022.md`; same-day
  addendum: the orphan `macros:` block removed (settled config
  `b4862632…`), the **`w022c` creative-pair window DONE in 105 s**
  (both G4 writers proven through the keyed gateway on 9100/9101), and
  the v269 hot-reload lesson — a config reload is a full in-process
  server restart: all engines stop, in-flight streams get one cut,
  `.last-good` is never auto-re-seeded; plus the VLM key-inheritance fix
  `ab1b425`: a VLM entrypoint with an empty `apiKey` inherits the
  service's gateway key — the live bug it closed: the klein pipeline's
  shared-door entry was keyless against a now-keyed gateway).
- **task-019 closed 2026-11-05** (the full record — spec, run 1's failure
  and the design fix it produced, run 2's evidence, before/after — is in
  `done/task-019.md`); the only open thread is the owner's eyes on a klein
  edit of a sub-1 MP input (the one behavior change: a 1 MP upscale instead
  of pass-through — pre-cleared, not a window phase).
- **task-023** (job-hold sidecar) stays parked on the owner's trigger —
  no observed pain from concurrent use; the queue after 17a/024/017.
- **task-028 — DONE (2026-11-12, same day as created)**: Qwen native PUPs per job mode
  (PE-T2I for t2i, PE-I2I for image-grounded, both qwen flavors;
  klein keeps the BFL door). Created 2026-11-12 from the owner's
  question; the investigation and the full build document live in
  `tasks/done/task-028.md`. **Naming per the owner the same day:
  explicit per-family door ids — `kokon-pup-flux-image-3-klein` (the
  shared door, renamed), `kokon-pup-qwen-image-2.1-t2i` / `-i2i`
  (new)** — one identity per door (gateway id = alias = service
  `model` field = script name). **Progress 2026-11-12:** steps 1–4 done
  (models + mmproj + PE contract bundle pinned with provenance; rename
  window w028r1d; the additive window w028r2e registered the two native
  doors at 9104/9105 with the qwen sets widened, and proved step 2's
  remaining proofs — boots, the mmproj-less T2I image rejection, the
  thinking switch, the JSON contract on both doors); **step 5 DONE**
  (schema-14 service code — `vlmVision`, mode-routed client, native
  bundle + decode/retry, one-shot compose, the suggested-shape hint;
  six commits `ae971a8…fedbcb5`; image `…-fedbcb5` deployed, box
  behaviour unchanged until the cutover); **step 6 DONE** (the A/B
  windows w028r3 + w028r3b: 7 jobs, same prompt + seed; arm A vs arm B
  (native, thinking on) vs B-off (thinking off); 7/7 in-contract, no
  retries, no leaks; the one-shot compose beat the two-step on time
  and prompt richness; measurements + the 7 images in
  `/tmp/w028r3/{digest.json,jobs-index.jsonl,images/}`). **Verdict the
  same evening: ADOPT-NATIVE-ON (t2i b > boff > a, edit a ≈ b, compose b >
  a) — the cutover is live (schema 14 native wiring; `config.json.pre028`
  is the one-step rollback), the vision-door 1 MP host fix proven
  (D-PE18/D-PE19).**
- (done this span: **task-018** — the matrix-router question parked since the task-013 era is the same work as done task-015: resolved as a duplicate, with the AC's written comparison written as `tracker/audits/matrix-vs-groups-018.md` (per-requested-model groups-vs-matrix table; the three delta classes; the one deliberately dropped asymmetry; the 4,608-case replay and the live 255 s window as evidence; `config.yaml.stock-015` is the realized candidate, one-copy rollback to stock-014). The queue now holds task-019 (parked) and task-022 (the owner's one-door single-key topology, parked on the owner's two decisions), with task-020 the current work.)
- (done this span: **task-016** — the fresh-box reproduction doc is `docs/fresh-box.md`: the whole stack from a new machine (host layout; verified prerequisites; final-state model list + provenance convention; the stock engine recipes at their pins; the v269 gateway with the full matrix routing block and its three-rule semantics; the service image + host-network compose + schema-13; the verification checklist incl. the `matrix:` journal line; the ops rules incl. reload-kills-models and marker-last windows). No BFL prompt bytes, no fork references. AGENTS.md's doc table points at it.)
- **task-025** (the lab's models on the box — BiRefNet → CUDA EP, MobileSAM → box-run endpoint, so weak clients use the lab as a thin client) — **PARKED on the owner's trigger** (created 2026-11-11 from the owner's question after the incident night: the VRAM fits on the heaviest pairs (20–21 GB of 24 GB; both lab models cost well under ~1 GB with a capped ORT arena). The 12-inch Intel MacBook cannot run the in-browser models natively (no WebGPU, one-thread WASM, and the BiRefNet WASM OOM'd in the spike) — a box-run lab is the long-term fix for that client. The file splits the work: BiRefNet→GPU is the small half, MobileSAM→box is the architectural half; in-browser stays the stock-mode default.)
- **task-026** (multi-user: per-user accounts + per-user galleries on 0.12's existing SQLite/Argon2/session base) — **PARKED on the owner's trigger** (created 2026-11-11, anticipating the demo's "what if two people use it?": today one shared password + one shared gallery; the unbundling adds a user dimension to `gallery.db`/`remote-auth.json` — zero new dependencies; fork-only additive, stock path byte-identical; the demo wording and the staged design live in the task file).
- **task-027** (move the kokon service + MCP + its three data volumes to the stable prod box behind nginx; the accelerator keeps only the gateway + engines) — **PARKED on the owner's trigger** (created 2026-11-11 after the owner moved Open WebUI to the prod box: 0.12 is already reverse-proxy-aware (`X-Forwarded-For`/`-Proto` from a private peer, `Secure` cookie) and the container runs `--persistent --remote`, so nginx-in-front is the intended usage; the task file carries the full footgun checklist — nginx SSE/600 s-timeout/`client_max_body_size`, the token port stays loopback and the MCP must move with the service, gateway URL → accelerator's IP, volume + token migration, drop the nvidia reservation, and the bonus: TLS at nginx unlocks WebGPU for the lab's in-browser model). **The execution plan is written:** `docs/service-on-prod-box.md` — the paste-able handover prompt, the compose with bind-mounted data dirs next to the yaml (the owner never hunts for volumes), the config seed, the load-bearing nginx values, the verification checklist, and the receiving side of the data move; AGENTS doc table points at it). **Dev-box half done 2026-11-12** (the accelerator's deployment moved from named volumes to the same bind-mounted shape beside its compose — token, password, lab models and config carried over, gallery fresh per the owner; old volumes kept as rollback). **Prod-box half pending**: the accelerator has no ssh path to the stable box, so it waits on the owner supplying access (an agent session there for the paste-able prompt, or ssh) plus the box's name/IP, nginx layout and TLS-or-not.
