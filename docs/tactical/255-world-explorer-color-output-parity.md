# Tactical 255: World Explorer Color Output Parity

Status: completed 2026-07-26.

Topics:

- `platform-host-boundary`
- `world-view-navigation`
- `procedural-horizon-clipmap`

Parent:

- [`253`](253-world-explorer-cross-host-parity.md) sequences the complete
  cross-host parity campaign.

Related implementation record:

- [`100`](100-render-color-profiles.md) established explicit shared render
  color profiles and target transforms for the full game renderer.

## Objective

Make World Explorer color an explicit shared renderer decision rather than an
accidental consequence of the platform surface format.

The accepted visual baseline is the current darker browser appearance:
display-space terrain colors written directly to an ordinary UNORM target.
An sRGB target must compensate so its visible output matches that baseline.

## Current Problem

Both Explorer hosts ask for an sRGB surface first. Native desktop commonly
receives `Bgra8UnormSrgb`, while the headed Chrome WebGPU probe on 2026-07-26
reported only `Rgba8Unorm`. The same terrain shader values are consequently:

- transformed by the native sRGB target and shown brighter; but
- stored directly by the browser UNORM target and shown darker.

The geometry, terrain source, palette, textures, and directional-light shader
are shared. The visible palette difference is output transfer, not separate
world generation.

The full game already owns `RenderColorProfile`,
`RenderTargetColorTransform`, surface-format preference, and matching shader
transforms. The lightweight Explorer currently bypasses that contract because
its dependency firewall intentionally excludes the complete render stack.

## Binding Color Contract

Define one shared target-independent output policy:

```text
selected display-space terrain color
        |
        +-- ordinary UNORM target: identity
        |
        +-- sRGB target: decode before target encoding
        |
        v
same visible dark Explorer appearance
```

Apply the policy consistently to:

- terrain fragments;
- river/material overrides;
- tree proxy fragments once vegetation is enabled;
- clear/background color; and
- any future procedural-horizon pass sharing the target.

Do not fix this with CSS filters, browser canvas styling, target-specific
palette constants, duplicated shaders, or a brightness change in worldgen.

## Shared Ownership

Do not import the full `mclone-render` crate into the Explorer merely to reuse
one enum and transfer function.

Before implementation, inspect whether the existing small color-profile and
target-transform contract can move to a lightweight shared rendering owner
that:

- is consumable by `mclone-render` and `mclone-terrain-view`;
- keeps WGPU/platform surface construction in app adapters;
- lets `mclone-render` re-export its existing public vocabulary where useful;
- does not import client, scene, UI, networking, or gameplay dependencies; and
- contains one authoritative CPU and WGSL transfer definition or generated
  equivalent.

If extraction is disproportionately invasive, record the alternative before
adding a terrain-view-local contract. Duplicating the existing transfer
semantics silently is not acceptable.

## Implementation Order

1. Capture the selected dark browser baseline and surface-format diagnostics.
2. Extract or expose the minimal shared color-output contract.
3. Pass the selected profile/transform into the procedural horizon renderer.
4. Apply the final transform to every Horizon color-producing pipeline.
5. Configure native, browser, and offscreen hosts through the same product
   profile while retaining platform-owned surface selection.
6. Add format-pair tests that render the same inputs through UNORM and sRGB
   targets and compare their visible/readback-normalized result.
7. Capture and inspect equivalent native and headed-browser views.

## Acceptance

- The current darker browser appearance remains the selected visual baseline.
- Native desktop no longer becomes brighter merely because its surface is
  sRGB.
- Browser and native report the same explicit color profile while truthfully
  reporting different physical surface formats when applicable.
- Equivalent pinned captures have matching terrain, water, sand, vegetation,
  snow, lighting, and background color within a documented tolerance.
- Offscreen UNORM and sRGB-target fixtures prove the transfer independent of
  OS/browser screenshot color management.
- No browser JavaScript or CSS participates in color selection or correction.
- Terrain Lab and the full game do not receive an accidental profile change.
- The Explorer dependency firewall remains focused and records any newly
  allowed lightweight shared rendering crate.

## Implementation Record

The accepted browser baseline was captured before implementation at
1,280 by 720 pixels. Its SHA-256 was
`384831ef9bbede9a7ecb22818cb6860562b97223266a3c1077a773098e7c8ea6`;
the completed headed-browser capture has the same hash.

`mclone-render-color` is now the small authoritative owner for:

- `RenderColorProfile` and `RenderTargetColorTransform`;
- CPU sRGB encode/decode and target-format selection;
- the generated WGSL transfer implementation; and
- strict WGSL marker replacement for fixed or uniform-selected transforms.

`mclone-render` re-exports its existing public color vocabulary and injects
the shared WGSL into chunk and grass variants. `mclone-terrain-view` injects
the same WGSL into terrain/material/river and tree-proxy fragments and applies
the matching CPU transform to the clear color. The old terrain-view
constructor remains identity-output for Terrain Lab. World Explorer alone
selects `vanilla`, then derives the physical-target transform in its shared
session.

The dependency firewall explicitly admits `mclone-render-color`. It still
rejects `mclone-render`, `mclone-scene`, `mclone-ui`, and the other game
runtime crates.

Diagnostics now report all three relevant facts:

| Host | Product profile | Physical format | Target transform |
|---|---|---|---|
| Headed Chrome/Wayland | `vanilla` | `Rgba8Unorm` | `identity` |
| Native Wayland/Vulkan | `vanilla` | `Bgra8UnormSrgb` | `srgb-decode` |
| Native offscreen | `vanilla` | `Rgba8UnormSrgb` | `srgb-decode` |

The explicit GPU format-pair fixture renders a display-space swatch over a
display-space clear through `Rgba8Unorm` and `Rgba8UnormSrgb`. Their readback
channels match within one encoded byte.

Pinned native and browser captures used seed `12345`, center `(-304, 336)`,
4,096 blocks across, perspective 3D, yaw `pi/4`, pitch `0.52`, and
1,280 by 720 pixels. Both were inspected. ImageMagick comparison reported
normalized MAE `0.00152386` and RMSE `0.00870103`, within the acceptance
tolerance of MAE `1/255` and RMSE `3/255`. Native vegetation remained enabled
while browser vegetation remained deferred to Tactical
[`256`](256-shared-horizon-vegetation-worker-topology.md), accounting for the
small tree-proxy differences without changing terrain, material, river,
lighting, or background color.

Validation:

```text
cargo test -p mclone-render-color
cargo test -p mclone-render-color --test wgpu_format_pair -- --ignored
cargo test -p mclone-render -p mclone-terrain-view --lib
cargo test -p mclone-render grass::tests::all_grass_pipeline_variants_validate_on_gpu -- --ignored
cargo test -p mclone-world-explorer
cargo check -p mclone-terrain-lab
cargo check -p mclone-world-explorer --lib --target wasm32-unknown-unknown
pnpm native:world-explorer:deps
pnpm native:world-explorer:web:build
pnpm host:check
pnpm native:world-explorer:web:smoke
pnpm native:desktop-offscreen:smoke
```

The native surface and offscreen commands additionally captured and validated
direct depth alongside the inspected color outputs. The full-game offscreen
capture and all six full-renderer grass pipeline variants were also inspected
or GPU-validated after the WGSL extraction.

## Non-Goals

- Adding a brightness slider, HDR, tone mapping, PBR, fog, or full game
  lighting.
- Retuning terrain material colors to compensate for a target-format bug.
- Selecting an artistic bright profile.
- Requiring identical encoded swapchain bytes when the surface formats differ;
  visible output is the parity target.
