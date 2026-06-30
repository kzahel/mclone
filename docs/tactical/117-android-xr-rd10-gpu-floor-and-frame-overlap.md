# 117: Android XR RD10 GPU-Floor Reduction And Frame Overlap

Status: proposed parent. Consolidates the remaining high-leverage Quest 3
standalone RD10 performance levers and supersedes the open/forward-looking items
in [`096`](096-android-xr-quest-performance.md),
[`099`](099-android-xr-rd10-render-cost-attribution.md),
[`106`](106-android-xr-static-render-cpu-reduction.md), and
[`107`](107-xr-stereo-uniform-ownership-and-multiview.md) (all now closed as
references). We will step through the slices below in priority order.

## Why this doc exists (the pivot)

096/099/106 landed a strong diagnosis and the cheap CPU wins (cached records,
FxHash cull scratch, shared stereo prep, per-eye draw masks). Those pulled
frozen-RD10 p50 from `15.66ms` to `13.87ms` — real, but RD10 is still only
*borderline* 72 Hz (p95 `~15ms`, ~40-48% of frames over the `13.889ms` budget),
and live/flight RD10 is worse.

The reason more CPU work will not finish the job is experiment **E1** (106): the
frame is **serial `CPU + GPU` with zero overlap**, and the true GPU half is
**`~10.7ms`** (in-stream wgpu timestamp `10.680ms` ≈ OS fence wait `11.139ms`).
The Meta `app/gpu_frametime` counter under-reported this as `~7ms / 57% util`,
which is why GPU-side levers were deprioritized for months. With CPU now `~4ms`,
the frame is `4 + 10.7 ≈ 14.7ms`, and any GPU thermal spike blows p95.

So the only two things that move the needle now are:

1. **Lower the `~10.7ms` GPU floor** (resolution, foveation, fewer fragments,
   cheaper shader, less geometry), and
2. **Overlap CPU and GPU** so the frame becomes `max(CPU, GPU) ≈ 11-12ms`
   instead of the sum.

Both are largely untried. Code spot-check confirms the cheapest GPU levers are
not even turned on:

- **Fixed-foveated rendering is enabled but never applied.** The `fb_foveation*`
  extensions are enabled (`mclone-android-xr-client/src/lib.rs:905`), but there
  is no `xrCreateFoveationProfileFB` / `xrUpdateSwapchainFB` anywhere — applied
  level is effectively NONE.
- **No render-scale.** The swapchain is created at
  `recommended_image_rect_width/height` (1680x1760) verbatim
  (`mclone-xr-host/src/lib.rs:325`).
- **One terrain pipeline with an unconditional `discard`**
  (`chunk_textured.wgsl:130`) is used for *all* opaque terrain. The mesh splits
  opaque/translucent but not solid/cutout, so solid stone (the bulk of RD10)
  runs a shader with `discard`, which disables early-Z / hidden-surface removal
  on the Adreno tile GPU.

## Baseline to beat

Frozen RD10, masked per-eye default path (commit `ecb502f`), fixed pose
`0,80,-96,180`, budget `13.889ms`:

| Field | Value |
|---|---:|
| p50 / p95 / p99 | `13.746` / `15.166` / `18.813ms` |
| Over budget | `567 / 1433` (~40%) |
| Drawn sections / indices (per eye) | `179` / `1.356M` |
| True GPU floor (E1) | `~10.7ms` |
| CPU after F/G/H | `~4ms` |

Lanes: `native:android-xr:perf:frozen:rd10` and `:frozen:rd10:metrics`. RD1 and
RD5 already hold solid 72 Hz; RD10 is the stress lane.

## Cross-cutting tips and things to watch out for

- **Do not trust the Meta `app/gpu_frametime` counter** for GPU attribution — it
  under-reports by ~35% here. Trust an in-stream wgpu timestamp or the stereo
  poll-wait (E1 proved the poll *is* GPU execution).
