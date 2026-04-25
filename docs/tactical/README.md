# Tactical docs

Numbered, short-lived implementation plans. Each covers a cohesive group of modules scoped to ~1–2 focused sessions of work. Strategy lives in [`../strategy.md`](../strategy.md); these are the sequenced "do this next" plans. For the non-tactical view of what is actually landed, what is still missing, and how worldgen should be prioritized, see [`../worldgen-status.md`](../worldgen-status.md). For vanilla chunk-status order, deterministic decoration finality, lighting gates, and publication gates, see [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md). For the narrower live status of classic overworld carvers, see [`../carver-status.md`](../carver-status.md). For vanilla overworld structure generation architecture and suggested implementation order, see [`../structures.md`](../structures.md). For runtime/host boundaries and durable data/protocol/loading contracts, see [`../architecture.md`](../architecture.md), [`../runtime-data-model.md`](../runtime-data-model.md), [`../protocol.md`](../protocol.md), and [`../loading-persistence.md`](../loading-persistence.md). For liquid simulation architecture, see [`../liquids.md`](../liquids.md).

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
| `08-` | Carvers: `CaveWorldCarver`, `CanyonWorldCarver` | integration | historical placeholder only — the current carver follow-through lives in [`28-carver-material-parity-and-oracle-expansion.md`](28-carver-material-parity-and-oracle-expansion.md), [`29-underwater-liquid-carver-parity.md`](29-underwater-liquid-carver-parity.md), [`30-liquid-floor-oracle-and-tick-capture.md`](30-liquid-floor-oracle-and-tick-capture.md), [`31-frozen-and-badlands-material-matrix.md`](31-frozen-and-badlands-material-matrix.md), and [`32-podzol-coarse-dirt-and-mycelium-matrix.md`](32-podzol-coarse-dirt-and-mycelium-matrix.md) |

