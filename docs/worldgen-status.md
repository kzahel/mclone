# Worldgen Status

Living overview of the actual worldgen state in the codebase.

Unlike [`docs/tactical/`](./tactical/README.md), which is session-scoped and intentionally short-lived, this document is meant to answer four standing questions:

- What is really landed today?
- How complete is each major worldgen bucket?
- What is still missing for vanilla 1.17.1 overworld parity?
- What should be prioritized next?

Treat the percentages below as directional, not promises.

## Scope

- Target: vanilla Java `1.17.1` overworld seed parity.
- Method: direct translation from the decompiled Java source under `reference/minecraft-1.17.1/src/`.
- Ordering contract: [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md) is the canonical source for chunk-status dependencies, decoration finality, lighting gates, and publication gates.
- Out of scope for the current target: the disabled Caves & Cliffs Part 1 paths called out in [`../AGENTS.md`](../AGENTS.md) (`Aquifer`, `Cavifier`, `NoodleCavifier`, `OreVeinifier`, deepslate substitution, and their `NormalNoise`-driven branches).

## Current state

The project is past the “terrain demo” phase. The renderer is already consuming generated chunks from the translated worldgen pipeline, and the current browser smoke shows recognizable overworld terrain with:

- translated biome source
- translated terrain density sampling
- translated surface material placement
- translated classic air carvers
- translated first-pass biome decoration
- translated common overworld ore generation, the active underground variety material blobs, the live badlands/mountain underground extras, glow lichen, rare dripstone, and soft disks
- translated rendering for water, tint, dark-oak trees, acacia trees, jungle trees, bamboo, mega spruce / mega pine conifers, huge mushrooms, bee nests, desert wells, grass, flowers, sunflowers, lily pads, seagrass, kelp, coral, sea pickles, mushrooms, cactus, sugar cane, vine, cocoa, melon, and related surface features

Several later worldgen capabilities landed through renderer-driven tacticals rather than through the original worldgen arc, so this document should be treated as the authoritative status view when it disagrees with the older tactical sequence.

The main remaining gap is not foundational plumbing. It is parity breadth and confidence. One scheduler-pinned full decorated chunk now matches exactly, but that is still only one run shape and one biome neighborhood:

- parity: extend full decorated fixtures to boundary chunks where loose materials, fluids, and cross-chunk decoration have more ways to fail
- confidence: promote later-stage worldgen from focused unit/browser checks to a small matrix of full server-backed chunk diffs
- breadth: add more biome tables or feature families only when a concrete oracle target proves they matter

## Rough completion by bucket

