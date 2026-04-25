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
- translated rendering for water, tint, dark-oak trees, acacia trees, jungle trees, bamboo, mega spruce / mega pine conifers, huge mushrooms, grass, flowers, lily pads, seagrass, kelp, coral, sea pickles, mushrooms, cactus, sugar cane, vine, cocoa, melon, and related surface features

Several later worldgen capabilities landed through renderer-driven tacticals rather than through the original worldgen arc, so this document should be treated as the authoritative status view when it disagrees with the older tactical sequence.

The main remaining gap is not foundational plumbing. It is parity and confidence, with any remaining breadth work now secondary:

- parity: burn down the remaining vanilla overworld content mismatches exposed by full decorated fixtures
- confidence: promote later-stage worldgen from focused unit/browser checks to full server-backed chunk diffs
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
| Tree pipeline | `80-90%` | Oak / swamp oak / fancy oak / spruce / pine / mega pine / mega spruce / birch / dark oak / acacia / jungle / mega-jungle / bamboo-jungle / huge-mushroom paths exist; bee-related parity still does not | [`23`](./tactical/23-true-tree-feature-placement.md), [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`41`](./tactical/41-bamboo-jungle-parity.md) |
| Surface vegetation + water decoration | `82-90%` | First substantial overworld set landed, now including dark-forest, savanna, jungle, bamboo-jungle, snowy, giant-taiga, mushroom-field, shoreline, river, warm-ocean, and cold-surface identity | [`20`](./tactical/20-surface-special-blocks-and-biome-tint.md), [`21`](./tactical/21-surface-feature-palette-expansion.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`37`](./tactical/37-shoreline-and-transition-parity.md), [`38`](./tactical/38-cold-surface-parity.md), [`39`](./tactical/39-warm-ocean-parity.md), [`41`](./tactical/41-bamboo-jungle-parity.md) |
| Biome decoration table coverage | `88-93%` | The full layered-overworld biome key set now has non-empty translated settings, including the last mountain / modified-jungle / badlands aliases; remaining gaps are narrower table exactness and confidence, not broad empty-table coverage | [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md), [`33`](./tactical/33-dark-forest-parity.md), [`34`](./tactical/34-savanna-parity.md), [`35`](./tactical/35-jungle-parity.md), [`36`](./tactical/36-snowy-giant-taiga-and-mushroom-table-coverage.md), [`37`](./tactical/37-shoreline-and-transition-parity.md), [`38`](./tactical/38-cold-surface-parity.md), [`39`](./tactical/39-warm-ocean-parity.md), [`41`](./tactical/41-bamboo-jungle-parity.md), [`42`](./tactical/42-ore-and-underground-decoration-foundation.md), [`43`](./tactical/43-biome-specific-underground-extras.md), [`44`](./tactical/44-underground-tail-and-soft-disks.md), [`45`](./tactical/45-overworld-biome-table-aliases-and-exactness.md) |
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
- huge vegetation: huge brown mushroom, huge red mushroom
- plants: grass, tall grass, fern, large fern, flowers, double flowers, berry bushes, mushrooms, pumpkins, melon, cactus, sugar cane, dead bush, vine, cocoa, bamboo
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

Every layered-overworld biome key in the current target now resolves to a non-empty translated settings table. The remaining gaps now tilt toward narrow exactness issues such as `deep_warm_ocean` and toward confidence/oracle depth rather than missing biome-table rows.

## What is still missing

### Common biome families still missing or thin

No layered-overworld biome key in the current target still falls back to a carver-only settings table. The remaining biome-table gaps are narrow exactness issues rather than broad missing families. Important ones now include:

- exact `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK` follow-through and later coral state/death-tick behavior if narrow ocean-table exactness becomes worth another slice

### Tree and decorator parity gaps

The current tree system is enough to render believable forests, but not enough for broad vanilla parity. Notable missing classes of work:

- bee-related tree decorators
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

The target architecture is status-aware rather than a decoration shortcut: structure starts are recorded first, references are computed before noise, and each chunk places clipped structure slices during `FEATURES`. See [`structures.md`](./structures.md) for structure details and [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md) for the broader status-order contract.

## Current confidence level

Not every landed bucket has the same validation strength.