- **Thermal drift is large.** The stereo poll wait crept `10.5 -> 11.1 -> 12.1ms`
  across baseline -> E1 -> F purely from GPU clock thermals. Do not attribute a
  sub-`1ms` frame delta to a change. For honest A/B, prefer **within-run
  alternating-frame A/B** (the E2 contention probe pattern) or matched
  back-to-back runs at similar poll/GPU temperature.
- **Per-view uniforms must stay immutable across a whole submission.** 107 fixed
  the shared-uniform clobber that broke the reverted single-submit path; the
  per-view frame ring is the prerequisite for any overlap/multiview/single-submit
  path. **Do not re-land bare single-submit.**
- **Validate on the headset and look at it** (native rule). Use the frozen RD10
  lanes; keep all artifacts in `/tmp`. Confirm validator cleanup (`pidof` empty,
  `mWakefulness=Asleep`).
- **The multiview path depends on a vendored `wgpu-hal 25.0.2`** with
  `imageless_framebuffers` disabled (`native/vendor/wgpu-hal-25.0.2`); that is a
  temporary Adreno workaround, not a permanent fork.
- **Solid-layer and greedy-mesh are parity-relevant.** Read
  `reference/minecraft-1.17.1` `RenderType` / `ItemBlockRenderTypes` first; keep
  appearance identical and document any meshing divergence per the
  reference-porting policy.

## Priority order

| # | Slice | Idea | Cost | Risk |
|---|---|---|---|---|
| 1 | **L** — fill-vs-geometry probe | Confirm whether the `~10.7ms` is fragment-fill-bound or vertex/draw-bound | ~hrs | none |
| 2 | **M** — render-scale | Render eyes below native resolution | ~hrs | low |
| 3 | **N** — fixed-foveated rendering | Apply an FFR profile to the eye swapchains | ~hrs | low |
| 4 | **O** — solid render layer | Drop `discard` for opaque-solid terrain; restore early-Z; audit fragment cost | ~day | low/parity |
| 5 | **K (E4)** — CPU/GPU frame overlap | Overlap N+1 pose-independent prep with N's GPU poll (carried from 106) | ~days | latency/comfort |
| 6 | **J** — draw batching / indirect arena | Shared vertex/index arena + `multi_draw_indexed_indirect` (carried from 106) | ~days | medium |
| 7 | **P** — greedy meshing | Merge coplanar same-light/same-texture faces to cut index count | ~days | parity |
| 8 | **Q** — application spacewarp | Render at 36, compositor reprojects to 72 | ~days | quality |
| 9 | **R** — ship-distance / dynamic RD | Benchmark RD7/RD8; ship dynamic render distance + dynamic foveation/scale | ~hrs | product |

---

## Slice L — Fill-vs-geometry probe (do this first)

**Idea.** Before investing in any GPU lever, disambiguate *why* the GPU takes
`~10.7ms`. Run a frozen-RD10 A/B at **half eye resolution** and/or with a
**flat/constant-color fragment shader** (the `chunk_flat` pipeline already
exists). If the frame drops a lot at half-res → fragment-fill-bound → Slices
M/N/O win big. If it barely moves → vertex/draw-bound → go to Slices J/P.

**Tips.** 099 suggested exactly this ("diagnostic flat-material pass") then
deprioritized it on the bad GPU counter; revive it. The E1 GPU-timestamp split
was removed with the single-submit rollback (`d0c5161`), so either re-add a
minimal timestamp bracket or just read the frame-time delta from the half-res
A/B. A quick `chunk_flat` swap isolates fragment cost from vertex/draw cost.

**Watch out.** Thermal drift (see cross-cutting). Half-res changes the projection
image rect, so make the probe a clean toggle, not a hack that desyncs the depth
target or the projection-layer rect.

## Slice M — Render-scale

**Idea.** Create the eye swapchain at `recommended * scale` (e.g. `0.8-0.85`)
instead of native. ~28-40% fewer fragments straight off the `~10.7ms` floor.
Quest titles do this routinely; paired with FFR the periphery loss is minimal.

**Tips.** The single edit point is `eye_config` (`mclone-xr-host/src/lib.rs:323`)
plus the projection-layer `imageRect`. Reuse the shared `RenderConfig`
render-scale boundary from tactical 102 (desktop F8/F9) — do not fork a
Quest-only path. Make it a runtime setting / launch flag so it is A/B-able.