Everything after 08 (features, ores, structures, aquifers, noodle caves) is post-MVP *worldgen* polish. Any chunk-generation tactical that touches scheduling, decoration spillover, lighting gates, send gates, or render-readiness should preserve [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md). Structure implementation should follow the status-aware starts/references/placement model in [`../structures.md`](../structures.md), not a one-off decoration shortcut. The original `08-` row above was never written as a standalone doc; the real current carver parity follow-through is [`28-carver-material-parity-and-oracle-expansion.md`](28-carver-material-parity-and-oracle-expansion.md), [`29-underwater-liquid-carver-parity.md`](29-underwater-liquid-carver-parity.md), [`30-liquid-floor-oracle-and-tick-capture.md`](30-liquid-floor-oracle-and-tick-capture.md), [`31-frozen-and-badlands-material-matrix.md`](31-frozen-and-badlands-material-matrix.md), and [`32-podzol-coarse-dirt-and-mycelium-matrix.md`](32-podzol-coarse-dirt-and-mycelium-matrix.md).

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
| [`28-carver-material-parity-and-oracle-expansion.md`](28-carver-material-parity-and-oracle-expansion.md) | widen the carved/surface numeric model for carver-relevant materials, align `WorldCarver` replaceable-material parity for the live overworld AIR-step path, and broaden carved oracle coverage with a third land fixture | unit + integration | **done** — the widened material model, sandstone surface follow-through, explicit replaceable-material tests, and the third desert carved fixture are now committed |
| [`29-underwater-liquid-carver-parity.md`](29-underwater-liquid-carver-parity.md) | port `UnderwaterCaveWorldCarver` / `UnderwaterCanyonWorldCarver`, wire ocean `GenerationStep.Carving.LIQUID`, widen the numeric model with underwater-floor outputs, and add a dedicated AIR-plus-LIQUID ocean oracle | unit + integration + browser smoke | **done** — LIQUID carvers are now integrated, the new staged oracle path is committed, and the browser smoke still renders the generated world through the remote host path |
| [`30-liquid-floor-oracle-and-tick-capture.md`](30-liquid-floor-oracle-and-tick-capture.md) | record scheduled underwater water/magma tick consequences on generated chunks, carry them through snapshots/oracles, add a floor-branch LIQUID fixture, and replace the generic smoke follow-through with a ravine-focused browser frame | unit + integration + browser visual | **done** — carved fixtures now pin scheduled tick consequences, the LIQUID floor branch has a committed oracle, and browser validation uses a targeted ravine frame instead of generic terrain smoke |
| [`31-frozen-and-badlands-material-matrix.md`](31-frozen-and-badlands-material-matrix.md) | port the missing frozen-ocean and badlands surface follow-through, widen the shared runtime/oracle/render material matrix for those families, add committed frozen/badlands fixtures, and validate the new surfaces in the browser | unit + integration + browser visual | **done** — frozen-ocean and badlands surfaces now match committed Java fixtures, the widened render/runtime palette stays stable through generation, and browser validation captures the new surface families directly |
| [`32-podzol-coarse-dirt-and-mycelium-matrix.md`](32-podzol-coarse-dirt-and-mycelium-matrix.md) | port the remaining giant-tree-taiga / shattered-savanna / mushroom surface follow-through, restore the matching `LakeFeature` mycelium/ice behavior, add committed oracle chunks for those families, and validate them in the browser | unit + integration + browser visual | **done** — the remaining live podzol/coarse-dirt/mycelium surface families now match committed Java fixtures, `LakeFeature` restores mushroom-field ceilings correctly, and browser validation captures all three surfaces directly |
| [`33-dark-forest-parity.md`](33-dark-forest-parity.md) | port the dark-oak tree path, the first huge-mushroom feature path, the dark-oak placement decorator, and the dark-forest / dark-forest-hills biome tables | unit + browser visual | **done** — dark forest and dark-forest hills now route through translated dark-oak and huge-mushroom vegetation instead of the old reduced fallback |
| [`34-savanna-parity.md`](34-savanna-parity.md) | port the acacia tree path, the savanna/shattered-savanna vegetation selectors, and the savanna biome tables | unit + browser visual | **done** — savanna and shattered-savanna families now route through translated acacia vegetation instead of the old generic forest fallback |
| [`35-jungle-parity.md`](35-jungle-parity.md) | port the jungle tree/decorator ecosystem, the first standalone vine/melon follow-through, and the jungle / jungle-hills / jungle-edge biome tables | unit + browser visual | **done** — jungle core biomes now route through translated jungle vegetation, vine, cocoa, and melon paths instead of the old generic forest fallback |
| [`36-snowy-giant-taiga-and-mushroom-table-coverage.md`](36-snowy-giant-taiga-and-mushroom-table-coverage.md) | port the snowy-tree / giant-conifer / mushroom-field selector path, add mega spruce / mega pine podzol follow-through, and wire the snowy, giant-taiga, and mushroom-field biome tables | unit + browser visual | **done** — snowy, giant-taiga, and mushroom-field families now route through translated spruce / mega-conifer / huge-mushroom vegetation instead of reduced fallback tables |
| [`37-shoreline-and-transition-parity.md`](37-shoreline-and-transition-parity.md) | port shoreline/river/ocean seagrass+kelp follow-through, add the missing `COUNT_NOISE_BIASED` / kelp path, and wire beach / river / non-warm ocean biome tables | unit + browser visual | **done** — beaches, rivers, and the first ocean-table families now route through translated shoreline vegetation instead of carver-only fallback settings |
| [`38-cold-surface-parity.md`](38-cold-surface-parity.md) | port `FREEZE_TOP_LAYER`, `ICE_SPIKE`, and `ICE_PATCH`, add the disk-feature support they depend on, and wire the remaining cold shoreline / `ice_spikes` follow-through | unit + browser visual | **done** — the missing cold-surface feature path is now translated, `ice_spikes` has a real biome table and surface definition, and browser validation captures a dedicated packed-ice-spire frame |
| [`39-warm-ocean-parity.md`](39-warm-ocean-parity.md) | port coral features, `SeaPickleFeature`, the warm-ocean coral block family, and the `warm_ocean` / `deep_warm_ocean` biome tables | unit + browser visual | **done** — warm-ocean families now route through translated coral / sea-pickle vegetation instead of the old reduced ocean fallback |
| [`41-bamboo-jungle-parity.md`](41-bamboo-jungle-parity.md) | port the bamboo block / feature path, restore light-bamboo jungle follow-through, and wire the `bamboo_jungle` / `bamboo_jungle_hills` biome tables | unit + browser visual | **done** — bamboo-jungle families now route through translated bamboo vegetation instead of the last reduced jungle-family fallback |
| [`42-ore-and-underground-decoration-foundation.md`](42-ore-and-underground-decoration-foundation.md) | port the `OreFeature` / `OreConfiguration` / `RuleTest` foundation, add common overworld ores plus the active underground-variety material blobs, and wire vanilla underground defaults into the current biome tables | unit + integration + browser visual | **done** — common overworld ore generation and the active underground-variety material path now run through translated features, biome-table wiring, and browser validation |
| [`43-biome-specific-underground-extras.md`](43-biome-specific-underground-extras.md) | port badlands extra gold, mountain emeralds, mountain infested stone, and the minimal `ReplaceBlockFeature` path they depend on | unit + browser visual | **done** — the live badlands/mountain underground extras now run through translated configured features and browser validation |
| [`44-underground-tail-and-soft-disks.md`](44-underground-tail-and-soft-disks.md) | port glow lichen, rare dripstone clusters, rare small dripstone, soft disks, and the minimal block/palette support those underground helpers need | unit + browser visual | **done** — the remaining live vanilla overworld underground-helper tail now runs through translated configured features, biome tables, block palette wiring, and browser validation |
| [`45-overworld-biome-table-aliases-and-exactness.md`](45-overworld-biome-table-aliases-and-exactness.md) | fill the last layered-overworld biome-table aliases, add `TREES_BADLANDS`, and correct the nearby mountain / badlands / modified-jungle helper exactness gaps | unit + browser visual | **done** — the layered-overworld biome key set no longer falls back to carver-only settings, and the nearby mountain / badlands / modified-jungle helper mismatches are corrected |
| [`46-full-decorated-spawn-chunk-parity.md`](46-full-decorated-spawn-chunk-parity.md) | reusable full-decorated chunk diffing, seed `12345` chunk `(0, 0)` mismatch burn-down, and exact full-block server-fixture parity | integration + runtime | **done** - seed `12345`, chunk `(0,0)` now matches the scheduler-pinned fixture at `65,536 / 65,536` blocks after the fixture-equivalent generated-liquid tick window |
| [`47-generated-chunk-status-orchestration.md`](47-generated-chunk-status-orchestration.md) | explicit generated chunk status model, `FEATURES` stability, initial-light gate, and publishability gate | runtime + lighting | **done** — generated chunks now advance through explicit statuses before lighting and publication |
| [`48-vanilla-scheduler-trace-oracle.md`](48-vanilla-scheduler-trace-oracle.md) | executable vanilla 1.17.1 scheduler trace for status task order and neighboring `FEATURES` commit order | oracle + runtime diagnostics | **done** - the bounded spawn-bootstrap fixture records z-major/x-major target-3x3 `FEATURES` completion order and supports block probes |
| [`49-vanilla-status-futures-and-partial-chunks.md`](49-vanilla-status-futures-and-partial-chunks.md) | vanilla-shaped status futures, mixed-status dependencies, and partial `ProtoChunk`-like records instead of the flattened authority terrain window | runtime + perf | **proposed** - still the right status-orchestration cleanup, but no longer blocking the exact spawn fixture |
| [`50-beach-river-full-decorated-parity.md`](50-beach-river-full-decorated-parity.md) | scheduler-pinned full-decorated fixture for seed `12345`, chunk `(5,115)`, focused on legitimate sand/gravel boundary parity | integration + runtime | **next parity** - expand the full-block diff harness from the exact spawn fixture to the existing sand/gravel surface-oracle target |

