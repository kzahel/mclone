# Tactical docs

Numbered, short-lived implementation plans. Each covers a cohesive group of modules scoped to ~1–2 focused sessions of work. Strategy lives in [`../strategy.md`](../strategy.md); these are the sequenced "do this next" plans. For the non-tactical view of what is actually landed, what is still missing, and how worldgen should be prioritized, see [`../worldgen-status.md`](../worldgen-status.md). For the runtime/host boundaries these tacticals are expected to converge toward, see [`../architecture.md`](../architecture.md).

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
| [`10-vertex-buffer-layer.md`](10-vertex-buffer-layer.md) | `BufferBuilder`, `VertexFormat`, `DefaultVertexFormat`, `VertexBuffer`, `PoseStack`, `Matrix4f`/`Matrix3f` | unit + smoke | **done** — CPU-side mesh building landed; first non-trivial draw call (solid quad) passes in Chrome |
| [`11-render-pipeline-infrastructure.md`](11-render-pipeline-infrastructure.md) | `RenderType`/`RenderStateShard` → `GPURenderPipelineDescriptor` mapping (blend, depth, cull, writeMask); bind group layouts; uniform buffer layout derived from MC shader JSON; stub WGSL to exercise pipeline creation end-to-end | unit + smoke | **done** — WebGPU pipeline cache and shader-json-derived bind groups now drive the smoke quad |
| [`12-texture-atlas-plumbing.md`](12-texture-atlas-plumbing.md) | `Stitcher`, `TextureAtlas`, `TextureAtlasSprite` (u0/v0/u1/v1), `NativeImage`, `MipmapGenerator`, asset loading from extracted jar | unit + smoke | **done** — extracted block textures now stitch into a WebGPU atlas and drive the browser smoke |
| [`13-block-state-groundwork.md`](13-block-state-groundwork.md) | `Block`, `BlockState`, property system (`Property`, `BlockStateDefinition`), `BlockGetter` stub | unit | **done** — translated state/property scaffolding is in place for model parsing and baking |
| [`14a-block-model-unbaked-graph.md`](14a-block-model-unbaked-graph.md) | `BlockModel` JSON parse → `UnbakedModel`; `Element`, `Face`, `FaceUV`; parent model resolution; texture variable resolution | unit | **done** — extracted model JSON now parses into resolved unbaked model graphs with inherited textures and transforms |
| [`14b-block-model-baking.md`](14b-block-model-baking.md) | `ModelBakery` baking pass, `SimpleBakedModel`, `BakedQuad` int\[\] layout, `BlockModelShaper` | unit + oracle | **done** — resolved block models now bake into vanilla-shaped quad arrays and state-keyed shaper lookups |
| [`14c-blockstate-variant-and-multipart.md`](14c-blockstate-variant-and-multipart.md) | `BlockModelDefinition`, variant JSON, multipart selectors, `ModelBakery.loadModel(...)` blockstate path | unit + oracle | **done** — registered block states now resolve through vanilla blockstate JSON into weighted and multipart baked models |
| [`15-block-tesselation-path.md`](15-block-tesselation-path.md) | `ModelBlockRenderer`, `BlockRenderDispatcher`, minimal block-light/level plumbing, browser baked-block smoke | unit + visual (via `09a`) | **done** — baked block models now tesselate through `BufferBuilder` and render as real block pixels in Chrome |
| [`16-core-wgsl-shaders.md`](16-core-wgsl-shaders.md) | WGSL ports of `assets/minecraft/shaders/core/` (`position_color`, `position_tex`, `rendertype_solid`, `cutout`, `cutout_mipped`, `translucent`, `translucent_no_crumbling`, `translucent_moving_block`, `tripwire`, `lines`); wire into pipeline cache from `11` | visual (via `09a`) | **done** — real MC shader semantics replaced the stub WGSL path and Chrome now validates the whole tactical-16 shader set |
| [`17-chunk-section-compilation.md`](17-chunk-section-compilation.md) | `RenderChunkRegion`, `ChunkBufferBuilderPack`, `VisGraph`/`VisibilitySet`, `ViewArea`, `ChunkRenderDispatcher` (async compilation) | visual (via `09a`) | **done** — section rebuilds now compile through the translated chunk pipeline and the browser smoke draws compiled chunks instead of a handcrafted mesh |
| [`18-camera-driven-world-frame.md`](18-camera-driven-world-frame.md) | `LevelRenderer`, `GameRenderer`, `Frustum`, `LightTexture`, `FogRenderer` | visual (via `09a`) | **done** — camera-owned frame assembly now drives frustum culling, fog, and lightmap binding for compiled chunk layers |
| [`19-generated-world-chunk-bridge.md`](19-generated-world-chunk-bridge.md) | generator-backed `StaticRenderLevel` replacement, moving chunk cache, grass-block state fix, browser generated-world smoke | visual (via `09a`) | **done** — the renderer now compiles and draws real generated overworld chunks instead of the static smoke scene |
| [`20-surface-special-blocks-and-biome-tint.md`](20-surface-special-blocks-and-biome-tint.md) | `BlockColors`, biome tint metadata, `LiquidBlockRenderer`, `LiquidBlock`, `SnowLayerBlock`, fluid chunk-layer submission | visual (via `09a`) | **done** — generated terrain now keeps biome tint and the first water/snow surface-special paths instead of collapsing them out of the frame |
| [`21-surface-feature-palette-expansion.md`](21-surface-feature-palette-expansion.md) | `BushBlock`, `LeavesBlock`, `CactusBlock`, `SugarCaneBlock`, feature-block tint/render-layer wiring, expanded generated block palette | visual (via `09a`) | **done** — generated terrain smoke frames now render the first tree/plant/simple-feature palette instead of only terrain blocks |
| [`22-simple-feature-placement-bridge.md`](22-simple-feature-placement-bridge.md) | `Feature`, `ConfiguredFeature`, `DecoratedFeature`, simple decorators, `RandomPatchFeature`, `SimpleBlockFeature`, smoke-scene feature placement | visual (via `09a`) | **done** — smoke-scene plants and simple columns now come through translated 1.17.1 feature placement instead of direct harness block writes |
| [`23-true-tree-feature-placement.md`](23-true-tree-feature-placement.md) | `TreeFeature`, `TreeConfiguration`, `StraightTrunkPlacer`, `BlobFoliagePlacer`, `TwoLayersFeatureSize`, leaf post-processing, smoke-scene oak placement | visual (via `09a`) | **done** — the smoke scene’s remaining oak canopy now comes from translated 1.17.1 tree placement instead of handwritten block blobs |
| [`24-biome-vegetation-decoration-bridge.md`](24-biome-vegetation-decoration-bridge.md) | `BiomeGenerationSettings`, chunk `applyBiomeDecoration(...)`, first selector/decorator helpers, mountain/taiga vegetation tables, spruce/pine/fancy-oak tree configs | visual (via `09a`) | **done** — generated chunks now own the first biome-driven tree-and-grass decoration pass instead of depending on smoke-harness vegetation writes |
| [`25-biome-decoration-palette-expansion.md`](25-biome-decoration-palette-expansion.md) | `DoublePlantBlock`, berry/mushroom/pumpkin/sugar-cane/cactus feature tables, `DoublePlantPlacer`, `ColumnPlacer`, `ChanceDecorator`, generated palette expansion, natural shoreline smoke framing | visual (via `09a`) | **done** — the translated biome-decoration path now covers the first double-plant and column vegetation set, and the browser smoke no longer injects a manual water patch |
| [`26-overworld-water-and-swamp-decoration.md`](26-overworld-water-and-swamp-decoration.md) | `LakeFeature`, `SpringFeature`, range/count-noise decorators, `WaterlilyBlock`, swamp surface mutation, first forest/plains/swamp biome tables, swamp smoke framing | visual (via `09a`) | **done** — generated chunks now own the first real overworld water-feature path, swamp lily pads, and the first forest/plains/swamp decoration tables |
| [`27-biome-decoration-parity-follow-through.md`](27-biome-decoration-parity-follow-through.md) | flower providers, `Feature.FLOWER`, `SeagrassFeature`, dead-bush/seagrass blocks, birch/flower-forest/swamp biome-table follow-through, expanded plant palette | visual (via `09a`) | **done** — plains / forests / birch forests / swamps now use translated flower, dead-bush, and seagrass decoration paths instead of the old reduced fallback tables |

