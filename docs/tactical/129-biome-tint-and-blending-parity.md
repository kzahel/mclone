# 129: Biome Tint And Blending Parity

Status: active; first parity slices landed, remaining audit/cache/diagnostic work planned
Workstream: shared native Rust terrain mesh, render-session tint inputs, and vanilla visual parity

## Impetus

Recent biome visual work fixed one direct transcription bug in the dark-forest
grass modifier and several over-broad biome metadata groupings. That uncovered a
larger renderer-facing gap: grass, foliage, and water colors are still not
being resolved through the same path as Minecraft Java 1.17.1.

The user-visible symptom is square, patchy grass tint in seed `1124` near
block `(-51, 76, 193)`. A scratch Java oracle against the local 1.17.1 classes
and vanilla grass colormap showed:

- the nearby 25x25 block area is all `minecraft:swamp`,
- direct vanilla swamp grass uses the two expected swamp colors, `#4c763c` and
  `#6a7039`,
- vanilla default biome blending radius `2` turns those into a 5x5 block
  average, for example center direct `#4c763c` becomes blended `#54743b`,
- the old renderer applied a hash-like swamp modifier per block and did not
  average colors over the blend radius.

The swamp noise, seeded block-position biome lookup, default radius-2 blend, and
basic water blend have since landed in `mclone-mesh`. This tactical remains open
for the remaining `BlockColors` audit, upper-half plant sampling, compile-local
tint caching, and visual/debug validation.

This is not terrain generation, decoration, or feature breadth. It is a visual
parity lane for how block models and liquid faces get their tint color during
mesh building.

## Java 1.17.1 Reference Shape

Primary source files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/color/block/BlockColors.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/BiomeColors.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/color/block/BlockTintCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/FuzzyOffsetConstantColumnBiomeZoomer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSpecialEffects.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/GrassColor.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/FoliageColor.java`

Relevant behavior:

- `BlockColors` routes grass block, grass plant, fern, tall grass, sugar cane,
  foliage, and water through `BiomeColors`.
- `BiomeColors` delegates to `BlockAndTintGetter.getBlockTint(...)`.
- `ClientLevel.calculateBlockTint(...)` reads
  `Minecraft.options.biomeBlendRadius`.
- The default radius is `2`, which means a `(2 * radius + 1)^2 = 25` sample
  square centered on the tinted block.
- Radius `0` is the explicit "blend off" path.
- Every sample resolves `this.getBiome(samplePos)`, not just the chunk biome
  container cell at the rendered block.
- In the overworld, `getBiome(BlockPos)` goes through `BiomeManager` and
  `FuzzyOffsetConstantColumnBiomeZoomer`.
- `BiomeSpecialEffects.GrassColorModifier.SWAMP` uses
  `Biome.BIOME_INFO_NOISE.getValue(x * 0.0225, z * 0.0225, false)` and returns
  `5011004` (`#4c763c`) when the value is below `-0.1`, otherwise `6975545`
  (`#6a7039`).

## Current Native Shape

Current tinting is concentrated in:

- `native/crates/mclone-mesh/src/builder.rs`
- `native/crates/mclone-mesh/src/catalog.rs`
- `native/crates/mclone-mesh/src/tint.rs`
- `native/crates/mclone-worldgen/src/biome.rs`
- `native/crates/mclone-worldgen/src/surface.rs`

Current strengths:

- The asset colormap sampler uses the same temperature/downfall indexing shape
  as Java.
- `mclone-worldgen` already has the Java-shaped fuzzy block-position biome
  lookup in `OverworldBiomeSource::get_block_position_biome_definition_at_y`.
- `mclone-worldgen::surface` already has a `BIOME_INFO_NOISE` implementation
  for surface behavior.
- The recent metadata audit added regression coverage for water colors,
  colormap climate inputs, swamp/badlands overrides, birch/spruce constants,
  and the dark-forest modifier constant.
- `mclone-mesh::tint` now uses the Java swamp
  `Biome.BIOME_INFO_NOISE` permutation and has source-derived sample coverage.
- `mclone-mesh::tint` now averages grass, foliage, and water colors over the
  Java default 5x5 block window with `BIOME_BLEND_RADIUS = 2`.
- `mclone-mesh::builder` now resolves seeded tint biome reads through a local
  `FuzzyOffsetConstantColumnBiomeZoomer` translation when the mesh input carries
  `biome_zoom_seed`.
- `mclone-mesh::catalog` already maps the current rendered grass, fern,
  large-fern, potted-fern, sugar-cane, leaf, vine, and water consumers onto the
  appropriate tint families.

