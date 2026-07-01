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
frozen-RD10 frame cadence close to the 72 Hz period, but later Playbox-style
busy/free accounting showed that `frame_avg_ms` was mostly compositor pacing.
Use `MCLONE_ANDROID_XR_PERF_HEADROOM` / `app_work_*` for all future RD10
comparisons. Current scale-1.0 per-eye frozen RD10 app work is about
`13.2-13.3ms` avg and `14.1ms` p95 against a `13.889ms` 72 Hz budget, so RD10 is
still borderline; live/flight RD10 is worse.

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

Initial probes corrected the measurement path and split the cheap GPU levers:

- **Render scale is the proven GPU-side win.** Scale `0.5` on frozen RD10 drops
  app work from about `13.2ms` to `10.4ms` at 72 Hz and gives about `3.5ms`
  average headroom. It is not enough for reliable 90 Hz by itself (`~11.0ms`
  average app work, p95 over the `11.111ms` budget), but it proves real fragment
  cost.
- **Fixed-foveated rendering is wired but not useful today.** The runtime accepts
  HIGH foveation (`VrApi Fov=3`), but corrected headroom runs showed no win:
  scale-1.0 app work was `13.198ms` off vs `13.273ms` high, within noise and
  slightly worse.
- **Solid render layer landed, but it is not an RD10 performance lever.** Commit
  `0f5ecaa` split terrain into solid/cutout/translucent mesh ranges and renders
  solid terrain through a no-`discard` shader by default. The headset run on
  commit `0f5ecaa` measured `13.720ms` app-work avg / `14.653ms` p95, worse than
  the `~13.2-13.3ms` / `~14.1ms` baseline target. Keep the split for
  vanilla-shaped render-layer correctness, but do not count it as a Quest RD10
  perf win unless a later within-run A/B proves otherwise.
- **Frame overlap is landed as an opt-in frozen-RD10 win.** Commit `1977a67`
  added `--xr-frame-overlap` for the per-eye Android XR path. In a matched
  frozen RD10 A/B it moved app work from `12.685ms` avg / `13.554ms` p95 to
  `10.959ms` avg / `11.686ms` p95, with `0.0%` app-over-period and about
  `2.93ms` average headroom. Frozen render has no live runtime/upload work, so
  this proves the deferred-stereo-wait/pacing part first; live stationary/flight
  validation and comfort/latency signoff are still needed before making it
  default.

## Baseline to beat

Frozen RD10, masked per-eye default path (commit `ecb502f`), fixed pose
`0,80,-96,180`, budget `13.889ms`:

| Field | Value |
|---|---:|
| `app_work_avg_ms` / p95 | `~13.2-13.3ms` / `~14.1ms` |
| `app_over_period` | `~10-12%` in recent scale-1.0 per-eye runs |
| Legacy frame-wall p50 / p95 / p99 | `13.746` / `15.166` / `18.813ms` |
| Drawn sections / indices (per eye) | `179` / `1.356M` |
| True GPU floor (E1) | `~10.7ms` |
| CPU after F/G/H | `~4ms` |

Lanes: `native:android-xr:perf:frozen:rd10` and `:frozen:rd10:metrics`. RD1 and
RD5 already hold solid 72 Hz; RD10 is the stress lane.

## Cross-cutting tips and things to watch out for

- **Do not trust the Meta `app/gpu_frametime` counter** for GPU attribution — it
  under-reports by ~35% here. Trust an in-stream wgpu timestamp or the stereo
  poll-wait (E1 proved the poll *is* GPU execution).
- **Do not use `frame_avg_ms` or legacy `over_budget` to judge headroom.** They
  include OpenXR pacing wait, so they collapse toward the refresh period at 72
  Hz or 90 Hz. For perf wins, compare `MCLONE_ANDROID_XR_PERF_HEADROOM`
  `app_work_*`, `headroom_*`, and `app_over_period_*`; the main
  `MCLONE_ANDROID_XR_PERF_SUMMARY` also repeats the key app-work fields.
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
| 2 | **M** — render-scale | Render eyes below native resolution; landed as a probe and showed real headroom | ~hrs | low |
| 3 | **N** — fixed-foveated rendering | Applied successfully, but measured no app-work win; keep only as a possible dynamic/quality lever | ~hrs | low |
| 4 | **O** — solid render layer | Landed for parity; measured no RD10 app-work win at scale 1.0 | done | keep/default |
| 5 | **K (E4)** — CPU/GPU frame overlap | Opt-in per-eye path landed; frozen RD10 app work improved, live/comfort validation pending | done+validate | latency/comfort |
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
instead of native. Quest titles do this routinely. The landed probe confirms it
buys real headroom: at scale `0.5`, frozen RD10 app work dropped to about
`10.4ms` at 72 Hz with no app-work over-period frames.

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
level (start HIGH), and apply it via `xrUpdateSwapchainFB` with
`XrSwapchainStateFoveationFB`. This is now wired and accepted by the runtime, but
the corrected headroom runs show no measurable win on the current Quest 3 RD10
path.

