# 100: Render Color Profiles And Vanilla Surface Parity

Status: implemented.

## Purpose

Make renderer color style an explicit shared engine setting instead of an
accidental side effect of the platform swapchain format.

The current native renderer has two competing goals:

- match Minecraft Java 1.17.1's vanilla raster appearance for parity work
- leave room for a brighter, more stylized mclone look and later linear/PBR/HDR
  experiments

Those goals should not fight through hidden color-space behavior. The default
must stay vanilla because Java visual parity remains the correctness target for
assets, lighting, AO, fog, sky, and render-layer work. Brighter or more modern
looks should be opt-in render profiles with stable cross-platform output.

## Current Problem

The textured terrain shader ports Java's `LightTexture` math procedurally and
then writes that Java-style display-space color directly to the render target:

- `native/crates/mclone-render/src/light_texture.rs`
- `native/crates/mclone-render/src/shaders/chunk_textured.wgsl`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java`

That is correct only if the final target does not apply another sRGB transfer
curve. Several live platform paths currently prefer sRGB surfaces:

- desktop flat prefers `Bgra8UnormSrgb` / `Rgba8UnormSrgb` in
  `native/crates/mclone-render/src/native.rs`
- native web prefers sRGB first in
  `native/apps/mclone-web-client/src/web_canvas.rs`
- flat Android prefers any available sRGB format in
  `native/apps/mclone-android-client/src/lib.rs`

Headless captures use `Rgba8Unorm`, so the same scene can be materially darker
in screenshots than in an interactive window. The user-visible symptom is that
outside terrain feels too bright and caves remain visible even when the packed
light path is nominally dark. Numerically, Java's zero-light gray lift is about
`0.0588`; an sRGB presentation curve lifts that to roughly `0.27`.

This is separate from local packed-light fallback issues, where missing
neighbor/light data can still produce local `FULL_BRIGHT` samples. Those should
be fixed as lighting/meshing boundary bugs, not by darkening the whole renderer.

## Target Shape

Add an explicit shared color-profile contract owned by `mclone-render`.

Suggested initial type:

```rust
pub enum RenderColorProfile {
    Vanilla,
    StylizedBright,
    LinearExperimental,
}
```

### `Vanilla`

Default profile.

Target behavior:

- Java-style display/gamma-space atlas, vertex color, AO, and lightmap math.
- Prefer non-sRGB presentation formats: `Bgra8Unorm`, then `Rgba8Unorm`.
- If a platform can only provide an sRGB target, compensate in the final
  presentation path so the visible output matches the non-sRGB vanilla result.
- Keep the profile deterministic across desktop flat, headless, native web,
  flat Android, desktop OpenXR, and Android XR where platform APIs permit.

This profile is the correctness target for Java 1.17.1 visual parity.

### `StylizedBright`

Explicitly preserves the liked brighter look as a stable artistic option.

Target behavior:

- Start from the same classic vanilla shader inputs and packed-light values.
- Apply a deliberate brightness/transfer curve or post-process instead of
  relying on the surface format.
- Tune in renderer-owned code with named parameters, not platform conditionals.
- It may approximate the current accidental sRGB lift at first, but it should be
  documented as an aesthetic style, not as parity.

This profile is useful for later original textures and a more colorful mclone
look.

### `LinearExperimental`

Future profile, not required for the first implementation slice.

Target behavior:

- Treat texture color, lighting, fog, bloom, and effects as a linear/HDR
  pipeline.
- Decode authored sRGB textures intentionally, use a floating intermediate
  render target such as `Rgba16Float` when useful, then tone-map to SDR/HDR
  presentation.
- Do not promise Java pixel parity.

This profile exists to reserve the architecture lane for PBR/HDR without
contaminating vanilla parity.

## Ownership

Shared renderer owns:

- `RenderColorProfile`
- surface-format preference policy for renderer-created surfaces
- shader/post-process color transforms
- profile labels used by diagnostics/UI
- headless/offscreen validation behavior

App/platform crates own only:

- collecting a requested profile from CLI, UI, local storage, Android intent, or
  XR startup args
- passing the profile into shared render setup
- platform-specific fallback when a surface format is unavailable

Do not add gameplay, asset, lighting, or UI policy to platform app crates just
because one platform exposes the profile first.

## Implementation Slices

- [x] **Slice 1: shared profile contract.**
  - Add `RenderColorProfile` in `mclone-render`.
  - Add profile to textured/full-frame render options or a small render-config
    type used by the app/runtime paths.
  - Keep `Vanilla` as the default in every constructor and CLI parse path.
  - Add unit tests for default selection and stable labels.

- [x] **Slice 2: vanilla surface-format policy.**
  - Desktop flat: prefer `Bgra8Unorm`, then `Rgba8Unorm`, then sRGB fallback.
  - Native web: prefer non-sRGB when exposed by WebGPU.
  - Flat Android: prefer non-sRGB when exposed by Vulkan/wgpu.
  - Audit desktop OpenXR and Android XR swapchain format choice; prefer non-sRGB
    for `Vanilla` where runtimes support it.
  - Keep a clear warning/diagnostic when a platform is forced onto sRGB.

- [x] **Slice 3: sRGB fallback compensation.**
  - If the target is sRGB but the active profile is `Vanilla`, render through a
    small profile-aware presentation transform or internal `Rgba8Unorm` target
    so final visible pixels match the vanilla non-sRGB path.
  - Prefer one shared implementation over per-platform shader forks.
  - Ensure headless and live-window captures converge for the same profile.

- [x] **Slice 4: stylized bright profile.**
  - Add a deliberate post/light response for `StylizedBright`.
  - Start with a conservative approximation of the current accidental sRGB lift,
    then make it tunable only through renderer-owned constants or explicit
    settings.
  - Verify it remains visually distinct from `Vanilla` but is stable across
    non-sRGB and sRGB presentation targets.

- [x] **Slice 5: controls and diagnostics.**
  - Add CLI flags for desktop/headless perf/capture modes, for example
    `--render-color-profile vanilla|stylized-bright|linear-experimental`.
  - Add native UI/options plumbing only after the shared setting exists.
  - Persist the profile per platform only where there is already a clean
    preference-storage path; otherwise keep it session-local.
  - Expose the active profile in the debug overlay.

- [ ] **Slice 6: validation matrix.**
  - Capture and inspect desktop/headless `Vanilla` and `StylizedBright`
    screenshots for the same seed, time, view, lighting, and fullbright state.
  - Confirm `Vanilla` caves are materially darker than today's sRGB live
    surface path.
  - Confirm `StylizedBright` reproduces the liked brighter aesthetic without
    depending on the swapchain format.
  - Run:
    - `cargo test --manifest-path native/Cargo.toml -p mclone-render`
    - `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
    - `pnpm native:web:build`
    - `pnpm native:web:smoke`