Ordering rationale: `BlockState` prereqs (13) deferred until just before model baking (14a/b) since nothing before that needs them. WGSL shader ports (16) deferred until after the mesher (15) so we can verify geometry correctness with stub shaders first, then swap in real shaders. `ModelBakery` split from `BlockModel` parse (14a/b) because `ModelBakery` is one of the largest classes in the client and the data-loading and baking passes are independently testable.

## Renderer MVP definition

"Fly around a vanilla-parity overworld" — textured blocks, day lighting, face culling, AO, chunk-level visibility culling, fog. No entities, no GUI, no particles, no item rendering, no post-processing.

## Lighting Arc

Lighting is both simulation data and renderer input. Use [`../lighting.md`](../lighting.md) as the durable reference before writing tactical slices. The intended split is a direct 1.17.1 port of the light data/solver in the authoritative host, with engine-native worker scheduling and packed light facts flowing through snapshots/deltas into render-world meshing. Do not put propagation on the browser main thread, and do not make mesh payloads the source of truth for light.

| Doc | Modules | Validation tier | Purpose |
|---|---|---|---|
| [`L0-lighting-oracle-foundation.md`](L0-lighting-oracle-foundation.md) | Anvil `BlockLight` / `SkyLight` decode, light fixture shape, byte-exact comparison helpers, first committed server light fixture | unit + server oracle | **done** — persisted vanilla light bytes now ride in the pinned integration fixture and comparison helpers are ready for solver tests |
| [`L1-light-data-foundation.md`](L1-light-data-foundation.md) | `DataLayer`, `LightLayer`, light-section padding helpers, packed snapshot/wire light payloads | unit + L0 fixture roundtrip | **done** — runtime snapshots can now carry vanilla sky/block light bytes without interpreting propagation yet |
| [`L2-light-solver-foundation.md`](L2-light-solver-foundation.md) | `DynamicGraphMinFixedPoint`, section storage, block/sky engines, `LevelLightEngine`, `LightChunkGetter` adapter | unit | **done** — synthetic levels now compute stored block and sky light before host snapshot wiring |
| [`L3-initial-chunk-lighting.md`](L3-initial-chunk-lighting.md) | generated-world host light initialization, section activation, sky sources, emitter scan, lit packed snapshots | runtime + unit | **done** — authoritative generated chunks now publish vanilla stored sky/block light bytes with `lightCorrect` |

