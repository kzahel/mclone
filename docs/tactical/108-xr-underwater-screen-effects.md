# 108: XR Underwater Screen Effects

## Goal

Bring the existing flat-screen underwater visual cues into shared XR rendering
without changing OpenXR runtime projections by default.

Flat screen already detects eye-in-water, narrows camera FOV, applies water fog,
and draws the vanilla `textures/misc/underwater.png` screen effect. XR currently
renders stereo terrain with no water fog or overlay. The XR slice should reuse
the shared renderer pieces where practical, while keeping headset comfort ahead
of exact flat-screen projection parity.

## Reference

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/GameRenderer.java`
  multiplies water/lava FOV by `0.85714287` in the normal perspective path.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ScreenEffectRenderer.java`
  draws `textures/misc/underwater.png` as a full-screen water overlay when the
  player eye is in water.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/FogRenderer.java`
  applies underwater fog with the water-vision distance ramp.
- Native flat uses `mclone-render::screen_effect::UnderwaterOverlay` and
  `mclone-app-runtime::frame_render` for the same screen effect, fog, and FOV
  plumbing.

## Design

Do not modify OpenXR projection matrices for the first pass. XR
`ChunkRenderView`s are marked `ChunkProjectionKind::External`, and the shared
underwater FOV helper intentionally ignores external projections. The OpenXR
runtime owns the per-eye projection/lens contract; applying the flat
`0.85714287` multiplier there is a comfort risk and should remain a separate
experiment if ever needed.

Instead, XR should:

- detect water against headset view positions,
- reuse `UnderwaterEffectState` for the water-vision/fade timing,
- pass `UnderwaterOverlay` into the shared full-frame path so terrain and actors
  receive underwater fog,
- draw the vanilla underwater texture into each eye target after terrain,
- leave XR selection outlines and world-space UI above the overlay, matching the
  current flat ordering.

## Detection Modes

The default mode is `midpoint`.

- `midpoint`: test the midpoint between left and right eye positions. If the
  midpoint is underwater, both eyes receive the same overlay and fog. This avoids
  one-eye flicker at the waterline and should be the comfort default.
- `per-eye`: test each eye independently. This is useful for validation and
  waterline behavior experiments, but it may be uncomfortable if one eye toggles
  while the other remains clear.

Expose both modes as `--xr-underwater-mode midpoint|per-eye` on desktop OpenXR
and Android XR launch paths.

## Slice

- Add `XrUnderwaterDetectionMode` to `mclone-xr-scene::XrSceneOptions`.
- Add XR screen-effect resources and per-mode underwater effect state to
  `XrMcloneTerrainState`.
- Thread the asset source into shared XR scene construction so the underwater
  texture is loaded by the shared XR owner, not app-local code.
- Pass midpoint/per-eye overlays into the existing shared
  `render_full_frame_for_view*_in_slot` calls.
- Keep FOV unchanged in XR because the view projection remains external.
- Add parser/tests for the new mode in desktop XR and Android XR.

## Validation

Minimum:

- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-android-xr-client`

Headset follow-up:

- Desktop OpenXR midpoint:
  `scripts/start-xr.sh --smoke mclone --xr-underwater-mode midpoint --view-pose ...`
- Desktop OpenXR per-eye:
  `scripts/start-xr.sh --smoke mclone --xr-underwater-mode per-eye --view-pose ...`
- Quest midpoint/per-eye through:
  `android-xr/validate-quest-openxr.sh --xr-underwater-mode midpoint|per-eye ...`

## Next Step

After the shared effect is visible, capture/inspect headset behavior around a
waterline. If `per-eye` is visibly useful but uncomfortable, keep it as a debug
mode only and consider a small hysteresis band around midpoint detection instead
of changing XR projection FOV.