Ordering rationale: `BlockState` prereqs (13) deferred until just before model baking (14a/b) since nothing before that needs them. WGSL shader ports (16) deferred until after the mesher (15) so we can verify geometry correctness with stub shaders first, then swap in real shaders. `ModelBakery` split from `BlockModel` parse (14a/b) because `ModelBakery` is one of the largest classes in the client and the data-loading and baking passes are independently testable.

## Renderer MVP definition

"Fly around a vanilla-parity overworld" — textured blocks, day lighting, face culling, AO, chunk-level visibility culling, fog. No entities, no GUI, no particles, no item rendering, no post-processing.

## Renderer oracle approach

- **Unit**: dump atlas UVs, baked-model quads, and section visibility graphs from MC as JSON; exact-diff those. Catches most correctness bugs before pixels are involved.
- **Visual inspection**: run the browser harness from [`09a-renderer-browser-harness.md`](09a-renderer-browser-harness.md) — system Chrome via Playwright `channel: "chrome"` against a Vite-served page. Take a screenshot and look at it. Does the geometry look right? Are colors and UVs sensible? This is a human (or agent) eyeball check, not an automated diff.

## Runtime / host arc (rough, cross-cutting)

This arc is intentionally separate from the numbered worldgen and renderer translation arcs above. It is about correcting the current execution model so the engine can support browser singleplayer without main-thread stalls, browser multiplayer clients, and a headless Node host.