## Liquid Simulation Arc

Liquids are authoritative simulation data and renderer input. Use [`../liquids.md`](../liquids.md) as the durable reference before writing tactical slices. The intended split is a direct 1.17.1 port of water state/flow/tick rules in the authoritative host, with engine-native worker scheduling and block/tick facts flowing through snapshots/deltas into render-world meshing. Do not put propagation on the browser main thread, do not scan all water blocks every frame, and do not settle liquids by advancing an arbitrary number of ticks during load.

| Doc | Modules | Validation tier | Purpose |
|---|---|---|---|
| [`Liquid0-liquid-oracle-foundation.md`](Liquid0-liquid-oracle-foundation.md) | scripted official-server liquid scenarios, bounded region fixtures preserving water `level`, pending `LiquidTicks` extraction, fixture comparison helpers | unit + server oracle | **done** — water-slope official-server fixture and reusable bounded comparison helpers are committed |
| [`Liquid1-liquid-simulation-foundation.md`](Liquid1-liquid-simulation-foundation.md) | `FluidState`, `FlowingFluid`, `WaterFluid`, `LiquidBlock` level mapping, vanilla-shaped liquid tick queue, first fixture comparison | unit + server oracle | **done** — test-local water simulation matches the Liquid0 water-slope fixture exactly |
| [`Liquid2-authoritative-host-integration.md`](Liquid2-authoritative-host-integration.md) | host-owned liquid tick queue, chunk tick hydration, runtime water execution, dirty chunk snapshot publication, pending tick persistence | runtime + unit | **done** — generated/stored pending liquid ticks now execute through the authoritative host and republish dirty chunks through the existing snapshot protocol |