**Tips.** `XR_FB_foveation_vulkan` is required for the Vulkan path and the API
path is already present. `VrApi Fov=3` confirms HIGH was accepted, but that is
not enough: keep judging it by `app_work_*` / `headroom_*`, not successful API
calls. Dynamic foveation may still be useful as a quality/thermal policy later,
but do not treat fixed HIGH as an RD10 perf lever.

**Watch out.** Foveation is per-swapchain — confirm it applies to the two-layer
multiview swapchain as well as the per-eye swapchains. Check the `openxrs` crate
exposes these calls (may need a raw function pointer). Periphery shimmer/aliasing
at HIGH; tune the level. Reserve `debug.mclone.xr_foveation` (already noted as a
future property in 083) for level control.

## Slice O — Solid render layer (drop `discard`) + fragment-cost audit

**Status.** Landed in `0f5ecaa`; measured on Quest 3 and recorded in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).
This should stay as the default render path for vanilla-shaped render-layer
correctness, but it should be treated as **not a measured RD10 performance win**:
`native:android-xr:perf:frozen:rd10:metrics` reported `13.720ms` app-work avg,
`14.653ms` p95, and `37.8%` app-over-period at RD10. Drawn work stayed at the
masked per-eye baseline (`179` sections / `1.356M` indices), so the change only
altered shader/pipeline selection.

**Idea.** Split opaque terrain into vanilla-shaped `solid` (no alpha test) and
`cutout`/`cutout_mipped` (alpha test/`discard`) layers, and give `solid` a
pipeline **without `discard`** so the bulk of terrain regains early-Z /
hidden-surface removal on the tile GPU. While in the shader, audit the
`apply_color_profile` path: mode `> 1.5` runs **3x `pow()` per fragment** for
sRGB encode — if a color profile is active on Quest, that is pure fragment cost
that could be precomputed into the atlas or approximated.

**Tips.** `reference/minecraft-1.17.1` `RenderType` + `ItemBlockRenderTypes` is
the authority: most blocks = `solid`, leaves = `cutout_mipped`, glass = `cutout`,
water/ice = `translucent`. The mesh now carries solid/cutout/translucent ranges,
and per-block layer assignment lives next to the existing model/`solid_render`
facts.

**Watch out.** Only route fully-opaque textures through the no-`discard` solid
pipeline (alpha is ignored there). `cutout_mipped` (leaves) needs mipmaps on vs
`cutout` (no mip) — different sampler/UV-shrink behavior; match vanilla. This is
a render-layer parity path now, not a reason to keep spending RD10 optimization
time here unless a future run exposes a visual or layer-classification bug.

## Slice K (E4) — CPU/GPU frame overlap (carried from 106)

**Status.** Commit `1977a67` landed `--xr-frame-overlap` as a default-off
Android XR per-eye path and added
`native:android-xr:perf:frozen:rd10:frame-overlap`. The path defers both eye
submission waits into one stereo wait and caches live runtime/render-section
prefetch work so the next frame consumes it instead of polling twice. It is
rejected with multiview/proof lanes and with the older overlap probe flags.

Frozen RD10 result:
[`quest-standalone-performance-records`](../quest-standalone-performance-records.md)
shows a matched A/B on `1977a67`: default per-eye `12.685ms` avg /
`13.554ms` p95 / `1.5%` app-over-period versus frame-overlap `10.959ms` avg /
`11.686ms` p95 / `0.0%` app-over-period. A first overlap run was consistent at
`10.967ms` avg / `11.734ms` p95. This is enough to keep the flag and continue,
but frozen render disables runtime polling/upload, so `MCLONE_ANDROID_XR_PERF_OVERLAP`
was `0.000ms`; it has not yet proven the live N+1 runtime-prefetch half.

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

**Watch out.** Keep it default off until live stationary/flight RD10 and a
headset comfort check are done. Motion-to-photon in the frozen A/B improved on
the sampled Meta counter (`35.487ms` default vs `23.011ms` confirmation), but the
Meta counter is noisy enough that this should be treated as a comfort-screening
prompt, not proof. The `+1.9ms` GPU contention from concurrent CPU on the
unified-memory SoC (E2) is real but bounded; validate it in live lanes. **E5**
(GPU-side semaphore / no CPU block) is the cleaner architecture but needs raw
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
