# Graphics API and high frame rates

Native startup and Video settings offer OpenGL on Windows/Linux and GLES on
Android. DX12 (Windows) and Vulkan (Windows/Linux/Android) remain preferred;
OpenGL is a compatibility candidate, not a guaranteed performance upgrade.
Apple platforms retain Metal and the browser retains WebGPU.

To explicitly test GL from a shell on Linux:

```sh
WGPU_BACKEND=gl ./target/release/cinnabar --no-vsync --frame-cap 400
```

Use a release build and a ready, focused gameplay window. `--frame-cap 400`
is a ceiling, not a minimum. The existing CLI accepts caps up to 1000 FPS.
Disabling VSync may introduce tearing and increases power consumption.
Menus, background windows, VRR policies and the compositor can impose other limits.

GL still must satisfy the renderer's capability checks, including vertex and
fragment storage buffers. Alternate texture views and multisampled shading are
optional: adapters without alternate views keep a single matching texture/view
format and ordinary world pipelines use linear blending into the sRGB attachment.
Adapters without multisampled shading use one sample before view/depth/pipeline
preparation. GL always uses CPU culling and direct terrain draws.
GL UI uses four clamped level-zero loads for bilinear filtering while retaining
nearest sampling for crisp sprites. Terrain assigns each image global to one
sampler; native sRGB atlas views restore encoded RGB in shaders, preserving alpha.
The label overlay uses the matching single-format target and linear composite.
GLES array allocations pad one-layer and square six-multiple textures without
changing their authored layer indices or reading extra upload data. Padding must
fit the device's array-layer limit; unsupported extents fail rather than truncate
logical layers. Exposing the API does not
make older GPUs compatible. Saved Video choices use the existing automatic
fallback and startup crash recovery; an explicit `WGPU_BACKEND` override is for
diagnostics and does not promise automatic fallback. Remove it to restore Auto.
GL end-to-end rendering and driver acceptance remain unverified. In this pinned
wgpu/Bevy version, downlevel flags are queried on `RenderAdapter`, not `RenderDevice`.
The GLES backend is also rejected by standalone alternate-view upload guards.

The renderer's own surface descriptors use empty `view_formats`. Native window
surface configuration is owned by the pinned Bevy dependency, which selects an
sRGB surface format when available (and then requests no alternate view). Its
non-sRGB-only surface fallback is not validated here; do not interpret the native
texture fallback as validation of every possible GL presentation surface.

## Measuring the 400 FPS target

400 FPS requires a sustained frame interval of at most 2.5 ms. Compare APIs using
the same device, resolution, world, camera path, render distance and pack settings.
Warm up asset loading and pipelines first. Record median, p95 and p99 frame times,
not only an instantaneous FPS counter; include chunk-loading and turning passes
separately. Check GPU usage, thermal throttling and CPU render/streaming costs
before lowering resolution or render distance. Do not change network or simulation
tick timing to increase presentation throughput.

No hardware benchmark for this target has been recorded for this change. See
[benchmarks](benchmarks.md) and [live testing](agents/live-testing.md) for the
project's measurement and acceptance workflow.

## CPU work reductions

These changes are independent of the selected graphics API:

- Actor coverage classifies each instance once into stable main/coverage lists.
  Owner and independent-layer membership use sorted keys rather than linear scans
  inside an instance-sort comparator. Scratch buffers are reused between frames;
  unchanged ordering retains the published payloads and revision.
- Shadow volume planes are initialized once and reused by CPU containment and GPU
  uniform construction. After initialization, containment performs no trigonometry.
- Shadow mesh construction uses fixed-size arrays and precomputed rings instead
  of two temporary heap vectors and repeated ring evaluation. Triangle order is
  preserved. This is a setup-path reduction, not a per-frame FPS claim.
- The diagnostic/compatibility actor path streams visible candidates instead of
  building an intermediate visible vector. Identical previous/current poses share
  one allocation and can reuse the pose-matrix cache.
- Terrain publication deltas skip normalized unchanged columns. Removed sections
  are found by a linear merge rather than one binary search per old section.
- Transparent terrain reference slots share immutable storage until a byte actually
  changes. Shared-byte results still commit their new group layout, and remeshed
  allocations retain urgent upload requirements even when their bytes match.
- Oversized urgent transparent patches are rejected before copying the slot.
  Full slot repacks reserve the known reference count rather than growing a vector.
- The single transparent view is selected with a minimum scan instead of allocating
  and sorting a list of every view; shader-pack exclusion and extra-view warnings stay.
- Uniform-biome water groups resolve their diagnostic tint once per group rather
  than decoding and collecting the same tint for every face.

Regression tests cover stable ordering across 256 owner subsets, warmed scratch
allocation counts for 4096 instances, shadow plane/vertex equivalence, allocation-free
mesh generation, and terrain retirement equivalence. They require Rust to execute;
syntax parsing alone does not verify types, behavior or performance.

Suggested checks on a machine with the pinned toolchain:

```sh
cargo check --tests -p render -p render-model -p render-scene -p launcher -p launcher-host -p cinnabar
cargo test -p render transparent
cargo test -p render-model entity_shadow
cargo test -p render-scene actor::coverage
cargo test -p render-scene terrain_publication
```

Measure the gameplay paths separately from diagnostic actors and mesh setup before
attributing any end-to-end frame-time improvement to these changes.