Current gaps:

- Radius `0` / configurable blend-radius plumbing remains a target; the current
  radius is a shared compile-time default.
- Seeded fuzzy lookup depends on mesh inputs carrying a world/zoom seed and
  enough neighboring biome data. Region-edge behavior still belongs with the
  broader render compile-region parity work.
- Tinting still repeats the 25-sample blend work per tinted query; there is no
  Java-like `BlockTintCache` or native compile-local equivalent yet.
- Tall grass / large fern upper-half behavior needs an audit before broadening
  plant tint parity, because Java samples the block below for upper halves.
- The remaining `BlockColors` surface still needs an explicit current-content
  audit so future rendered blocks do not silently choose the wrong tint path.
- Pixel/debug probes for swamp, dark forest, river/forest boundary, ocean
  boundary, and badlands captures are still missing.
- The tint code lives inside `mclone-mesh`, with a small local translation of
  `BIOME_INFO_NOISE`; keep this shared renderer-facing boundary unless a future
  crate split can reuse worldgen noise helpers without pulling full worldgen
  into renderer-only workers.

## Target Shape

Create a shared renderer-facing biome tint resolver that can answer:

- grass color at a block position,
- foliage color at a block position,
- water color at a block position,
- optional blend radius, defaulting to Java's `2`,
- direct radius-0 mode for diagnostics and parity toggles.

The resolver should use the same conceptual pipeline as Java:

1. resolve each sampled block position to a biome through the fuzzy
   block-position biome lookup,
2. apply the biome's grass/foliage/water color rules, including modifiers,
3. average RGB channels over the configured blend square,
4. feed the result into mesh vertex tinting.

The implementation does not need to duplicate Java's `BlockTintCache` class
shape exactly, but it needs the same output and similar locality:

- cache per-block tint results where mesh compilation would otherwise repeat
  25 samples for many faces,
- invalidate naturally with render-section rebuilds,
- keep old mesh output visible until replacement mesh output is ready, following
  the broader terrain render pipeline policy.

## Ownership And Boundary

Shared owner should be renderer-facing, not app-local:

- `mclone-mesh` should own applying tints to vertices.
- A small shared tint/biome-visual module should own vanilla color rules and
  blending. This may live in `mclone-mesh` if it only consumes biome ids from a
  supplied source, or in a new shared crate if `mclone-worldgen` noise helpers
  need to be reused cleanly.
- `mclone-worldgen` should continue to own vanilla biome source and
  block-position biome lookup behavior.
- App crates should only pass configuration such as blend radius or debug
  toggles. They should not own color policy.

Avoid a desktop-only fix. Desktop flat is the quickest validation lane, but the
mesh and tint resolver must remain shared across desktop, web/WASM, Android,
and XR render compilation.

## Implementation Slices

### Slice A: Swamp Modifier Parity

Status: landed for code/test coverage; visual recapture remains under Slice F.

`coarse_position_noise(...)` has been replaced in the mesh tint path with Java's
`Biome.BIOME_INFO_NOISE` behavior:

- `mclone-mesh::tint` carries a fixed
  `PerlinSimplexNoise(new WorldgenRandom(2345L), [0])` permutation for
  renderer-side use,
- it implements the exact `x * 0.0225`, `z * 0.0225`, threshold `-0.1` rule,
- `swamp_grass_modifier_matches_java_biome_info_noise` covers source-derived
  sample points including `(-51, 193)`.

The helper was kept local to `mclone-mesh` instead of exposing
`mclone-worldgen::surface::biome_info_noise()` because the renderer-facing tint
path only needs this fixed swamp-color lookup, not full surface generation.

### Slice B: Block-Position Biome Lookup For Tints

Status: landed for seeded mesh inputs; compile-region edge coverage remains
tracked with `127-render-compile-region-parity.md`.

Mesh tinting no longer has to use the local quart-cell biome id as the final
block tint biome:

- `TexturedChunkMeshInput::with_world_seed(...)` / `with_biome_zoom_seed(...)`
  passes the seed context into mesh compilation,
- `biome_id_at_world_or_default(...)` uses
  `fuzzy_offset_constant_column_biome_id(...)` when a zoom seed is available,
- tests cover reference quart cases and seeded lookup through neighboring chunk
  biome containers.

This still overlaps with `127-render-compile-region-parity.md`: biome/tint reads
are one more reason the compile region needs a named boundary instead of an
arbitrary snapshot vector.

### Slice C: Vanilla Blend Radius 2

Status: landed for the Java default radius; configurable radius and radius-0
diagnostics remain planned.

