# 036 — Native sky and day/night cycle

Status: active

Replace the flat grey world-pass clear (`mclone_render::default_clear_color()`,
`native/crates/mclone-render/src/lib.rs:21`) with a Minecraft-parity sky: a
day/night color cycle, sky dome, sunrise/sunset glow, and sun/moon/stars. Clouds
and (non-parity) god rays are tracked as follow-up, not part of this slice.

## Reference anchors (1.17.1)

- `LevelRenderer.renderSky` — `reference/.../client/renderer/LevelRenderer.java:1699`.
  Draw order: sky disc (`skyBuffer`, `buildSkyDisc(+16)`) → sunrise/sunset glow
  `TRIANGLE_FAN` → sun quad (`sun.png`) → moon quad (`moon_phases.png`, 4×2 atlas)
  → stars (`starBuffer`, 1500 quads) → void disc (`darkBuffer`, y=-16, eye below
  horizon). Celestial rig is rotated by `timeOfDay * 360°`.
- `LevelRenderer.renderClouds` — `:1816` (follow-up only).
- Color math in `ClientLevel`: `getSkyColor` `:555`, `getCloudColor` `:598`,
  `getStarBrightness` `:629`, `getSkyDarken` `:545`.
- `DimensionSpecialEffects.getSunriseColor` — `:40` (glow only within ±0.4 of the
  dawn/dusk sun angle).
- Time: `DimensionType.timeOfDay(dayTime)` — `:432`:
  `frac = frac(dayTime/24000 - 0.25); angle = (frac*2 + (0.5 - cos(frac·π)/2))/3`.
  `Level.getSunAngle` — `:422`.

## Current native state

- World pass clears to a fixed dark color, then draws chunks
  (`native/apps/mclone-native-client/src/app.rs:123`). No sky geometry, no
  celestial bodies, no time-of-day clock anywhere in `native/`.
- `IntegratedServer` has `simulation_tick: u64` advancing per tick
  (`integrated.rs:136`) but no world `dayTime`.
- `ServerUpdate` (`mclone-protocol/src/lib.rs:31`) has no time packet.

## Phasing

### Phase 0 — day-time clock + protocol sync (no pixels) — DONE
- `mclone_core::time` (`time_of_day`, `sun_angle`), oracle-formula tested.
- `IntegratedServer.day_time` starts at 1000, advances per simulation tick, emits
  `ServerUpdate::TimeUpdate { day_time }` each tick (protocol v3).
- `ClientRuntime` tracks `day_time` and exposes `time_of_day`/`sun_angle`.
- `--day-time <ticks>` capture override on the headless screenshot path.

Original plan:
- Port `timeOfDay` / `getSunAngle` into `mclone-core` (or a small sky module),
  unit-tested against the oracle formula.
- Add authoritative `day_time: u64` to `IntegratedServer`, advancing each tick
  (vanilla starts at 1000). Emit `ServerUpdate::TimeUpdate { game_time, day_time }`
  periodically (vanilla cadence: every 20 ticks); add codec tag + round-trip test.
- Client tracks `day_time`, advances locally between updates, exposes
  `time_of_day: f32` (0..1) and `sun_angle` to the renderer.

### Phase 1 — gradient sky clear color (kills the grey) — DONE
- `mclone_render::sky` ports `Mth.hsvToRgb`, `VanillaBiomes.calculateSkyColor`
  (plains temp 0.8 → `0x78A7FF`, asserted), and the `getSkyColor` day factor.
- World-pass clear driven by `overworld_clear_color(time_of_day)` across the live,
  headless, and perf render paths.
- Validated: noon = plains blue, dusk = dimmed blue, midnight clamps to black.

### Phase 2 — sky dome + sunrise/sunset glow — DONE
- `mclone_render::sky_render::SkyRenderer` (new module + `shaders/sky.wgsl`) draws
  the `buildSkyDisc(+16)` disc (flat-tinted by the day/night sky color) and the
  `getSunriseColor` glow `TRIANGLE_FAN`, with the celestial rig rotation baked into
  camera-relative vertices. Position-color pipelines, no depth attachment; the disc
  is opaque and the glow blends additively (`SRC_ALPHA, ONE`). Fans are expanded to
  indexed triangle lists (wgpu has no fan topology).
