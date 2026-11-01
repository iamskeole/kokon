# task-002 — named sdcpp checkout: leejet master, built for the 3090

## goal
`backends/stable-diffusion.cpp` = a clean checkout of **leejet/stable-diffusion.cpp**
(canonical upstream) pinned at a verified master commit; built for the RTX 3090
(CUDA, SageAttention-capable); deployed so one build serves both image
pipelines (klein now, qwen in WP3). The engine has **no PUP** (upstream has
no PUP flags at all).
**Checkout status:** created 2026-11-01 as a local clone of the owner's
freshly-pulled `backends/latest/stable-diffusion.cpp` — pinned at
`290148d` (2026-11-02, "feat: add custom modulate operator and ConvRot
fusion (#2258)"). Day-0 FLUX.3 support in this line since 2026-10-16.

## build phase (the part the owner asked to have in the plan)
Verified against this tree's `CMakeLists.txt` (line 88: `option(SD_CUDA ...)`)
and `docs/build.md` ("Build with CUDA"):

```sh
cd backends/stable-diffusion.cpp
# cutlass is a git submodule (thirdparty/cutlass) — required by the default
# SD_CUDA configure for INT8 GEMM + CuTe attention. Fastest: copy the
# already-populated submodule from the latest/ tree; else:
cp -a ../latest/stable-diffusion.cpp/thirdparty/cutlass thirdparty/ \
  || git submodule update --init --recursive
cmake -S . -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DSD_CUDA=ON \
  -DCMAKE_CUDA_ARCHITECTURES=86 \
  -DSD_USE_UPSTREAM_GGML=OFF
cmake --build build --config Release -j$(nproc)
```

- `SD_CUDA=ON` — the tree's CUDA switch (enables GGML_CUDA + CUDAToolkit;
  toolkit present: `/usr/local/cuda-13.2`).
- `CMAKE_CUDA_ARCHITECTURES=86` — the 3090 (sm_86; same as the old fork
  build's CMakeCache).
- `SD_USE_UPSTREAM_GGML=OFF` — the **patched** ggml; this is what includes
  the SageAttention extension (default is already OFF — stated explicitly
  because production klein runs `--sage-attn` and needs it in the build).
- CUTLASS: with the submodule in place the bundled default is used; no
  extra flags needed (an explicit empty `SD_CUDA_CUTLASS_PATH` would
  disable it — don't).
- **CPU-only work** (~70 min like the old full builds; no GPU, vllm
  untouched): run as `setsid ... > build.log 2>&1 &` and poll with sleeps
  under 120s. No autonomy-protocol window needed for the build itself.
- Toolkit note: `nvcc` is not on the default PATH; cmake finds CUDAToolkit
  in `/usr/local/cuda*` automatically (the old build's cache used
  `/usr/local/cuda-13.2/bin/nvcc`).

## acceptance criteria
- Checkout at the pinned commit (`290148d` unless re-pinned); `git status`
  clean (the untracked `CMakePresets.json` in the latest/ tree does not
  exist in this git clone — carry it over only if the owner wants it).
- Build completes; the produced sd-server + libstable-diffusion + libggml
  libs are copied to `backends/stable-diffusion.cpp/deploy/klein/`
  (deploy dir + serve/stop scripts per pipeline).
- `--help` of the built binary: no `--pup-*` flags (pure upstream).
- One klein t2i succeeds through the raw engine API using the existing
  `models/sd-cpp/flux3/` weights (DiT Q8 + Qwen3-VL-4B + ae.safetensors —
  the exact stack in leejet's `docs/flux3.md`) — **this one is a GPU
  window**: autonomy protocol (blocking call, setsid watcher, vllm reload).
- vllm healthy after the window.

## blockers
- None known; first configure of a fresh tree may surface a flag rename —
  the option names above are verified against the current tree.

## decisions
- Named-slot path `backends/stable-diffusion.cpp` (the owner's naming);
  `backends/latest/stable-diffusion.cpp` stays as the cutting-edge sync.
- Pin = current master (`290148d`); keep the pin stable (leejet master
  moves daily) — deliberate re-pins only, with the owner's re-test.
- The GabrieleLozupone `kokon`-repo branch (1814814) is no longer needed
  for the engine; it remains only as the audit baseline for the old fork.