**Watch out.** Clamp to `max_image_rect_width/height`. The depth target and the
two multiview array layers must match the scaled size. UI/HUD/menu text
legibility suffers at low scale — consider rendering the world at scale but UI at
native, or accept it for a first pass. Report the rendered rect in the
projection-layer view so the compositor samples correctly.

## Slice N — Fixed-foveated rendering (FFR)

**Idea.** Create an `XrFoveationProfileFB` (`xrCreateFoveationProfileFB`), set a
level (start HIGH or HIGH_TOP), and apply it via `xrUpdateSwapchainFB` with
`XrSwapchainStateFoveationFB`. Cuts periphery fragment cost ~25-40% with no
geometry/CPU change. Extensions are already enabled — this is the cleanest
unexploited GPU win in the whole effort.

**Tips.** `XR_FB_foveation_vulkan` is required for the Vulkan path (already
enabled). Consider dynamic foveation (`XR_FB_foveation_configuration` dynamic
level) so the runtime ramps foveation with GPU load. **Verify it actually
applies** — 099 flagged the applied level as unverified; confirm via the GPU
frame-time drop and a periphery-sharpness capture, not just a successful API
call.

**Watch out.** Foveation is per-swapchain — confirm it applies to the two-layer
multiview swapchain as well as the per-eye swapchains. Check the `openxrs` crate
exposes these calls (may need a raw function pointer). Periphery shimmer/aliasing
at HIGH; tune the level. Reserve `debug.mclone.xr_foveation` (already noted as a
future property in 083) for level control.

## Slice O — Solid render layer (drop `discard`) + fragment-cost audit

**Idea.** Split opaque terrain into vanilla-shaped `solid` (no alpha test) and
`cutout`/`cutout_mipped` (alpha test/`discard`) layers, and give `solid` a
pipeline **without `discard`** so the bulk of terrain regains early-Z /
hidden-surface removal on the tile GPU. While in the shader, audit the
`apply_color_profile` path: mode `> 1.5` runs **3x `pow()` per fragment** for
sRGB encode — if a color profile is active on Quest, that is pure fragment cost
that could be precomputed into the atlas or approximated.

**Tips.** `reference/minecraft-1.17.1` `RenderType` + `ItemBlockRenderTypes` is
the authority: most blocks = `solid`, leaves = `cutout_mipped`, glass = `cutout`,
water/ice = `translucent`. The mesh builder already splits opaque/translucent
(`mclone-mesh/src/builder.rs`, `data.rs` `translucent_index_range`); add the
solid/cutout split *within* opaque and a second pipeline. Per-block layer
assignment lives next to the existing model/`solid_render` facts.

**Watch out.** Only route fully-opaque textures through the no-`discard` solid
pipeline (alpha is ignored there). `cutout_mipped` (leaves) needs mipmaps on vs
`cutout` (no mip) — different sampler/UV-shrink behavior; match vanilla. This is
a render-layer parity change, so keep appearance identical to the current single
pipeline.

## Slice K (E4) — CPU/GPU frame overlap (carried from 106)

**Idea.** Submit frame N's GPU work, then **during the `~10.7ms` poll** do frame
N+1's *pose-independent* work (runtime poll, completed-section upload
application, record maintenance, world tick), and only block on N's fence right
before `xrReleaseSwapchainImage`/`xrEndFrame`. Frame -> `max(CPU, GPU+contention)
≈ 12.6ms`, under budget. The per-view uniform frame ring (107) makes this safe.

**Tips.** The three prior overlap probes were "mixed" because they overlapped the
**wrong** work: runtime-prefetch overlapped heavy section-sync (inflated the
record rebuild to `26-47ms`); eye-submit-overlap only overlaps L/R, not N+1.
Overlap only cheap, pose-independent work. Cull/encode are pose-dependent (need
the final head pose), so they stay *after* the wait — but they are now small
(`~4ms`).