| Bucket | Rough coverage | State | Key references |
|---|---:|---|---|
| PRNG + core noise primitives | `95-100%` | Landed and well-covered | [`00`](./tactical/00-worldgen-ts-port.md), [`01`](./tactical/01-noise-octaves.md), [`02`](./tactical/02-remaining-synth.md) |
| `NoiseSampler` + terrain density | `90-95%` | Landed and driving generated chunks | [`03`](./tactical/03-noise-sampler-settings.md), [`06`](./tactical/06-noise-based-chunk-generator.md) |
| Overworld biome source / layered biome pipeline | `90-95%` | Landed and driving terrain + decoration lookup | [`05`](./tactical/05-overworld-biome-source.md) |
| Surface rules / bedrock / top materials | `88-92%` | Landed for the current overworld path, now including frozen-ocean, badlands, giant-tree-taiga, shattered-savanna, and mushroom follow-through | [`06a`](./tactical/06a-pre-07-surface-prep.md), [`07`](./tactical/07-surface-builders.md), [`31`](./tactical/31-frozen-and-badlands-material-matrix.md), [`32`](./tactical/32-podzol-coarse-dirt-and-mycelium-matrix.md) |
| Classic carvers (`CaveWorldCarver`, `CanyonWorldCarver`) | `85-90%` | Implemented for the overworld AIR+LIQUID path, integrated, and now backed by widened desert/ocean/frozen/badlands/podzol/coarse-dirt/mycelium material coverage, scheduled-tick capture, and a broader carved-fixture matrix, but still below full vanilla parity | code: [`src/worldgen/carver/`](../src/worldgen/carver), tacticals: [`28`](./tactical/28-carver-material-parity-and-oracle-expansion.md), [`29`](./tactical/29-underwater-liquid-carver-parity.md), [`30`](./tactical/30-liquid-floor-oracle-and-tick-capture.md), [`31`](./tactical/31-frozen-and-badlands-material-matrix.md), [`32`](./tactical/32-podzol-coarse-dirt-and-mycelium-matrix.md), status: [`carver-status.md`](./carver-status.md), generator hook: [`noise-based-chunk-generator.ts`](../src/worldgen/levelgen/noise-based-chunk-generator.ts) |
| Feature/decorator framework | `90-95%` | Enough for the current vegetation, water-feature, dark-forest, savanna, jungle, bamboo-jungle, snowy, giant-taiga, mushroom-field, shoreline, river, cold-surface, warm-ocean, common-ore placement, the replace-single-block underground extra path, glow lichen, rare dripstone, and soft disks | [`22`](./tactical/22-simple-feature-placement-bridge.md), [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`37`](./tactical/37-shoreline-and-transition-parity.md), [`38`](./tactical/38-cold-surface-parity.md), [`39`](./tactical/39-warm-ocean-parity.md), [`41`](./tactical/41-bamboo-jungle-parity.md), [`42`](./tactical/42-ore-and-underground-decoration-foundation.md), [`43`](./tactical/43-biome-specific-underground-extras.md), [`44`](./tactical/44-underground-tail-and-soft-disks.md) |
| Tree pipeline | `85-92%` | Oak / swamp oak / fancy oak / spruce / pine / mega pine / mega spruce / birch / dark oak / acacia / jungle / mega-jungle / bamboo-jungle / huge-mushroom paths exist, bee-decorated oak/birch/fancy-oak variants are now wired through the live flower-forest, birch, and plains selectors, swamp-oak leaf vines now match vanilla again, and the remaining gaps are narrower selector/decorator exactness | [`23`](./tactical/23-true-tree-feature-placement.md), [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`41`](./tactical/41-bamboo-jungle-parity.md), [`51`](./tactical/51-bee-tree-decorator-follow-through.md), [`53`](./tactical/53-swamp-oak-vine-follow-through.md), [`54`](./tactical/54-deep-warm-ocean-seagrass-simple-follow-through.md) |
| Surface vegetation + water decoration | `84-91%` | First substantial overworld set landed, now including dark-forest, savanna, jungle, bamboo-jungle, snowy, giant-taiga, mushroom-field, shoreline, river, warm-ocean, cold-surface identity, and the translated desert-well `SURFACE_STRUCTURES` oddity | [`20`](./tactical/20-surface-special-blocks-and-biome-tint.md), [`21`](./tactical/21-surface-feature-palette-expansion.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`37`](./tactical/37-shoreline-and-transition-parity.md), [`38`](./tactical/38-cold-surface-parity.md), [`39`](./tactical/39-warm-ocean-parity.md), [`41`](./tactical/41-bamboo-jungle-parity.md), [`55`](./tactical/55-desert-well-follow-through.md) |
| Biome decoration table coverage | `88-93%` | The full layered-overworld biome key set now has non-empty translated settings, including the last mountain / modified-jungle / badlands aliases and the non-natural `deep_warm_ocean` seagrass exactness follow-through; remaining gaps are narrower table exactness and confidence, not broad empty-table coverage | [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`37`](./tactical/37-shoreline-and-transition-parity.md), [`38`](./tactical/38-cold-surface-parity.md), [`39`](./tactical/39-warm-ocean-parity.md), [`41`](./tactical/41-bamboo-jungle-parity.md), [`42`](./tactical/42-ore-and-underground-decoration-foundation.md), [`43`](./tactical/43-biome-specific-underground-extras.md), [`44`](./tactical/44-underground-tail-and-soft-disks.md), [`45`](./tactical/45-overworld-biome-table-aliases-and-exactness.md), [`54`](./tactical/54-deep-warm-ocean-seagrass-simple-follow-through.md) |
| Ore generation / underground decoration | `55-60%` | The live overworld underground helper stack is now translated: common ores, underground variety, biome-specific badlands/mountain extras, glow lichen, rare dripstone, soft disks, target-rule plumbing, replace-single-block support, block/palette coverage, and biome-table wiring all landed; the main remaining gaps are stronger decorated-stage confidence/oracle coverage and later underground feature families outside this helper surface | [`42`](./tactical/42-ore-and-underground-decoration-foundation.md), [`43`](./tactical/43-biome-specific-underground-extras.md), [`44`](./tactical/44-underground-tail-and-soft-disks.md) |
| Structures | `0-5%` | Not meaningfully started; structure architecture is documented, with orchestration covered by the deterministic-order contract | [`structures.md`](./structures.md), [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md) |