Because generated chunks and chunk rebuild work still run too close to the render path today, this arc should generally take precedence over additional parity slices until:

- the renderer no longer owns chunk generation
- browser singleplayer runs behind an authoritative local host boundary
- chunk meshing no longer stalls the main thread

WebRTC is intentionally deferred. The preferred path is:

1. shared protocol shapes
2. local worker transport
3. dedicated Node host
4. browser clients talking to that host
5. optional WebRTC transport later if it still buys something

| Doc | Modules | Validation tier | Purpose |
|---|---|---|---|
| [`R0-authoritative-world-boundary.md`](R0-authoritative-world-boundary.md) | shared client/server contracts: `WorldHost`, `WorldClient`, chunk subscription/view commands, `ChunkSnapshot` / unload / delta message shapes, local transport abstraction | unit + browser smoke | **done** — the browser smoke now runs through a local authoritative host/client boundary and a read-only client chunk cache instead of calling worldgen directly |
| `R1-` | browser singleplayer local-server worker: worker bootstrap, local transport adapter, worker-owned chunk service, render-path conversion to a read-only client chunk cache | browser smoke + perf probe | **done** — browser singleplayer now runs the authoritative generated-world host behind a module worker and reuses the same client/cache protocol shape from `R0` |
| `R2-` | client mesh worker pipeline: section-meshing jobs, worker-facing mesh input/output records, render-thread upload handoff, chunk rebuild scheduling cleanup | browser smoke + perf probe | remove the remaining main-thread stalls caused by chunk rebuild/meshing work |
| `R3-` | persistence interfaces plus browser adapter: `WorldStorage`, `ChunkStorage`, save metadata, IndexedDB-backed implementation, load/evict policy hooks | unit + browser smoke | make browser singleplayer durable without baking IndexedDB assumptions into simulation code |
| `R4-` | headless Node host: authoritative server runtime bootstrap, file-backed storage adapter, CLI/config entry point, local integration harness | integration | prove the same server/runtime core works outside the browser |
| `R5-` | remote browser-client transport to dedicated host: network transport adapter, connection/session bootstrap, browser client consuming remote chunk/state stream, two-client local smoke | integration + 2-client smoke | reach the first practical multiplayer shape with the simpler dedicated-host path |
| `R6-` | protocol hardening: reconnect/resync, chunk interest management, baseline player/session state sync, error handling and versioning discipline | integration | make the host/client boundary robust enough that future gameplay systems can accumulate on top of it |
| `R7-` | optional browser-hosted peer/server transport: WebRTC/WebTransport-style adapter reusing the same protocol and host boundary | integration | slot in a browser-hosted server mode later without redesigning the engine around it up front |

The first tactical to plan in detail from this arc should be `R0-`, not `R7-`. If `R0-` and `R1-` are not real, every later runtime mode becomes a special case.

## Skipped entirely (for renderer MVP)

`entity/`, `blockentity/`, `item/`, `PostChain` post-processing, particles, GUI, `CubeMap`/`PanoramaRenderer`, sounds (`com.mojang.blaze3d.audio`), `OutlineBufferSource`, `DimensionSpecialEffects` beyond overworld defaults.
