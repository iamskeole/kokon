# STATUS — kokon v2 separation

Updated 2026-11-03. `[ ]` open, `[x]` done. **Current work = the oldest file
in `tasks/todo/`.** Keep this file current at the end of every task.

## Decisions (resolved 2026-11-01 — details in PLAN.md §6)
- [x] D-K1 — klein engine = **leejet/stable-diffusion.cpp master** (canonical upstream; day-0 FLUX.3 support since 2026-10-16; one build serves klein + qwen)
- [x] OPEN ITEM — pin + build-verify the specific leejet master commit for the named sdcpp checkout (task-002: `290148d`, done 11-02)
- [x] D-LS — llama-swap pinned at v269, deploy artifacts moved to a non-git sibling (task-001)
- [x] D-QW — shared PUP for qwen: A/B first (task-014)
- [x] D-FB — VLM down mid-job = hard-fail (making the VLM work is our job)
- [x] D-WIP — in-engine Qwen3.5 port branch deleted (was ff69b0d; rabbit hole, do not follow)
- [x] D-ST — host's `stable/` (vllm) is owner-owned; stays as-is

## Gaps (PLAN.md §2)
- [x] G1 per-pipeline config surface: entrypoints + key + prompt bundle, all pointable (task-005)
- [ ] G2 prompts as repo data assets — klein BFL set now (task-007), qwen set in WP3 (task-014)
- [x] G3 service VLM client (OpenAI chat + image parts) replaces the in-engine `/text_gen` call (task-006)
- [ ] G4 production klein = stock sdcpp + PUP sidecar (task-002, 004, 009, 010, 011)
- [ ] G5 production qwen = stock 0a06712 + shared PUP (task-012, 013, 014)
- [ ] G6 gateway: generic N-models-per-job, the "matrix" (task-015)
- [ ] G7 fresh-box doc (task-016)
- [ ] G8 upstream PR / merge / philosophy discussion — last (task-017)

## Work packages (PLAN.md §4)
- [x] WP0 housekeeping — task-001 done 11-02 (v269 pin + `llama-swap-deploy/` sibling) / task-002 done 11-02 (`290148d` pinned + built + stock klein verified, findings logged) / task-003 done 11-02 (`models/gguf`+`models/sd-cpp` reorg per the provenance convention, all reference paths updated, config validates 6/6; klein DiT conversion source unresolved — owner question, non-blocking)
- [x] WP1 sidecar + service wiring — task-004 done 11-02 (PUP sidecar: named llama.cpp May build, alias `kokon-pup` on :39175, 1-token readiness, vision + BFL t2i re-verified through the gateway — vllm self-healed after; retry-storm + setsid-pitfall findings logged for 009/010) / task-005 done 11-02 (schema 13: per-pipeline gateway override + VLM endpoint + prompt-bundle ref — nothing hardcoded, fresh deploy is config-only; cargo 302/302, vitest 160/160) / task-006 done 11-02 (service-side VLM client, OpenAI chat, non-streaming v1; D-FB hard-fail proven live + unit-tested; t2i + box-edit through the sidecar in a co-resident window; cargo 308/308, vitest 160/160, mcp 35/35) / task-007 done 11-02 (BFL prompt bundle as committed data in `assets/pup/` + LICENSE with source commit and file hashes; runtime loader with hash tests; four constants and both request-line formats out of the binary — `strings` on the service: zero BFL fragments, logs clean; cargo 313/313, vitest 160/160, mcp 35/35) / task-008 done 11-02 (rebuilt image `external-engine-0.11.2-4193733` deployed from `frontends/kokon/` — compose + README written; **stock-klein cutover absorbed from 011**: live gateway alias → stock serve/stop with adopt path, `.orig` = post-cutover state; e2e live proof: t2i sidecar-enhanced → stock engine (service + engine logs), box-edit preserving the locked bbox table, D-FB hard-fail `upsampler` with the sidecar down; the `starting_backend` freeze fixed (bounded nvidia-smi + `BoundedCache`, `4193733`) — two consecutive windows then passed in ~105 s / ~90 s; v269 monitor + eviction-loop semantics measured, window cleanup unloads the klein alias, vLLM self-heals; owner visual image: `jobs/klein-smoke-0.11.2-t2i.png`)
- [ ] WP2 klein goes stock — task-009 done 11-03 (the live proof: `groups.kokon = {swap:false, members:[kokon-klein, kokon-pup]}` in config.yaml + joint supervisor as the klein cmd — one request raised both members (40 s, page-cached; 240 s startup grace covers cold), job A t2i (16 s) + job B box-edit (20 s, locked bbox table survived the sidecar verbatim + extended by 3 of its own locked rows), 120 s TTL took both down together, second request revived the pair (20 s), vLLM back through the gateway's own trigger with its wrapper watching; the 21:15 reboot incident, the 22:26 broken-probe gate, and all four overnight window failures are post-mortemed in the task file, and the v269 semantics are corrected — there is NO autonomous monitor (revival is request-driven EnsureReady), API unload = Stop-only (no removal route exists), cmd start/stop are Debugf-invisible at `logLevel: info`; my own harness was the "nudge" that kept reviving vLLM mid-window — windows now stop pi-debug-proxy for their duration; **verdict for task-015: Path A + B fused — native `groups` = gateway authority/eviction/co-residence, joint supervisor cmd = shared lifetime**, v269 alone cannot start two processes from one request) / task-010 done 11-03 (PUP A/B, 4 windows: **decision — production PUP = BFL `flux-3-image-klein-pup` Q8_0** (9.5 GB sidecar, ~20 GB pair, tightest fit, best quality ceiling; 8B baseline stays config-selectable). The fine-tune lifts t2i + compose over stock Qwen3-VL-8B and is clean at all three quants on the production path; window C's C2 "loop" was the ad-hoc driver's simple scaffold, not the model (window D, the production request path, was clean at all quants). Model-identity correction logged (the earlier "9B" candidates were unsloth stock, not the PUP), BFL maker-spec audit logged (sidecar sampler + pixel budget + JPEG q92; DiT 16/euler/no-CFG; request body beats server flags; config = startup snapshot), evidence in `llama-swap-deploy/jobs/pupab-{A,B,C,D}-digest.txt`) / **task-011 in doing** (stock klein first-class gateway tenant — its cutover half was absorbed into task-008; remainder: whatever of it survives after 009/010, now including the PUP-Q8 sidecar default)
- [ ] WP3 qwen goes stock — task-012/013/014
- [ ] WP4 matrix + close — task-015/016/017

## Current
- [ ] task-011 — in doing (stock klein as a first-class gateway tenant — the cutover half landed in task-008; remainder after 009/010: the rest of what the file asks, now including making the PUP-Q8 sidecar the production default).
