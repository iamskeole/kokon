# STATUS — kokon v2 separation

Updated 2026-11-02. `[ ]` open, `[x]` done. **Current work = the oldest file
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
- [ ] G1 per-pipeline config surface: entrypoints + key + prompt bundle, all pointable (task-005)
- [ ] G2 prompts as repo data assets — klein BFL set now (task-007), qwen set in WP3 (task-014)
- [ ] G3 service VLM client (OpenAI chat + image parts) replaces the in-engine `/text_gen` call (task-006)
- [ ] G4 production klein = stock sdcpp + PUP sidecar (task-002, 004, 009, 010, 011)
- [ ] G5 production qwen = stock 0a06712 + shared PUP (task-012, 013, 014)
- [ ] G6 gateway: generic N-models-per-job, the "matrix" (task-015)
- [ ] G7 fresh-box doc (task-016)
- [ ] G8 upstream PR / merge / philosophy discussion — last (task-017)

## Work packages (PLAN.md §4)
- [x] WP0 housekeeping — task-001 done 11-02 (v269 pin + `llama-swap-deploy/` sibling) / task-002 done 11-02 (`290148d` pinned + built + stock klein verified, findings logged) / task-003 done 11-02 (`models/gguf`+`models/sd-cpp` reorg per the provenance convention, all reference paths updated, config validates 6/6; klein DiT conversion source unresolved — owner question, non-blocking)
- [ ] WP1 sidecar + service wiring — task-004 done 11-02 (PUP sidecar: named llama.cpp May build, alias `kokon-pup` on :39175, 1-token readiness, vision + BFL t2i re-verified through the gateway — vllm self-healed after; retry-storm + setsid-pitfall findings logged for 009/010) / task-005 done 11-02 (schema 13: per-pipeline gateway override + VLM endpoint + prompt-bundle ref — nothing hardcoded, fresh deploy is config-only; cargo 302/302, vitest 160/160) / task-006 done 11-02 (service-side VLM client, OpenAI chat, non-streaming v1; D-FB hard-fail proven live + unit-tested; t2i + box-edit through the sidecar in a co-resident window; cargo 308/308, vitest 160/160, mcp 35/35) / task-007 done 11-02 (BFL prompt bundle as committed data in `assets/pup/` + LICENSE with source commit and file hashes; runtime loader with hash tests; four constants and both request-line formats out of the binary — `strings` on the service: zero BFL fragments, logs clean; cargo 313/313, vitest 160/160, mcp 35/35) / 008
- [ ] WP2 klein goes stock — task-009/010/011
- [ ] WP3 qwen goes stock — task-012/013/014
- [ ] WP4 matrix + close — task-015/016/017

## Current
- [ ] task-008 — next (oldest in todo/): image build + `frontends/kokon` deploy + end-to-end smoke (owner visual validation); the deploy carries the new build, `promptBundle` on the klein pipeline, and the bundle in the service's data-dir assets folder