## Creature / Entity Arc

Entities are authoritative simulation data. Use [`../entities.md`](../entities.md) and [`../creatures.md`](../creatures.md) as the durable references before tactical creature work.

| Doc | Modules | Validation tier | Purpose |
|---|---|---|---|
| [`Creatures0-generation-entity-oracle-foundation.md`](Creatures0-generation-entity-oracle-foundation.md) | official-server entity fixture extraction and committed sheep fixture | server oracle | **done** — generated entity facts can be observed from vanilla saves |
| [`Entities0-runtime-entity-foundation.md`](Entities0-runtime-entity-foundation.md) | host-owned entity sections, visibility, tick list, and memory persistence shape | unit | **done** — runtime entity lifecycle has a vanilla-shaped host-owned home |
| [`Creatures1-generation-passive-spawning.md`](Creatures1-generation-passive-spawning.md) | passive `CREATURE` generation path, spawn settings, placements, sheep color | unit + fixture | **done** — generation-time passive spawning matches the committed fixture through the runtime sink |
| [`Creatures2-host-entity-publication.md`](Creatures2-host-entity-publication.md) | generated-world host entity runtime, entity snapshots, local/remote client caches | runtime | **done** — generated original mobs now publish as authoritative protocol data |
| `Creatures3-` | browser presentation of authoritative entity snapshots | browser visual | **next** — draw simple placeholders at host-owned entity positions before real models or behavior |

## Renderer oracle approach

- **Unit**: dump atlas UVs, baked-model quads, and section visibility graphs from MC as JSON; exact-diff those. Catches most correctness bugs before pixels are involved.
- **Visual inspection**: run the browser harness from [`09a-renderer-browser-harness.md`](09a-renderer-browser-harness.md) — system Chrome via Playwright `channel: "chrome"` against a Vite-served page. Take a screenshot and look at it. Does the geometry look right? Are colors and UVs sensible? This is a human (or agent) eyeball check, not an automated diff.

## Runtime / host arc (rough, cross-cutting)

This arc is intentionally separate from the numbered worldgen and renderer translation arcs above. It is about correcting the current execution model so the engine can support browser singleplayer without main-thread stalls, browser multiplayer clients, and a headless Node host.

The first three runtime prerequisites are now landed, and they should generally take precedence over additional parity slices until:

- the renderer no longer owns chunk generation
- browser singleplayer runs behind an authoritative local host boundary
- chunk meshing no longer stalls the main thread