- `mclone_render::sky::sunrise_color` ports `DimensionSpecialEffects.getSunriseColor`
  (±0.4 dawn/dusk band), asserted present at dusk / absent at noon+midnight.
- `ChunkRenderView::sky_view_projection` supplies a rotation-only (camera-at-
  infinity) view-projection; `ChunkRenderTarget::with_loaded_color` lets the chunk
  pass load the sky instead of clearing. The sky pass clears + draws before chunks
  across the live (`app.rs`), headless (`headless.rs`), and perf (`perf.rs`) paths,
  threading `time_of_day`/`sun_angle` from `WindowSceneRuntime` like `sky_clear_color`.
- Validated: dawn capture (`--day-time 23000`) shows the orange sunrise bloom on the
  sun side fading to the dimmed sky; noon is uniform plains blue; terrain composites
  over the sky correctly. Full native suite (28 binaries) + WASM web build green.

Original plan:
- New sky render pass (position-color shader, depth-write off, drawn before
  chunks): sky disc + sunrise/sunset glow fan (`getSunriseColor`). Establishes the
  reusable sky-geometry plumbing (vertex buffers, sky shader, ordering).
- Validate: capture at dawn (orange band on the sun side).

### Phase 3 — sun, moon, stars
- Visible textured sun quad — **DONE 2026-08-16**. The shared sky renderer
  resolves the selected pack's Mclone or Minecraft `sun.png`, with a generated
  proprietary-free fallback for packs that intentionally contain neither.
  The ordinary fixed celestial rig drives one quad implementation in mono,
  placed/per-eye, and full-frame multiview paths. The same CPU-side texture is
  retained across renderer-device rebuilds.
- Moon and star completion now has a bounded execution owner in Tactical
  [`314`](314-celestial-moon-stars-and-square-sun.md). It retains this
  tactical's Java 4×2 moon-atlas, seeded-star, brightness, and draw-order
  baseline while adding the explicit original-Mclone square moon,
  latitude-aware star law, per-feature cost controls, and cross-platform
  evidence.
- Validate moon/stars with captures across a full day cycle through Tactical
  314 rather than growing this historical phase checklist in parallel.

The first sun pixel gate used frozen `day_time=6000` captures. Looking straight
up produced an inspected centered textured sun at
`/tmp/mclone-t307-baseline-sun-up.png`; the ordinary terrain view remained
coherent in `/tmp/mclone-t307-baseline-sun-noon.png`. A synthetic-stereo frame
also completed through the shared per-eye sky path at
`/tmp/mclone-t307-baseline-sun-stereo.png`. These temporary paths are execution
evidence, not repository artifacts. Moon and stars remain outside Tactical
307's acceptance requirement.

This phase remains the owner of the ordinary visible sun/moon/star baseline.
Tactical
[`307`](307-seasonal-solar-path-and-cyclical-latitude.md) depends on that
visible sun and separately owns the original Mclone plane's cyclical-latitude
and orbital path. Do not duplicate the baseline celestial renderer there or
make retained vanilla-shaped profiles consume Mclone cosmology.

"Reasonably pretty" target = end of Phase 3.

## Out of scope (follow-up)
- **Clouds** (`renderClouds`): self-contained but geometry/perf-sensitive; do
  after the sky behind them reads correctly.
- **God rays**: NOT vanilla 1.17.1 — a screen-space post-process divergence from
  the parity bar. Scope separately and only after the parity sky is solid.
- Biome sky-color blending (`CubicSampler.gaussianSampleVec3`): Phase 1 uses a
  fixed plains color; revisit when biome data is wired to the client renderer.
- Rain/thunder/lightning-flash terms in the color math; void disc below horizon.