If you compress all of that to one number, the project is roughly `60-70%` of the way to “recognizable vanilla-overworld worldgen,” but much less complete than that for broad biome/decor/structure parity.

## What is concretely landed

### Terrain backbone

- `NoiseBasedChunkGenerator` is live and feeds the generated render level.
- `OverworldBiomeSource` and the layered biome area pipeline are live.
- Surface builders are live for the current overworld path, including badlands, frozen-ocean, giant-tree-taiga, shattered-savanna, and mushroom follow-through.
- Classic overworld AIR and LIQUID carvers are live and called from `NoiseBasedChunkGenerator.applyCarvers(...)`; see [`carver-status.md`](./carver-status.md) for the narrower parity/oracle breakdown.

### Feature plumbing

The project now has translated support for:

- `Feature`, `ConfiguredFeature`, `DecoratedFeature`
- `RandomBooleanSelectorFeature`
- common decorator chains such as `count`, `count_extra`, `square`, `heightmap`, `water_depth_threshold`, `range`, `spread_32_above`, and `count_noise`
- `RandomPatchFeature`
- `SimpleBlockFeature`
- `TreeFeature`
- `LakeFeature`
- `SpringFeature`
- `SeagrassFeature`
- `KelpFeature`
- `CoralTreeFeature`
- `CoralClawFeature`
- `CoralMushroomFeature`
- `SeaPickleFeature`
- `SnowAndFreezeFeature`
- `IceSpikeFeature`
- disk-based `IcePatchFeature`
- `BambooFeature`
- `OreFeature`
- `ReplaceBlockFeature`
- `GlowLichenFeature`
- `DripstoneClusterFeature`
- `SmallDripstoneFeature`
- `HugeBrownMushroomFeature`
- `HugeRedMushroomFeature`
- flower-provider-backed flower placement
- ocean `count_noise_biased` decorator follow-through
- warm-ocean coral / sea-pickle selector follow-through
- bamboo-jungle bamboo / podzol / selector follow-through
- badlands oak-tree selector follow-through for wooded badlands variants
- common overworld ore target lists and default ore configured features
- active `addDefaultUndergroundVariety(...)` material blobs: dirt, gravel, granite, diorite, andesite, tuff, and deepslate
- the remaining live underground helper tail: glow lichen, rare dripstone clusters, and rare small dripstone
- soft disks: sand, clay, gravel, plus the swamp clay-only follow-through
- biome-specific underground extras: badlands extra gold, mountain emeralds, and mountain infested stone

### Current tree / plant / water feature families

The current worldgen path covers a meaningful first-pass overworld set:

- trees: oak, swamp oak, fancy oak, spruce, pine, mega pine, mega spruce, birch, tall birch, dark oak, acacia, jungle, mega jungle, jungle bush
- bee follow-through: bee-decorated oak / birch / fancy-oak variants plus generated `bee_nest` block/render support
- huge vegetation: huge brown mushroom, huge red mushroom
- plants: grass, tall grass, fern, large fern, flowers, sunflowers, double flowers, berry bushes, mushrooms, pumpkins, melon, cactus, sugar cane, dead bush, vine, cocoa, bamboo
- water/surface flora: lily pads, seagrass, tall seagrass
- ocean flora: kelp, kelp plant, live/dead coral blocks/plants/fans/wall fans, sea pickles
- cold-surface features: top-layer snow/ice freezing, packed-ice patches, ice spikes
- water features: water and lava lakes, plus water and lava springs

### Current biome-table coverage

`src/worldgen/biome/overworld-biome-generation-settings.ts` currently has non-empty translated settings for:

- badlands, wooded badlands plateau, badlands plateau, eroded badlands, modified wooded badlands plateau, modified badlands plateau
- desert, desert hills, desert lakes
- forest, wooded hills, flower forest
- birch forest, birch forest hills, tall birch forest, tall birch hills
- dark forest, dark forest hills
- giant tree taiga, giant tree taiga hills, giant spruce taiga, giant spruce taiga hills
- ice spikes
- jungle, jungle hills, jungle edge, modified jungle, modified jungle edge
- bamboo jungle, bamboo jungle hills
- beach, snowy beach, stone shore
- mushroom fields, mushroom field shore
- mountains, wooded mountains, mountain edge, gravelly mountains, modified gravelly mountains
- river, frozen river
- ocean, deep ocean, warm ocean, deep warm ocean, cold ocean, deep cold ocean, lukewarm ocean, deep lukewarm ocean, frozen ocean, deep frozen ocean
- plains, sunflower plains
- savanna, savanna plateau, shattered savanna, shattered savanna plateau
- snowy tundra, snowy mountains, snowy taiga, snowy taiga hills, snowy taiga mountains
- swamp, swamp hills
- taiga, taiga hills, taiga mountains

Every layered-overworld biome key in the current target now resolves to a non-empty translated settings table. `deep_warm_ocean` is no longer an open table gap: the table and `SEAGRASS_SIMPLE` path are now translated, and the 1.17.1 Java source shows the final ocean mix does not naturally emit that biome in ordinary overworld generation. The remaining gaps now tilt toward selector/decorator exactness and toward confidence/oracle depth rather than missing biome-table rows.

## What is still missing

### Common biome families still missing or thin

No layered-overworld biome key in the current target still falls back to a carver-only settings table. The remaining biome-table gaps are narrow exactness issues rather than broad missing families.

One important clarification from tactical [`54`](./tactical/54-deep-warm-ocean-seagrass-simple-follow-through.md): `deep_warm_ocean` now has the translated `SEAGRASS_SIMPLE` table path, but the live 1.17.1 Java `OceanMixerLayer` does not naturally surface that biome in normal overworld generation. Tactical [`55`](./tactical/55-desert-well-follow-through.md) also closes the separate desert-well oddity: desert wells are no longer a missing configured-feature path, but they still should not be confused with the deferred structure-start pipeline. Later coral state/death-tick behavior is still an optional narrow ocean follow-through if it becomes worth another slice.

### Tree and decorator parity gaps

The current tree system is enough to render believable forests, but not enough for broad vanilla parity. Notable missing classes of work:

- remaining biome-specific decorators and selector exactness
- more biome-specific trunk/foliage/feature-size combinations

### Underground content gaps

The project now covers the full live vanilla overworld underground-helper surface used by the translated biome tables, but underground parity is still far from complete:

- stronger decorated-stage / oracle coverage beyond focused unit tests and browser validation
- non-helper underground feature families
- later replace-material underground families if we decide they still matter for 1.17.1 overworld recognizability

### Structures

Structure generation is still effectively absent:

- no village/structure placement pipeline
- no start/piece/jigsaw system
- no generated structure injection into chunks

Desert wells are no longer part of this missing bucket because they are ordinary configured features, not `StructureFeature`s.

The target architecture is status-aware rather than a decoration shortcut: structure starts are recorded first, references are computed before noise, and each chunk places clipped structure slices during `FEATURES`. See [`structures.md`](./structures.md) for structure details and [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md) for the broader status-order contract.

## Current confidence level

Not every landed bucket has the same validation strength.

| Bucket | Confidence | Why |
|---|---|---|
| PRNG / noise / terrain sampling / biome source | High | These are the oldest and best-documented worldgen slices, with tactical docs and oracle-oriented work |
| Surface path | High | Landed and visible in generated frames, with committed fixture coverage for the major live surface families in the current overworld path |
| Carvers | Medium-high | AIR and LIQUID classic overworld carvers are now integrated, scheduled underwater tick consequences are captured, and the live surface/material matrix is broad, but exhaustive parity still needs block-state/tick follow-through decisions |
| Feature/decor framework | Medium-high | Good unit coverage on individual feature families, and the first scheduler-pinned full decorated server fixture now matches exactly after the measured generated-liquid tick window; broader fixture coverage is still thin |
| Biome decoration tables | Medium | The layered-overworld key set is now covered; remaining risk is exact feature-table ordering/selection rather than broad empty-table coverage |

### Decorated chunk baseline

The first exact full decorated milestone is complete for the existing scheduler-pinned official-server fixture `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`.

For seed `12345`, chunk `(0,0)`:

| Comparison | Full-block matches | Full-block mismatches |
|---|---:|---:|
| Static host snapshot, before generated-liquid host ticks | `65,533 / 65,536` | `3` |
| `liquidSimulationMode: vanilla17` after `10` deterministic host ticks | `65,536 / 65,536` | `0` |

The three static differences are expected fixture timing, not decoration mismatches: vanilla's scheduler trace probes show those cells are still air at the end of `FEATURES`, and the official-server fixture contains them only after startup advances generated liquid ticks. Tactical [`46`](./tactical/46-full-decorated-spawn-chunk-parity.md) encodes both assertions in `test/runtime/generated-world-boundary.test.ts`.

This raises the confidence bar from "one derived ground block per column" to a full-block exact server fixture, but it does not prove broad overworld parity. The next useful confidence step is a second scheduler-pinned full decorated fixture in a different material neighborhood, starting with tactical [`50`](./tactical/50-beach-river-full-decorated-parity.md): seed `12345`, chunk `(5,115)`, the existing sand/gravel surface-oracle target.

## Priorities

This is the current recommended ordering for worldgen work.

These priorities are only for parity-oriented worldgen work. The runtime/host arc already landed the browser-local authority, mesh-worker, browser-persistence, headless-Node-host, remote-browser-transport, protocol-hardening, first authoritative-player-loop, and browser-control integration prerequisites (`R0` through `R8`), so parity work no longer has to wait on the old browser render-path coupling. Remaining runtime/host work still matters, but it now shifts toward measuring whether polling remains sufficient under the live browser control path and then growing richer authoritative gameplay on top of the same boundary; see the runtime/host arc in [`tactical/README.md`](./tactical/README.md).

### 1. Expand full-decorated parity to a sand/gravel boundary

Tactical [`50`](./tactical/50-beach-river-full-decorated-parity.md) is the next best content-parity target. It uses seed `12345`, chunk `(5,115)`, which already has the committed surface-only sand/gravel oracle `test/fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json`.

This target should generate a scheduler-pinned full decorated fixture, measure the static and generated-liquid-tick diffs, then burn down exact mismatches. It specifically validates the risk that tactical 46 could not: legitimate shoreline/river loose material and soft-disk behavior in the same full-block harness that now catches tree, fluid, plant, ore, and edge-write drift.