## Non-Goals

- Do not tune block light levels, sky light propagation, or Java `LightTexture`
  constants to compensate for surface color-space mistakes.
- Do not make `StylizedBright` the default while Java visual parity is still the
  active target.
- Do not build full PBR/HDR in this slice. Reserve `LinearExperimental` as an
  explicit future lane.
- Do not require all platforms to expose all profiles before landing the shared
  contract. A platform may temporarily clamp to `Vanilla` with a diagnostic if
  the presentation path is not ready.

## Acceptance

- `Vanilla` is the default everywhere.
- Live desktop/web/Android presentation no longer gets brighter merely because
  the adapter selected an sRGB surface.
- `StylizedBright` is available as an explicit opt-in style and produces stable
  output independent of surface format.
- Headless and live captures of the same profile are meaningfully comparable.
- Future PBR/HDR work has a named profile lane and does not need to reinterpret
  vanilla parity code.

## Implementation Notes

Implemented in the native Rust workstream:

- `mclone-render` owns `RenderColorProfile`, surface-format preference, stable
  labels, and sRGB encode/decode presentation transforms.
- `Vanilla` is the default and live surfaces now prefer non-sRGB
  `Bgra8Unorm`/`Rgba8Unorm` where available.
- When terrain/sky must render to an sRGB target, `Vanilla` applies an inverse
  sRGB transform before presentation. `StylizedBright` applies the deliberate
  sRGB-style lift on non-sRGB targets so the brighter look is explicit.
- Desktop CLI/startup paths accept
  `--render-color-profile vanilla|stylized-bright|linear-experimental`.
- Debug overlays and perf reports include the active profile label.

Validation completed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`
- `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm native:web:build`
- `pnpm native:web:smoke`
- Headless screenshot inspection:
  - `/tmp/mclone-color-profile-vanilla.png`
  - `/tmp/mclone-color-profile-bright.png`

Validation still pending:

- Live-window screenshot comparison of `Vanilla` and `StylizedBright`.
