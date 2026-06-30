# Mclone Texture Lab

Disposable TypeScript lab for authoring an original Minecraft-style texture
pack. The first block preview is a deterministic software isometric renderer;
browser-based previews remain optional later tooling.

The goal is to make it cheap for an agent or human to generate, preview,
validate, and revise block textures without distributing Mojang-owned PNGs.
This lab is an authoring and review tool first. Runtime integration should come
later through shared asset contracts, not through direct dependency on preview
code.

## Intent

Mclone needs original block textures for any distributable build. The vanilla
1.17.1 assets remain useful as local, gitignored reference material for model
shape, tint behavior, atlas semantics, mipmaps, and renderer parity, but they
must not become source art for a public pack.

The texture lab should support a fast loop:

1. Author a small, reviewable source file for a block or block family.
2. Render 2D texture views, tiling views, and block/terrain previews.
3. Capture deterministic review sheets under `/tmp`.
4. Export derived PNG/resource-pack payloads only after the source passes
   visual review.

Current first loop:

```sh
pnpm texture-lab:install
pnpm texture-lab:typecheck
pnpm texture-lab:export
pnpm texture-lab:runtime-compat
pnpm texture-lab:pack-overlay
pnpm texture-lab:coverage
```

For authoring rules, palette discipline, tint roles, and AI-agent brief shape,
see [`ILLUSTRATOR_GUIDE.md`](ILLUSTRATOR_GUIDE.md).

Accepted pack source is committed under:

```text
tools/texture-lab/packs/mclone-default/texture.ts
tools/texture-lab/packs/mclone-default/block/dirt.ts
tools/texture-lab/packs/mclone-default/block/grass-block.ts
```

These TypeScript files are the source of truth. They define palettes, tint
roles, seeded procedural layers, and ASCII masks. The generated PNGs and review
sheets are deterministic derived artifacts and stay out of git by default.

The export command writes the `mclone-default` overlay pack PNGs plus review
sheets to:

```text
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/dirt.png
/tmp/mclone-texture-lab/dirt-sheet.png
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/grass_block_top.png
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/grass_block_side.png
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/grass_block_side_overlay.png
/tmp/mclone-texture-lab/pack/assets/mclone/textures/block/grass_block_bottom.png
/tmp/mclone-texture-lab/grass-block-sheet.png
/tmp/mclone-texture-lab/grass-block-side-sheet.png
/tmp/mclone-texture-lab/mclone-default-metadata.md
/tmp/mclone-texture-lab/mclone-default-metadata.json
```

The runtime-compatible export also writes vanilla-path texture overrides:

```text
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/dirt.png
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_top.png
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_side.png
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_side_overlay.png
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/grass_block_bottom.png
```

This is a development bridge, not a standalone distributable pack. It overrides
PNG textures while the native engine still reads local vanilla blockstate/model
JSON from `reference/minecraft-1.17.1/extracted/` or `extracted.zip`. To try it
with the native runtime:

```sh
MCLONE_FIRST_PARTY_ASSET_ROOT=/tmp/mclone-texture-lab/runtime-pack pnpm native:timedemo:smoke
```

The packed overlay command builds the same overrides into a first-party-only
`.pbp` file without reading the local Mojang extraction:

```text
/tmp/mclone-texture-lab/mclone-default-overlay.pbp
/tmp/mclone-texture-lab/mclone-default-overlay.pbp.json
```

To try the packed overlay:

```sh
MCLONE_ASSET_OVERLAY_PACK=/tmp/mclone-texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
```

The coverage command compares that overlay pack against the texture materials
the native terrain atlas currently requests:

```text
/tmp/mclone-texture-lab/mclone-default-overlay-coverage.md
```

## Source Format

Use a constrained TypeScript DSL, following the same broad pattern as
`tools/asset-lab`:

- TypeScript gives editor completion and type checking.
- The DSL gives agents a limited vocabulary instead of free-form renderer code.
- Preview/render code is only tooling implementation.
- Exported files are derived artifacts; the editable source remains compact and
  reviewable.

The first source format is hybrid:

- Indexed ASCII textures for small pixel-art masks, icons, overlays, and
  simple 16x16 or 32x32 surfaces.
- Palette ramps for related colors such as grass greens, dirt browns, stone
  grays, sand yellows, and bark rings.
- Small procedural texture helpers for natural materials: wraparound noise,
  speckles, pebbles, scratches, cracks, roots, moss, and edge blending.
- Pack-level tint roles, such as `grass`, with normal and alternate review
  colors.
- Texture-level source categories: `final-color` or `tintable`.
- Texture-level `tintRole` links for neutral source art that needs a shared
  tint relationship.
- Explicit masks and overlays for blocks with multiple layers, such as grass
  side base plus tinted overlay.
- Cube block bundles for reviewing a full block from named texture roles.
- Preview-only metadata for showing alpha on a checkerboard, controlling tiling
  mode, or disabling cube/rotation panels where they are misleading.

Tintable textures are source-first in review sheets. The enlarged pixel panel
shows the raw, tintless source texture; repeat, mip, rotation, and block-context
panels use the preview tint when one is defined. Grass side textures should be
judged in block context because the final side is dirt base plus a tinted
transparent overlay, not either source texture alone.

