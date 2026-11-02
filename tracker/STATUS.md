# STATUS — kokon v2 separation

Updated 2026-11-02. `[ ]` open, `[x]` done. **Current work = the oldest file
in `tasks/todo/`.** Keep this file current at the end of every task.

## Decisions (resolved 2026-11-01 — details in PLAN.md §6)
- [x] D-K1 — klein engine = **leejet/stable-diffusion.cpp master** (canonical upstream; day-0 FLUX.3 support since 2026-10-16; one build serves klein + qwen)
- [ ] OPEN ITEM — pin + build-verify the specific leejet master commit for the named sdcpp checkout (task-002)
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
- [ ] WP0 housekeeping — task-001 done 11-02 (v269 pin + `llama-swap-deploy/` sibling; findings logged in the task file for task-010) / task-002, task-003 (wip-branch deletion already done)
- [ ] WP1 sidecar + service wiring — task-004/005/006/007/008
- [ ] WP2 klein goes stock — task-009/010/011
- [ ] WP3 qwen goes stock — task-012/013/014
- [ ] WP4 matrix + close — task-015/016/017

## Current
- [ ] task-002 — next (oldest in todo/): pin + build-verify the specific leejet/stable-diffusion.cpp master commit for the named sdcpp checkout