**Watch out.** Adds ~1 frame of latency; motion-to-photon is already `25-39ms`.
Make it a runtime toggle, default off, with a headset comfort check and an MTP
before/after measurement. The `+1.9ms` GPU contention from concurrent CPU on the
unified-memory SoC (E2) is real but bounded — still a net win. **E5** (GPU-side
semaphore / no CPU block) is the cleaner architecture but needs raw
Vulkan/OpenXR sync that wgpu does not expose ergonomically — defer it unless
E4's latency proves unacceptable.

## Slice J — Draw batching / indirect arena (carried from 106)

**Idea.** Today ~179 unbatched `draw_indexed` per eye, each with its own
`set_vertex_buffer` + `set_index_buffer`. Pack section meshes into a shared
vertex/index arena and issue `multi_draw_indexed_indirect`. Cuts CPU draw
submission and GPU draw-call overhead.

**Tips.** Do this only if Slice L shows draw/vertex-bound, or if M/N/O leave RD10
short. Playbox's instancing trick does **not** port (mclone sections are each a
unique mesh), so the arena + indirect is the right shape, not instancing.

**Watch out.** Arena allocation/fragmentation as sections stream in/out; keep the
existing incremental upload/removal budget working against the arena. Multiview
interaction: the indirect path must work under both per-eye and the two-layer
multiview swapchain.

## Slice P — Greedy meshing

**Idea.** Merge coplanar faces that share light/AO/tint/texture into larger
quads, cutting the `1.35M` indices/eye. Helps both CPU encode and GPU
vertex/raster. Deliberately deferred in tactical 023 pending lighting, AO,
liquids, and render layers — those are now largely landed, so it is revisit-able.

**Tips.** Merge is only valid across faces with **matching corner light + AO +
tint** (otherwise interpolation across the merged quad changes appearance). The
texture atlas complicates UV tiling on a merged quad — may need array textures or
a tiling-aware UV scheme.

**Watch out.** This is a meshing divergence from vanilla (which emits per-face
quads); document it per the reference-porting policy and keep a path back.
Appearance must be pixel-identical. Bigger change — sequence it after the cheap
GPU levers.

## Slice Q — Application SpaceWarp (AppSW)

**Idea.** Render the app at 36 Hz and let the compositor reproject to 72
(`compositor/spacewarp_mode` is `0` today). A "buy headroom" lever orthogonal to
everything above; mostly-static terrain is near its best case.

**Tips.** Needs `XR_FB_space_warp` enabled plus per-frame **motion-vector + depth**
submission. Evaluate only if Slices L-P cannot reach the target distance.

**Watch out.** Disocclusion artifacts at edges, head-motion wobble/judder, and a
hard dependency on accurate motion vectors (camera and any moving entities).
Quality risk is real in VR — treat as last resort, behind a toggle.

## Slice R — Ship-distance decision and dynamic render distance

**Idea.** RD8 likely holds solid 72 Hz with what already landed, but RD7/RD8 are
not yet benchmarked. Add frozen `rd7`/`rd8` lanes to find the real solid-72 Hz
distance, then ship a **dynamic render distance** (e.g. default ~RD8, RD10 opt-in
"experimental") plus dynamic foveation/resolution that ramps with measured GPU
load.

**Tips.** This is the pragmatic shipping answer regardless of how far L-Q get,
and it can proceed in parallel with them. The 099 open question "what render
distance do we ship at solid 72 Hz?" resolves here.

**Watch out.** Don't let dynamic RD/scale hide a regression in the fixed-RD10
lanes — keep the frozen RD10 benchmark as the standing stress lane even after a
lower default ships.

## Validation expectations

- Each slice: frozen RD10 A/B (`:frozen:rd10:metrics`) with within-run or matched
  back-to-back runs; record a row in
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  with the commit, pose, and marker block.
- Keep multiview timing markers in every Quest perf summary so results stay
  attributable.
- Headset visual signoff for anything that changes pixels (M/N/O/P/Q).
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
  --target aarch64-linux-android` plus the relevant `mclone-render` /
  `mclone-xr-scene` tests before each on-device run.