Those conditions are now satisfied by `R0` through `R8`. The next runtime/host priority is deciding whether the now-live browser control path keeps authoritative player/session work responsive while chunk jobs are active, and only then deciding whether the current poll-based remote transport still fits.

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
| `R2-` | client mesh worker pipeline: section-meshing jobs, worker-facing mesh input/output records, render-thread upload handoff, chunk rebuild scheduling cleanup | browser smoke + perf probe | **done** — browser chunk rebuilds now send snapshot-backed section-mesh jobs to a client mesh worker and only perform GPU uploads on the render thread |
| [`R3-browser-persistence.md`](R3-browser-persistence.md) | persistence interfaces plus browser adapter: `WorldStorage`, `ChunkStorage`, save metadata, IndexedDB-backed implementation, load/evict policy hooks | unit + browser smoke | **done** — browser singleplayer now persists authoritative save metadata and chunk snapshots through a worker-owned IndexedDB adapter behind engine-native storage contracts |
| [`R4-node-host.md`](R4-node-host.md) | headless Node host: authoritative server runtime bootstrap, file-backed storage adapter, CLI/config entry point, local integration harness | integration | **done** — the same authoritative generated-world host/runtime core now boots in Node with a file-backed storage adapter and a spawned CLI/config harness |
| [`R5-remote-browser-transport.md`](R5-remote-browser-transport.md) | remote browser-client transport to dedicated host: network transport adapter, connection/session bootstrap, browser client consuming remote chunk/state stream, two-client local smoke | integration + 2-client smoke | **done** — browser clients now consume the authoritative chunk/state stream from a dedicated Node host over a remote HTTP session transport, and the browser smoke runs two pages against that host |
| [`R6-protocol-hardening.md`](R6-protocol-hardening.md) | protocol hardening: reconnect/resync, chunk interest management, baseline player/session state sync, error handling and versioning discipline | integration | **done** — the remote path now has versioned envelopes, resumable sessions, per-session chunk interest, and shared dedicated-host authority per save instead of one host per remote session |
| [`R7-authoritative-player-loop.md`](R7-authoritative-player-loop.md) | authoritative player/session loop: player input commands, authoritative player-state snapshots, server-driven update flow, dedicated-host shared-session ticking | integration + browser smoke | **done** — the host/client boundary now carries player input, authoritative player-state snapshots, and polled server-originated updates on top of shared session authority |
| [`R8-authoritative-browser-control-integration.md`](R8-authoritative-browser-control-integration.md) | authoritative browser control/camera integration: bind debug/browser camera to `player_state`, translate browser input to `set_player_input`, schedule live update polling, keep renderer presentation-only | browser smoke + live debug path | **done** — the live browser debug path now drives authoritative player input/state against the same host boundary as remote clients, and chunk interest follows authoritative player state instead of a renderer-owned free-cam |
| `R9-` | optional browser-hosted peer/server or push-capable transport: WebSocket/WebTransport/WebRTC-style adapter reusing the same protocol and host boundary if polling stops fitting | integration | slot in a different transport later without redesigning the engine around it up front |

The first tactical to plan in detail from this arc should be `R0-`, not `R7-`. If `R0-` and `R1-` are not real, every later runtime mode becomes a special case.

## Runtime data / protocol / loading arc (rough, cross-cutting)

This arc replaces the old draft `P0`/`P1`/`P2` performance-packing notes. The durable decisions now live in:

- [`../runtime-data-model.md`](../runtime-data-model.md): block-state ids, packed sections, chunk snapshots/deltas, client chunk ownership, and `SharedArrayBuffer` criteria
- [`../protocol.md`](../protocol.md): logical host/client messages, session lifecycle, wire codecs, and polling-vs-push transport guidance
- [`../loading-persistence.md`](../loading-persistence.md): create/open/join flow, chunk lifecycle, lazy persistence, storage adapters, and aggregate remote interest

The tactical track should now stay implementation-sized. It should not re-argue the architecture unless new measurements or parity constraints invalidate the durable docs.

Governing rules:

- vanilla-shaped chunk facts are the target data model
- local singleplayer and remote multiplayer use the same logical protocol
- loading and persistence are authoritative-host responsibilities
- the browser main thread owns presentation, not raw chunk sections
- packed typed-array payloads come before any `SharedArrayBuffer` work
- `SharedArrayBuffer` is considered only after ownership, packing, and measurement justify it

Accepted end state for this arc:

- full `BlockStateId` mapping and 1.17.1-compatible `BitStorage` are shared runtime primitives
- authoritative chunk snapshots use packed, vanilla-shaped section facts rather than hot-path name/property object graphs
- browser and Node storage adapters persist the same logical chunk records behind adapter-specific physical layouts
- local worker and remote transports carry the same logical protocol and packed chunk facts through appropriate wire codecs
- browser main thread no longer owns raw chunk sections or gathers chunk neighborhoods for meshing
- browser singleplayer and browser multiplayer feed the same client render-world/cache/meshing path
- create/open/join, chunk loading, dirty-state, lazy-save, and eviction policy are explicit host responsibilities
- performance measurements identify the remaining bottleneck as GPU upload/render bookkeeping or prove that a push/SAB follow-up is needed

