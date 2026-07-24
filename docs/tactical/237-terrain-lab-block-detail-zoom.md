# Terrain Lab Block-Detail Zoom

Status: active 2026-07-24.

Topic: `gpu-procedural-terrain`

## Objective

Let Terrain Lab move continuously from continental coverage to an individual
canonical block face and its magnified texture pixels.

The completed Lab must:

- accept shared viewport widths from one through 131,072 blocks;
- keep canonical, CPU LOD, and GPU LOD panes at one visual scale;
- continue zooming in map and 3D below the old 64/96-block floors;
- keep the close camera above and aimed at the local production surface;
- preserve whole aligned 64-cell procedural tiles while cropping their
  presentation to a sub-tile viewport;
- retain nearest texture magnification and overview mip minification;
- keep wheel, button, pinch, URL, pan, and inspection behavior coherent at
  block scale; and
- prove the result with inspected desktop and phone canonical captures.

## Originating Direction

Human review found that the exact/final pane stops zooming long before its
actual block geometry and textures become inspectable. This is not a
canonical-generation limitation.

The browser state currently clamps `blocksAcross` to 64 because the first
procedural viewport was built from 64-cell tiles. The Rust viewport planner
repeats that limit even though aligned tiles can cover and be cropped to a
smaller logical viewport. The shared projection independently clamps its
input to 16 blocks and holds the 3D camera at its 96-block distance below that
scale. Lowering only the browser constant would therefore leave rendering
stuck or fail procedural planning.

## Product Contract

### Viewport

`blocksAcross` remains an integer, URL-addressable horizontal footprint. One
block across is the close limit: a normal phone or desktop pane then gives a
single 16-pixel block texture hundreds of physical pixels, which is enough to
inspect individual source texels without inventing fractional world
coordinates.

The procedural compiler continues to own aligned 64-cell tiles at minimum
sample spacing one. A one-block viewport does not create a one-cell storage
layout or sub-block terrain samples; it draws the relevant crop of the
already aligned resident tile. Auto detail remains spacing one below 64
blocks.

### Projection

The common projection targets the production display surface at the shared
center rather than the fixed Y54 sea-level-era target.

- Map mode remains top-down and frames the requested block footprint at the
  focus plane. It uses a safe elevated perspective camera with a derived
  narrow field of view, producing orthographic-like close inspection without
  intersecting mountains or vegetation.
- 3D preserves the existing overview projection at and above 96 blocks. Below
  96 blocks it holds a safe orbit distance and narrows the field of view in
  proportion to the requested footprint. This removes the floor without
  placing the camera inside exact blocks.
- Near/far planes remain finite and valid at every supported scale.

Canonical and procedural renderers consume the same target, eye, FOV,
near/far, and logical pane aspect. TypeScript does not duplicate projection or
terrain-height rules.

The LOD height field represents the visible top plane of its selected surface
block. Close projection may correct the existing one-block centerline
difference between the height sample's block coordinate and canonical block
top, but it must do so in the shared Rust/WGSL contract and keep picking
consistent.

### Texture Detail

Canonical geometry continues to use the first-party block atlas through the
Lab overview sampler:

- nearest magnification exposes authored texels as crisp squares;
- linear minification and mip interpolation remain active while zoomed out;
  and
- close zoom does not regenerate chunks or request higher-resolution texture
  assets.

Procedural LOD remains a one-height-sample-per-block surface at its finest
level. It can stay visually aligned but does not claim canonical vertical
faces, vegetation, or feature geometry.

## Ownership

- `tools/terrain-lab/src/state.ts` owns the shared one-block logical clamp.
- `mclone-terrain-view` owns viewport acceptance, local focus, shared
  projection, uniform packing, and procedural top-plane presentation.
- `mclone-terrain-lab` adapts the projection to exact chunk rendering and
  point inspection.
- Existing WebGPU tile, mesh, and atlas owners remain unchanged.

## Implementation And Acceptance

1. Lower browser and Rust viewport minima to one and pin sub-tile plans.
2. Add a production-derived shared focus height.
3. Replace fixed close camera floors with scale-responsive map/3D projection.
4. Keep canonical rendering, procedural uniforms, and picking on that
   projection.
5. Add state, projection, planner, and browser zoom regression coverage.
6. Capture and inspect individual canonical block faces/texture texels in map
   and 3D at desktop and phone sizes.
7. Run local and hosted Terrain Lab validation, deploy only `/terrain/`
   objects, and record the receipt.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.