### 2. Replace view-level status batching with vanilla status futures

The current runtime now has explicit status labels and gates, and its normal-publication closure is mostly vanilla-shaped: a 5x5 published square legitimately implies a 7x7 `FULL` gate, a 9x9 `FEATURES` gate, an 11x11 materialized terrain window, and a 25x25 metadata-status window. That count is recorded in [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md); do not treat it as accidental overgeneration by itself.

The remaining gap is data shape and durability. Tactical [`49`](./tactical/49-vanilla-status-futures-and-partial-chunks.md) is the umbrella. Tactical [`57`](./tactical/57-generated-chunk-holder-status-futures.md) has landed holder-owned status jobs, in-flight coalescing, and recursive current-view status requests instead of view-level terrain/features batches. Tactical [`59`](./tactical/59-generated-holder-residency-and-save-queue.md) has the first holder-residency and lazy generated-cache save-queue slice. Tactical [`58`](./tactical/58-generated-protochunk-partial-state-and-persistence.md) splits out partial `ProtoChunk`-like save/resume. Metadata-only `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES` inputs must stay metadata-only, and a `FEATURES` range-8 dependency must use the mixed-status window selected by `ChunkMap.getDependencyStatus(...)`.

This is a parity-shape and throughput fix, but it is no longer a blocker for the spawn exactness claim.

### 3. Add more narrow full-decorated fixtures only when the diff points there

After the sand/gravel boundary target, use the full-block diff reports to choose the next fixture deliberately. Good candidates remain:

- one taiga/snowy slope like the earlier visual regression area
- one desert or badlands chunk where loose material checks must not overfit grassland assumptions
- one ocean/shoreline chunk if ocean-table exactness or fluid follow-through is still visible

### 4. Finish the remaining narrow visible-exactness decoration slices

The current vegetation set is already enough to make scenes legible, and the bee, sunflower, swamp-oak vine, non-natural deep-warm-ocean table, and desert-well follow-through removed the clearest remaining explicit table omissions. The next leverage point is the remaining visible exactness work that is still narrow enough to validate directly:

- remaining biome-specific decorators and selector exactness
- other small non-structure decorated oddities such as monster rooms and overworld fossils

### 5. Revisit exhaustive carver parity only where the current matrix is still intentionally lossy

Classic carvers are now integrated and broadly covered across the current live surface families, so the remaining carver work is narrower and should stay driven by [`carver-status.md`](./carver-status.md) rather than by the old top-level blocker framing.

What still matters there:

- decide whether recorded scheduled underwater ticks stay a measured generation artifact or need later runtime execution
- widen the flattened numeric/oracle block model if exhaustive carved-stage diffs remain a goal
- keep ravine/cave-mouth browser validation whenever the carver path changes materially

### 6. Revisit underground confidence after the helper stack

Tacticals 42 through 44 landed the live overworld underground-helper surface. The next underground work should be confidence-driven rather than breadth-driven:

- add stronger decorated-stage/oracle coverage if the current browser/unit surface proves too weak
- only then broaden into underground families that sit outside the helper stack

### 7. Structures after the terrain/decor core is stable

Structures are important for parity, but they should not displace the terrain/carver/biome-decor core unless priorities change.

When they do become the priority, follow [`structures.md`](./structures.md) and keep the orchestration aligned with [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md): start with structure status/metadata and simple custom structures, then mineshafts, template-backed structures, strongholds/terrain blending, large structures, and finally jigsaw villages/outposts.

## Update rules

When a tactical lands that changes worldgen, update this document in the same change:

- adjust the rough coverage ranges
- move buckets between “landed”, “partial”, and “missing”
- update the priority order if the leverage changed
- add links to the new tactical doc
- keep the “Current biome-table coverage” section honest about whether any current-target biomes still fall back to carver-only settings without translated feature tables; that count is now zero

This document should be the authoritative worldgen status page. Tactical docs are the work log; this file is the map.