Each export also writes a metadata report next to the sheets. The Markdown
report is for quick human review; the JSON report is for future agent/tool
checks. Both list tint roles, texture source categories, preview tiling modes,
seam diagnostic modes, sheet paths, and block face composition.

The `mclone-default` grass block module encodes that relationship directly:

```ts
tint("grass", {
  normal: "#79b34e",
  alternates: ["#5fa343", "#98b85e", "#6fa35b"],
});

texture("grass_block_top", {
  source: "tintable",
  tintRole: "grass",
  // ...
});
```

Pure ASCII is useful, but it should not be the only tool. A 32x32 natural block
is 1024 cells per face; asking an agent to hand-paint every cell is slow and
usually less coherent than combining a small palette, a few masks, and seeded
tileable noise.

## Resolution

Default original source resolution should start at 32x32 per vanilla block
face. Vanilla 1.17.1 block textures such as dirt, stone, and grass are commonly
16x16, so every 32x32 texture must also be reviewed at 16x16 and at generated
mip levels. A texture that looks good only at 32x32 is not good enough for the
game view.

The lab should make resolution explicit per texture:

```ts
texture("dirt", {
  size: 32,
  palette: "dirt",
  layers: [
    noise({ scale: 5, colors: ["base", "shadow", "warm"] }),
    speckles({ density: 0.16, colors: ["dark", "light"] }),
  ],
});
```

## Block Bundles

Author per block or small block family, not as one huge texture file.

Useful first bundles:

- `dirt`: single tile, rotation/mirror preview.
- `grass_block`: top, side, bottom, tinted side overlay, biome tint previews.
- `stone`: tile plus mirrored/rotated variants.
- `oak_log`: side, top rings, axis rotation previews.
- `sand`: subtle tile, mip/noise review.
- `leaves`: alpha/cutout preview and foliage tint.

A grass block bundle should model the real rendering problem:

- bottom: dirt-like tile
- top: biome-tinted grass surface
- side: dirt/grass transition
- overlay: biome-tinted transparent grass fringe
- snowy variant placeholder later

## Preview Modes

The review sheet should include:

- 1x and enlarged pixel view with optional grid.
- 3x3 or 5x5 seamless tile view.
- seam view that highlights left/right and top/bottom discontinuities.
- 16x16 downsample and mip chain preview.
- isolated software-rendered cube in three-quarter view.
- small terrain patch preview, because repetition problems often appear only
  across many blocks.
- rotation and mirror preview for blocks that rely on deterministic model
  variants.
- optional biome tint swatches for grass, foliage, and water-tinted textures.
- optional local-only vanilla reference panel when the gitignored reference
  assets exist; never export or commit that panel as pack source.

Screenshots and review sheets should be written under `/tmp/mclone-texture-lab/`
by default.

Biome tint previews should stay secondary. The grass block sheet uses one
primary tint for texture review and keeps alternate tint swatches compact so
they do not distract from source texture quality.

The individual texture sheet seam panel shows the texture repeated across the
wrap boundary. Red/orange marks left-right edge mismatch; blue marks top-bottom
edge mismatch. Textures marked with horizontal-only tiling show only the
left-right diagnostic.

## Variants And Rotation

Vanilla commonly hides repetition by deterministic model rotation or mirroring,
not by shipping many texture tiles for every block. That works well at 16x16
because the art is coarse and rotational artifacts are less obvious. At 32x32,
auto-rotation can reveal directional details, broken shadows, or inconsistent
material grain.

The lab should test both:

- auto-rotation/mirroring of a single texture
- multiple authored variants selected with stable per-block-position randomness

Do not assume one approach wins globally. Dirt may work with rotation only.
Stone may need mirror-safe texture construction. Grass top may need a few
variants. Logs and planks may need orientation-aware authoring instead of
rotation.

If variants are exported for runtime use, they should use stable block-position
selection so the world does not shimmer frame to frame and so multiplayer
clients agree on visible results.

## Export Targets

Initial exports should be derived and disposable:

```text
/tmp/mclone-texture-lab/pack/
/tmp/mclone-texture-lab/runtime-pack/
/tmp/mclone-texture-lab/mclone-default-overlay.pbp
/tmp/mclone-texture-lab/mclone-default-overlay-coverage.md
/tmp/mclone-texture-lab/sheets/
```

The checked-in original pack source tree lives under
`tools/texture-lab/packs/mclone-default/`. Generated PNG export still goes to
`/tmp` by default. Do not place generated review screenshots in the repo.

Runtime adoption should go through shared owners such as `mclone-assets`,
`mclone-render-session`, and `mclone-render`. The desktop app should only gain
platform glue if needed for choosing or staging a pack.

## Legal Policy

- Do not commit Mojang texture PNGs.
- Do not trace, recolor, or mechanically transform Mojang textures into source
  art.
- Local vanilla assets may be used for private visual reference and renderer
  parity checks only.
- Checked-in source art must be original to mclone or explicitly licensed for
  redistribution.

## Non-Goals

- No runtime integration in the first slice.
- No full resource-pack manager.
- No arbitrary image-generation prompt log as the source of truth.
- No Photoshop-style binary source files as the primary editable format.
- No dependency from native runtime crates on Three.js or texture-lab preview
  code.
