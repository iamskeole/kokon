# Task-025 (PARKED — owner-triggered) — run the Layers lab's models on the box (GPU-first) so weak clients use the lab as a thin client

**Created:** 2026-11-11 — the owner asked, after the 401/Layers
incident night: "could we eventually get both to run on the box on
GPU? I think we should have the VRAM after the DiT and PUPs"; and
noted his wife's old 12-inch MacBook (Intel, 2015–17) as the weak
client whose native run is doubtful. **Do not start this task unless
the owner triggers it** (the demo and the parked task-023 come first).

## Trigger (when this stops being parked)

The owner says go. Likely real triggers: someone with a weak device
(the 12-inch, a phone, a friend's laptop) wants to use the lab, or the
owner wants the sticker matte faster (CPU ~4 s → GPU well under a
second).

## Where things run today (verified 2026-11-11, code is the source)

- **MobileSAM** (two ONNX files, 45 MB, click-to-select): **in the
  browser** via onnxruntime-web — WebGPU when the device has it, else
  single-threaded WASM (`src/lib/sam.ts`). The service only downloads
  and serves the files.
- **BiRefNet lite** (224 MB, the sticker matte): **on the box, in the
  service, on CPU** (ONNX Runtime CPU build; `crates/qls-core/src/
  matting.rs`). ~4 s per object on 16 CPU threads, first one ~8 s.
  It is on the CPU *by measurement*, not by laziness: the webview
  could not run it (spike s17, 2026-11-02: WebGPU kernels rejected,
  WASM out of memory).

## VRAM budget (measured on this box, 24 GB 3090)

- Heaviest co-resident pairs: klein pair ≈ 20 GB (task-010: "tightest
  fit"), qwen-full pair ≈ 21.3 GB (task-014). Headroom: a few GB.
- The lab models are small: BiRefNet 0.22 GB weights, MobileSAM 0.045
  GB. onnxruntime's CUDA EP uses an arena that grows by default — it
  must be **capped** (configurable; a few hundred MB is enough for
  nets this size). Total added cost with a capped arena: well under
  ~1 GB. **It fits on the tightest set**, at the price of the last
  few GB of headroom.

## What the two halves would take

1. **BiRefNet → GPU: the small one.** Build the service image with the
   CUDA execution provider (ort `cuda` feature + the matching
   CUDA/cuDNN runtime libs in the image; the host already carries a
   CUDA toolchain for the stock engines), select CUDA with CPU
   fallback, cap the arena. Payoff: ~4 s → ~0.1–0.2 s per sticker.
2. **MobileSAM → box: the architectural one.** Upstream 0.12 keeps
   selection in the browser on purpose (a stock install has no
   gateway-side box; clicks cost no network). The box variant is a
   **fork additive**: a service endpoint (image once → cached
   embeddings; click → mask, a few ms on GPU) and a UI mode that
   sends clicks there instead of running locally. On a LAN the
   per-click latency is the RTT, which is fine for interactive use.
   The in-browser path stays the default for stock mode.

## Why this matters for the 12-inch MacBook

Intel 12-inch (2015–17): no WebGPU in that era of OS/browsers, weak
single-core CPU. In-browser MobileSAM there = the per-image 1024-px
encoder on one WASM thread (tens of seconds per image); the BiRefNet
WASM OOM already on a modern desktop, so it is out there regardless.
**Everything else in Kokon is already a thin client for her** (all
inference runs on the box in this deployment) — the lab is the one
feature that would burden her machine. Running the lab on the box
makes the whole product thin-client.

## Decisions (pre-logged for when it starts)

- In-browser stays the stock-mode default (upstream behaviour wins);
  the box-run lab is an additive fork mode, config-selected.
- Capped ORT arena; CPU fallback when no CUDA EP is available.
- Any CUDA-ORT image change is validated in a GPU window per the
  autonomy protocol (vllm never evicted without the window's dance).

## blockers

None. Parked on the owner's trigger.