Java's default RGB channel average is now implemented for grass, foliage, and
water:

- default radius `2`,
- fixed square sample count `(2r + 1)^2`,
- integer channel averaging matching Java truncation,
- focused tests for the 5x5 block window and water blend averaging.

The seed `1124`, block `(-51, 76, 193)` swamp case remains a useful screenshot
regression because Java center direct `#4c763c` blends to `#54743b`.

### Slice D: Tint Cache And Compile Cost

Status: planned.

This is now the highest-value performance follow-up in this tactical after the
default blending path landed.

Avoid doing 25 full biome/color resolutions for every tinted face:

- cache block-position grass/foliage/water colors within one compile region,
- key by world block x/z and tint kind, with y included only where necessary,
- track compile-time cost before and after on desktop and one XR path,
- make the cache a mesh-compile detail, not a global mutable client cache at
  first.

Java has `BlockTintCache` because client-level tinting is queried frequently.
Native can start with per-compile caching because render-section rebuilds are
already the invalidation boundary.

### Slice E: Plant And Liquid Audit

Status: planned.

This is now the highest-value correctness follow-up in this tactical after the
default blending path landed. Start with tall grass and large fern upper-half
sampling because Java explicitly samples `pos.below()` for upper halves.

Audit all current block tint consumers against Java `BlockColors`:

- grass block,
- grass plant,
- fern,
- large fern and tall grass upper-half sampling,
- sugar cane,
- oak/jungle/acacia/dark-oak leaves,
- birch/spruce fixed foliage,
- vines if/when rendered,
- water, bubble columns, and water cauldron if/when rendered,
- lily pad if/when rendered.

Current native catalog coverage already includes grass block, grass, fern, large
fern, potted fern, sugar cane, oak/jungle/acacia/dark-oak leaves, vine,
birch/spruce fixed foliage, and water. The audit should therefore verify
position/state semantics and document deferred consumers, not blindly enable
everything listed above.

Only implement content that exists in the current native asset/block surface,
but document each deferred block so future content does not silently pick the
wrong tint path.

### Slice F: Debug And Validation Views

Status: planned.

Add a small diagnostic path for tint parity work:

- debug overlay line or capture metadata for active biome blend radius,
- optional capture mode for radius `0` versus default radius `2`,
- screenshot fixtures under `/tmp` for known swamp, dark forest, river/forest
  boundary, and ocean boundary positions,
- pixel/stat probes that count distinct grass/water tint colors in a small
  region without committing screenshots.

## Validation Plan

Use native desktop/headless first, but keep the work shared:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- targeted `mclone-worldgen` tests if the BIOME_INFO_NOISE helper moves or is
  exposed
- native full-frame screenshot under `/tmp` for seed `1124`, around
  `(-51, 76, 193)`
- a second screenshot on a true biome boundary to prove blend radius softens
  cross-biome seams
- web/WASM build smoke if resolver ownership crosses crates used by web
- Android/XR smoke only if the tint config or render compile payload changes
  platform-facing contracts

Pixel evidence to preserve in tests or doc updates:

- seed `1124`, block `(-51, 76, 193)`,
- vanilla block-position biome: `minecraft:swamp`,
- vanilla direct grass: `#4c763c`,
- vanilla default 5x5 blended grass: `#54743b`,
- nearby 25x25 direct vanilla swamp colors: `#6a7039` and `#4c763c`.

## Open Questions

- Should biome blend radius be a user-facing option now, or stay a hidden
  vanilla default plus debug CLI until the options UI is ready?
- Should tint resolution depend on the authoritative server seed/source at mesh
  compile time, or should chunk snapshots carry enough biome neighborhood data
  to avoid seed/source coupling in workers?
- Should `BIOME_INFO_NOISE` move to a small shared vanilla-noise helper crate,
  or remain in `mclone-worldgen` with a narrow public helper?
- Do far-LOD or future coarse terrain paths use vanilla tint blending, or a
  cheaper averaged biome color per coarse tile?

## Current Recommendation

Do this before adding more biome decoration variety. The visible problem is not
that swamps lack enough features; it is that existing vanilla swamp grass color
semantics are not yet rendered like Java.

Implement in this order:

1. exact swamp `BIOME_INFO_NOISE` modifier,
2. block-position biome lookup for tints,
3. vanilla blend radius `2` for grass/foliage/water,
4. per-compile tint cache,
5. broader `BlockColors` audit.

That order fixes the current screenshot first, then closes the general biome
blend gap without turning the renderer into a desktop-only special case.
