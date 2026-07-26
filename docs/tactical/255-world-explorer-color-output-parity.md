# Tactical 255: World Explorer Color Output Parity

Status: proposed.

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

## Non-Goals

- Adding a brightness slider, HDR, tone mapping, PBR, fog, or full game
  lighting.
- Retuning terrain material colors to compensate for a target-format bug.
- Selecting an artistic bright profile.
- Requiring identical encoded swapchain bytes when the surface formats differ;
  visible output is the parity target.
