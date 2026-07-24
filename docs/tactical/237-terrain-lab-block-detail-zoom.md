# Terrain Lab Block-Detail Zoom

Status: completed 2026-07-24, including inspected desktop/phone
block-detail pixels and hosted desktop/mobile headed-WebGPU validation.

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

## Implementation Receipt

Terrain Lab now accepts every integer viewport width from one through 131,072
blocks. Below one 64-cell procedural tile, the viewport planner retains an
aligned spacing-one tile and crops its presentation to the requested logical
footprint. Auto detail therefore reaches one sample per block without
introducing a special storage shape or sub-block terrain samples.

The shared Rust projection now derives its focus height from the production
display surface at the requested center. Map view holds a safe elevated camera
and narrows its field of view so the focus plane frames exactly the requested
footprint. 3D view retains the established overview orbit at 96 blocks and
above; below that threshold it holds a safe orbit distance and narrows the
field of view continuously down to one block. Canonical, CPU LOD, GPU LOD, and
point inspection consume the same projection.

Integer centers now denote the center of a complete block column rather than
the corner shared by four blocks. Canonical and procedural top planes use the
same visible block-top convention. This removed the close-view half-block and
one-block vertical discrepancies without changing production terrain
generation.

Canonical close magnification exposes the existing first-party 16-by-16 block
atlas texels as crisp squares. The same sampler continues to apply linear
minification and mip interpolation for overview scales. Procedural panes
remain honest geometric height surfaces at one sample per block; they do not
claim canonical side geometry or feature meshes.

Browser regression coverage proves shared button zoom from four to two to one
block, a stable one-block floor, canonical completion, CPU/GPU publication,
spacing-one procedural output, and bounded visible tiles. It captures
canonical map and 3D views plus the three-pane workspace at both desktop and
phone sizes.

The following local validation passed:

- `cargo test --manifest-path native/Cargo.toml -p
  mclone-terrain-view --lib`: 18 passed;
- `cargo check --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --target wasm32-unknown-unknown`;
- `pnpm --dir tools/terrain-lab test`: 17 passed;
- `pnpm terrain-lab:typecheck`;
- the headed-Wayland browser WebGPU probe; and
- `pnpm terrain-lab:web:test`: 8 passed and 2 intentionally skipped
  platform-inapplicable cases.

Inspected acceptance captures are:

- `/tmp/mclone-terrain-lab-desktop-chrome-block-workspace.png`;
- `/tmp/mclone-terrain-lab-phone-chrome-block-workspace.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-block-map.png`;
- `/tmp/mclone-terrain-lab-phone-chrome-canonical-block-map.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-block-3d.png`; and
- `/tmp/mclone-terrain-lab-phone-chrome-canonical-block-3d.png`.

The map captures show one complete canonical top face filling its pane with
individual atlas texels visibly magnified. The 3D captures show the same top
face and its canonical vertical side. The workspace captures show canonical,
CPU LOD, and GPU LOD panes at the same one-block footprint and the procedural
panes at effective spacing one.

The production bundle was built from product commit `696d6e9e` and uploaded
only under `/terrain/`. Each uploaded object was fetched from
`https://mclone.kzahel.com/terrain/` and byte-compared before `index.html` was
published last:

| Object | SHA-256 |
| --- | --- |
| `index.html` | `60c39040cda5ece43280218438a72c5cb8d588f8cec534b7e7e82da3b15eb925` |
| `assets/canonical-worker-BzmCAAHe.js` | `6da296ed3a8edb86ef616f7da82e5f7232d08139b8c9da15d8e3163ea09c65e6` |
| `assets/index-DAVU56jH.js` | `a6edf389b69e5c1222b1bb059193299d6c8100921b59b36ae3585fbca01ad735` |
| `assets/index-K-isbiIr.css` | `6f5faf8a7f3a76004a68ceca6aba710a6c8c93afd6ab97e22985ffdab3fbdd48` |
| `assets/mclone_terrain_lab_bg-CiUoZw5z.wasm` | `8b7061e2b1e2612674e3d6cd683a9ac6c3dc97f605cd366d3148d066e23f3cb8` |

Hosted headed-Wayland desktop and phone smokes passed the review, navigation,
exact publication, and independent cold CPU/GPU race flows with zero browser
errors. Both retained 100% ocean and visible-material agreement, negligible
continentalness error, and independent GPU publication well before the stress
CPU lane. The hosted standard smoke does not synthesize the new one-block
gesture itself; its served HTML and all content-addressed assets were
byte-verified as the exact bundle that passed the inspected local desktop and
phone block-detail E2E.

Implementation commits are `b8c5d715` (plan), `7c2aeb50` (one-block viewport
planning), and `696d6e9e` (surface-focused projection and browser acceptance).
