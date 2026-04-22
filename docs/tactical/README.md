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
| [`09a-renderer-browser-harness.md`](09a-renderer-browser-harness.md) | Vite dev server, `@playwright/test` against system Chrome (`channel: "chrome"`), WebGPU smoke test, `src/renderer/main.ts` entry point | infra | **done** — browser-test harness every renderer slice from `10` onward builds on |
| [`10-vertex-buffer-layer.md`](10-vertex-buffer-layer.md) | `BufferBuilder`, `VertexFormat`, `DefaultVertexFormat`, `VertexBuffer`, `PoseStack`, `Matrix4f`/`Matrix3f` | unit + smoke | CPU-side mesh building; first non-trivial draw call (solid quad) |
| `11-` | `RenderType`/`RenderStateShard` → `GPURenderPipelineDescriptor` mapping (blend, depth, cull, writeMask); bind group layouts; uniform buffer layout derived from MC shader JSON; stub WGSL to exercise pipeline creation end-to-end | unit + smoke | WebGPU pipeline infrastructure; stub shaders stand in until slice 16 |
| `12-` | `Stitcher`, `TextureAtlas`, `TextureAtlasSprite` (u0/v0/u1/v1), `NativeImage`, `MipmapGenerator`, asset loading from extracted jar | unit + oracle | atlas UVs match MC-dumped oracle byte-exact; GPU texture upload |
| `13-` | `ResourceLocation`, `Block`, `BlockState`, property system (`Property`, `BlockStateDefinition`), `BlockGetter` stub | unit | prereqs model baking needs; deferred from `09` since vertex layer and pipeline don't need them |
| `14a-` | `BlockModel` JSON parse → `UnbakedModel`; `Element`, `Face`, `FaceUV`; parent model resolution; texture variable resolution | unit | unbaked model data structures, no baking machinery yet |
| `14b-` | `ModelBakery` baking pass, `SimpleBakedModel`, `BakedQuad` int\[\] layout, `BlockModelShaper` | unit + oracle | baked-model quads match MC-dumped oracle per blockstate — first correctness gate on model output |
| `15-` | `ModelBlockRenderer` (`tesselateWithAO`, `tesselateWithoutAO`), `FaceInfo`, `BlockRenderDispatcher`, `LiquidBlockRenderer` | golden (via `09a`) | single chunk renders textured with AO; first golden screenshot |
| `16-` | WGSL ports of `assets/minecraft/shaders/core/` (rendertype\_solid, rendertype\_cutout, rendertype\_translucent, rendertype\_lines); wire into pipeline cache from `11` | golden (via `09a`) | proper MC shaders replace stub WGSL; textured lit blocks |
| `17-` | `RenderChunkRegion`, `ChunkBufferBuilderPack`, `VisGraph`/`VisibilitySet`, `ViewArea`, `ChunkRenderDispatcher` (async compilation) | golden (via `09a`) | section-level occlusion culling; many chunks at interactive rates |
| `18-` | `LevelRenderer`, `GameRenderer`, `Frustum`, `LightTexture`, `FogRenderer` | golden (via `09a`) | **MVP renderer reached here** — fly around a generated world |

Ordering rationale: `BlockState` prereqs (13) deferred until just before model baking (14a/b) since nothing before that needs them. WGSL shader ports (16) deferred until after the mesher (15) so we can verify geometry correctness with stub shaders first, then swap in real shaders. `ModelBakery` split from `BlockModel` parse (14a/b) because `ModelBakery` is one of the largest classes in the client and the data-loading and baking passes are independently testable.

## Renderer MVP definition

"Fly around a vanilla-parity overworld" — textured blocks, day lighting, face culling, AO, chunk-level visibility culling, fog. No entities, no GUI, no particles, no item rendering, no post-processing.

## Renderer oracle approach

Pixel-exact diffs don't work (driver float quirks, font differences, gamma). Strategy:

- **Unit**: dump atlas UVs, baked-model quads, and section visibility graphs from MC as JSON; exact-diff those. Catches most correctness bugs before pixels are involved.
- **Smoke / golden screenshots**: run the browser harness from [`09a-renderer-browser-harness.md`](09a-renderer-browser-harness.md) — system Chrome via Playwright `channel: "chrome"` against a Vite-served page. Smoke (clear-color pixel check) from slice `10`; SSIM-compared golden screenshots against vanilla MC at a pinned seed + fixed camera pose from slice `14` onward, with a generous threshold. One or two canonical poses per slice once meshing lands.

## Skipped entirely (for renderer MVP)

`entity/`, `blockentity/`, `item/`, `PostChain` post-processing, particles, GUI, `CubeMap`/`PanoramaRenderer`, sounds (`com.mojang.blaze3d.audio`), `OutlineBufferSource`, `DimensionSpecialEffects` beyond overworld defaults.