| Bucket | Confidence | Why |
|---|---|---|
| PRNG / noise / terrain sampling / biome source | High | These are the oldest and best-documented worldgen slices, with tactical docs and oracle-oriented work |
| Surface path | High | Landed and visible in generated frames, with committed fixture coverage for the major live surface families in the current overworld path |
| Carvers | Medium-high | AIR and LIQUID classic overworld carvers are now integrated, scheduled underwater tick consequences are captured, and the live surface/material matrix is broad, but exhaustive parity still needs block-state/tick follow-through decisions |
| Feature/decor framework | Medium | Good unit coverage on individual feature families, but the first full decorated server-fixture baseline still has `746 / 65,536` block mismatches |
| Biome decoration tables | Medium | The layered-overworld key set is now covered; remaining risk is exact feature-table ordering/selection rather than broad empty-table coverage |

### Decorated chunk baseline

The next parity milestone is exact block parity for the existing official-server fixture `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`.

For seed `12345`, chunk `(0, 0)`, staged terrain/surface/carver generation already matches the Java oracle exactly. The full runtime decorated chunk currently matches `64,790 / 65,536` block positions (`98.86%`). The remaining `746` full-block mismatches are concentrated in a few actionable buckets:

| Bucket | Current mismatches |
|---|---:|
| Tree logs/leaves placement | `590` |
| Carver/fluid edge | `80` |
| Deep underground blobs/lava | `31` |
| Plants/snow decoration | `26` |
| Surface dirt/grass choice | `15` |
| Ores/glow lichen | `4` |

The current ground-surface regression test for this same chunk is intentionally weaker: it compares one derived ground block per column, matches `232 / 256` materials, and rejects unexpected dry-land sand. Tactical [`47`](./tactical/47-generated-chunk-status-orchestration.md) has landed the explicit status-order foundation, but exact decorated parity now needs tactical [`48`](./tactical/48-vanilla-scheduler-trace-oracle.md) first: neighboring `FEATURES` passes can write into the same target chunk, and the remaining tree/log/leaf mismatch bucket may depend on vanilla's cross-chunk commit order. After 48 verifies that order, tactical [`46`](./tactical/46-full-decorated-spawn-chunk-parity.md) should replace the weaker confidence shape with an exact full-block decorated-chunk oracle.

## Priorities

This is the current recommended ordering for worldgen work.

These priorities are only for parity-oriented worldgen work. The runtime/host arc already landed the browser-local authority, mesh-worker, browser-persistence, headless-Node-host, remote-browser-transport, protocol-hardening, first authoritative-player-loop, and browser-control integration prerequisites (`R0` through `R8`), so parity work no longer has to wait on the old browser render-path coupling. Remaining runtime/host work still matters, but it now shifts toward measuring whether polling remains sufficient under the live browser control path and then growing richer authoritative gameplay on top of the same boundary; see the runtime/host arc in [`tactical/README.md`](./tactical/README.md).

### 1. Trace vanilla scheduler order for cross-chunk `FEATURES`

Tactical [`48`](./tactical/48-vanilla-scheduler-trace-oracle.md) is the immediate blocker before more decorated mismatch burn-down. It should produce an executable vanilla 1.17.1 scheduler trace for a bounded seed `12345`, chunk `(0, 0)` load scenario and identify the `FEATURES` completion/commit order for the 3x3 neighborhood around the target chunk.

### 2. Reach exact full-decorated parity for the spawn baseline chunk

The next broad parity win is no longer table breadth. The layered-overworld biome key set is covered now, the explicit status-order foundation has landed, and the project has enough feature/decorator surface area to make a stronger claim: one simple official-server chunk should match exactly.

The priority target is seed `12345`, chunk `(0, 0)`:

- use the reusable full-decorated chunk diff helper
- keep the full-block runtime check ratcheted against the exact oracle target
- burn down the current `746 / 65,536` mismatches, starting with tree placement
- keep the staged terrain/surface/carver oracle tests exact throughout

See [`46-full-decorated-spawn-chunk-parity.md`](./tactical/46-full-decorated-spawn-chunk-parity.md).

### 3. Narrow biome-table exactness for remaining overworld edge cases

After the spawn chunk is exact, use the new full-decorated diff harness to decide which narrow biome-table exactness issues are still worth a dedicated slice.

Candidate families:

- narrower ocean-table exactness like `deep_warm_ocean` `SEAGRASS_SIMPLE` / `CARVING_MASK`
- any remaining helper-level mismatches that are visible only because the broad biome-table holes are closed
- fixture-driven follow-through for shoreline, taiga/snowy slope, desert, badlands, or ocean chunks

### 4. Finish the remaining tree/decorator ecosystems

The current vegetation set is already enough to make scenes legible, and dark forest, savanna, and the first jungle slice are now in the covered set. The next leverage point is the still-missing ecosystems that unlock the next whole biome identities or finish the ones that are only partially covered:

- bees
- remaining biome-specific decorators

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
