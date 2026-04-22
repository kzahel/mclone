# Tactical docs

Numbered, short-lived implementation plans. Each covers a cohesive group of modules scoped to ~1–2 focused sessions of work. Strategy lives in [`../strategy.md`](../strategy.md); these are the sequenced "do this next" plans.

## Rule of thumb

- **Plan only the next one in detail.** What we learn porting module N changes what N+1 should look like.
- The arc below is *aspirational orientation*, not a commitment. Skip, merge, or reorder as reality dictates.
- Each tactical doc owns: scope (modules), unit-oracle inputs → expected outputs, "done when" criteria, and a pointer to the next.

## Arc (aspirational, toward MVP terrain gen)

| Doc | Modules | Oracle tier | Purpose |
|---|---|---|---|
| [`00-worldgen-ts-port.md`](00-worldgen-ts-port.md) | scaffold, `SimpleRandomSource`, `WorldgenRandom`, `ImprovedNoise` | unit | foundation numerics |
| [`01-noise-octaves.md`](01-noise-octaves.md) | `PerlinNoise`, `SimplexNoise`, `BlendedNoise` | unit | everything `NoiseSampler` consumes from `synth/` |
| [`02-remaining-synth.md`](02-remaining-synth.md) | `SurfaceNoise`, `PerlinSimplexNoise`, minimal `NormalNoise` | unit | remaining `synth/` pieces alive in default 1.17.1; `NoiseUtils` + C&C Part 1 consumers deferred (see [`../../AGENTS.md`](../../AGENTS.md)) |
| [`03-noise-sampler-settings.md`](03-noise-sampler-settings.md) | `NoiseSampler` + settings (`NoiseSettings`, `NoiseSamplingSettings`) | unit | 3D density field from chunk coords |
| [`04-integration-oracle-harness.md`](04-integration-oracle-harness.md) | **Integration oracle harness**: server-jar runner, MCA reader, chunk-level fixture format | infra | stand up the server-oracle *before* we need to diff chunks |
| [`05-overworld-biome-source.md`](05-overworld-biome-source.md) | `BiomeSource` subset — translated `OverworldBiomeSource` + layered climate pipeline → biome IDs | unit + integration | chunk biomes + `NoiseSampler` inputs now come from a real overworld biome source |
| [`06-noise-based-chunk-generator.md`](06-noise-based-chunk-generator.md) | `NoiseBasedChunkGenerator` (terrain-only: stone/air/water/bedrock; no surface/caves/features) | **integration** | first end-to-end chunk diff against a committed terrain-only Java oracle fixture for the pinned chunk |
| [`06a-pre-07-surface-prep.md`](06a-pre-07-surface-prep.md) | chunk buffer extraction, surface-stage oracle, explicit `buildSurfaceAndBedrock` staging | prep + integration | clear the staging/oracle/storage friction before porting surface builders |
| [`07-surface-builders.md`](07-surface-builders.md) | `SurfaceBuilder` dispatch and overworld surface materials | integration | visual recognizability |
| `08-` | Carvers: `CaveWorldCarver`, `CanyonWorldCarver` | integration | **MVP terrain gen reached here** |

Everything after 08 (features, ores, structures, aquifers, noodle caves) is post-MVP *worldgen* polish.

## MVP definition

"Recognizable Minecraft overworld" — terrain + carvers + surface rules, no features, no structures, no ores. Rendering a chunk should make you say "oh yeah, Minecraft."

## Renderer arc (aspirational, toward MVP overworld renderer)

Direct port of MC 1.17.1's client rendering stack to raw WebGPU. Target and skip-list for 1.17.1 vanilla inherit from [`../../AGENTS.md`](../../AGENTS.md). Two things do *not* transliterate from the Java source and get re-expressed in WebGPU idioms: `com.mojang.blaze3d.platform.GlStateManager` + `Uniform` (WebGPU bakes blend/depth/cull state and uniform layouts into precompiled pipelines and bind groups), and the vanilla GLSL core shaders (rewritten as WGSL). Everything else — vertex formats, buffer builders, atlas stitching, model baking, chunk meshing, visibility graphs, `LevelRenderer` orchestration — is direct translation.

| Doc | Modules | Oracle tier | Purpose |
|---|---|---|---|
| `09-` | `BlockState` + property system, `ResourceLocation`, `BlockGetter`/`Level` view over our chunks, vanilla-jar asset extraction | unit | prereqs renderer needs that worldgen didn't |
| `10-` | `com.mojang.blaze3d` vertex layer (`BufferBuilder`, `VertexFormat`, `DefaultVertexFormat`, `VertexBuffer`, `PoseStack`) + WebGPU bootstrap (canvas, device, swapchain, clear frame) | unit + smoke | CPU-side mesh building + a canvas that clears |
| `11-` | Pipeline cache, `RenderType`/`RenderStateShard` → `GPURenderPipeline`, uniform buffers + bind group layouts | unit | WebGPU re-expression of MC's state system (replaces `GlStateManager` + `Uniform`) |
| `12-` | `Stitcher` → `TextureAtlas` → `TextureAtlasSprite` → `MipmapGenerator` → `TextureManager`; asset-loading plumbing | unit + golden | atlas UVs match an MC-dumped oracle byte-exact |
| `13-` | `BlockModel` JSON parse → `UnbakedModel` → `ModelBakery` → `BakedModel` variants → `BlockModelShaper` | unit | baked-model quads match an MC-dumped oracle per blockstate |
| `14-` | `ModelBlockRenderer`, `LiquidBlockRenderer`, `BlockRenderDispatcher`, `FaceInfo`, `ItemBlockRenderTypes`; chunk mesher with face culling + AO | golden | single chunk renders and matches a reference screenshot |
| `15-` | `RenderChunkRegion`, `ChunkBufferBuilderPack`, `VisGraph`/`VisibilitySet`, `ViewArea`, `ChunkRenderDispatcher` | golden | section-level occlusion culling; many chunks at interactive rates |
| `16-` | `LevelRenderer`, `GameRenderer`, `Frustum`, `LightTexture`, `FogRenderer`; WGSL port of `assets/minecraft/shaders/core/*` | golden | **MVP renderer reached here** — fly around a generated world |

Slices 13 and 14 may split once we get there — `ModelBakery` and `ModelBlockRenderer` are both large. Numbering is fluid per the rule of thumb.

## Renderer MVP definition

"Fly around a vanilla-parity overworld" — textured blocks, day lighting, face culling, AO, chunk-level visibility culling, fog. No entities, no GUI, no particles, no item rendering, no post-processing.

## Renderer oracle approach

Pixel-exact diffs don't work (driver float quirks, font differences, gamma). Strategy:

- **Unit**: dump atlas UVs, baked-model quads, and section visibility graphs from MC as JSON; exact-diff those. Catches most correctness bugs before pixels are involved.
- **Golden screenshots**: run vanilla MC at a pinned seed + fixed camera pose, compare our rendering via structural similarity (SSIM) with a generous threshold. One or two canonical poses per slice once meshing lands.

## Skipped entirely (for renderer MVP)

`entity/`, `blockentity/`, `item/`, `PostChain` post-processing, particles, GUI, `CubeMap`/`PanoramaRenderer`, sounds (`com.mojang.blaze3d.audio`), `OutlineBufferSource`, `DimensionSpecialEffects` beyond overworld defaults.