Recommended sequence:

| Doc | Modules | Validation tier | Purpose |
|---|---|---|---|
| `D0-` | durable architecture consolidation | docs review | **done by the architecture docs above**; keep future tacticals short and refer back to durable decisions |
| [`D1-block-state-id-and-bitstorage-foundation.md`](D1-block-state-id-and-bitstorage-foundation.md) | shared `BlockStateId` table and runtime `BitStorage` helper | unit | **landed** foundation for packed sections without changing ownership or protocol |
| [`D2-packed-section-codecs.md`](D2-packed-section-codecs.md) | packed section codecs and chunk snapshot model | unit + fixture roundtrip | **landed** vanilla-shaped packed section records behind compatibility adapters |
| [`D3-packed-chunk-storage-protocol.md`](D3-packed-chunk-storage-protocol.md) | storage/protocol rollout for packed chunk facts | unit + integration | **landed** authoritative chunk snapshots move through packed records across worker, remote, IndexedDB, and file adapters |
| [`D4-browser-render-world-ownership.md`](D4-browser-render-world-ownership.md) | browser render-world ownership | perf probe + browser visual | **done** — live browser rendering now feeds packed chunks into a render-world worker, keeps raw chunk ownership and mesh-neighborhood gathering off the main thread, and passes the full browser validation gate |
| [`D5-transport-measurement-and-push-sab-decision.md`](D5-transport-measurement-and-push-sab-decision.md) | transport measurement and push/SAB decision | perf probe + deployment check | **done** — the repeatable traversal harness identified host chunk scheduling as the bottleneck, not polling, `SharedArrayBuffer`, or render-world subworkers |
| [`D6-authoritative-host-scheduler.md`](D6-authoritative-host-scheduler.md) | authoritative host scheduler | runtime + browser perf probe | **done** — chunk interest now streams through cooperative host phases, `pnpm perf:d5` stays bounded, and manual `debug.html?viewDistance=1` walking no longer shows stalls |
| [`D7-loading-persistence-hygiene.md`](D7-loading-persistence-hygiene.md) | loading/persistence hygiene | unit + browser smoke/probe | **done** — honest progress labels, explicit IndexedDB reset, persisted-light omission, and storage-version discipline are landed |
| [`D8-dirty-cache-save-policy.md`](D8-dirty-cache-save-policy.md) | dirty/cache save policy | unit | **done** — host code now distinguishes generated-cache writes from dirty durable saves, marks host block mutations dirty, and saves dirty chunks before eviction |

`D5` and `D6` close the scheduler/transport decision for now. `D7` and `D8` make current loading/persistence behavior visible, deterministic, and less ambiguous before larger persistence work. Keep `pnpm perf:d5` as the regression gate when changing loading, scheduling, transport, packed snapshots, or render-world ingestion. Do not start push transport, `SharedArrayBuffer`, render-world subworkers, or client-side prediction unless a later D5-style report selects that path.

With the runtime arc accepted and the spawn full-decorated fixture exact, the next highest-value content-parity slice is no longer broad biome-table breadth. Tactical [`50`](50-beach-river-full-decorated-parity.md) should extend the exact full-block harness to seed `12345`, chunk `(5,115)`, the existing sand/gravel surface-oracle target, and prove legitimate shoreline/river loose material plus decoration parity under a scheduler-pinned server fixture. Tactical [`49`](49-vanilla-status-futures-and-partial-chunks.md) remains the separate status-orchestration and throughput follow-through for replacing the flattened authority terrain window.

## Skipped entirely (for renderer MVP)

`entity/`, `blockentity/`, `item/`, `PostChain` post-processing, particles, GUI, `CubeMap`/`PanoramaRenderer`, sounds (`com.mojang.blaze3d.audio`), `OutlineBufferSource`, `DimensionSpecialEffects` beyond overworld defaults.
