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
- Out of scope for the current target: the disabled Caves & Cliffs Part 1 paths called out in [`../AGENTS.md`](../AGENTS.md) (`Aquifer`, `Cavifier`, `NoodleCavifier`, `OreVeinifier`, deepslate substitution, and their `NormalNoise`-driven branches).

## Current state

The project is past the “terrain demo” phase. The renderer is already consuming generated chunks from the translated worldgen pipeline, and the current browser smoke shows recognizable overworld terrain with:

- translated biome source
- translated terrain density sampling
- translated surface material placement
- translated classic air carvers
- translated first-pass biome decoration
- translated rendering for water, tint, trees, grass, flowers, lily pads, seagrass, mushrooms, cactus, sugar cane, and related surface features

Several later worldgen capabilities landed through renderer-driven tacticals rather than through the original worldgen arc, so this document should be treated as the authoritative status view when it disagrees with the older tactical sequence.

The main remaining gap is not foundational plumbing. It is breadth, parity, and confidence:

- breadth: more biome tables and feature families
- parity: more of the vanilla overworld content matrix
- confidence: stronger oracle coverage for later-stage worldgen than we currently have

## Rough completion by bucket

| Bucket | Rough coverage | State | Key references |
|---|---:|---|---|
| PRNG + core noise primitives | `95-100%` | Landed and well-covered | [`00`](./tactical/00-worldgen-ts-port.md), [`01`](./tactical/01-noise-octaves.md), [`02`](./tactical/02-remaining-synth.md) |
| `NoiseSampler` + terrain density | `90-95%` | Landed and driving generated chunks | [`03`](./tactical/03-noise-sampler-settings.md), [`06`](./tactical/06-noise-based-chunk-generator.md) |
| Overworld biome source / layered biome pipeline | `90-95%` | Landed and driving terrain + decoration lookup | [`05`](./tactical/05-overworld-biome-source.md) |
| Surface rules / bedrock / top materials | `85-90%` | Landed for current overworld path | [`06a`](./tactical/06a-pre-07-surface-prep.md), [`07`](./tactical/07-surface-builders.md) |
| Classic carvers (`CaveWorldCarver`, `CanyonWorldCarver`) | `75-85%` | Implemented and integrated, but under-documented and under-oracled compared to earlier terrain stages | code: [`src/worldgen/carver/`](../src/worldgen/carver), generator hook: [`noise-based-chunk-generator.ts`](../src/worldgen/levelgen/noise-based-chunk-generator.ts) |
| Feature/decorator framework | `70-80%` | Enough for current vegetation and water feature set | [`22`](./tactical/22-simple-feature-placement-bridge.md), [`24`](./tactical/24-biome-vegetation-decoration-bridge.md) |
| Tree pipeline | `55-65%` | Oak / swamp oak / fancy oak / spruce / pine / birch paths exist; broader tree parity does not | [`23`](./tactical/23-true-tree-feature-placement.md), [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md) |
| Surface vegetation + water decoration | `55-65%` | First substantial overworld set landed | [`20`](./tactical/20-surface-special-blocks-and-biome-tint.md), [`21`](./tactical/21-surface-feature-palette-expansion.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md) |
| Biome decoration table coverage | `35-45%` | A useful subset is real; many biome keys still fall back to `BiomeGenerationSettings.EMPTY` | [`24`](./tactical/24-biome-vegetation-decoration-bridge.md), [`25`](./tactical/25-biome-decoration-palette-expansion.md), [`26`](./tactical/26-overworld-water-and-swamp-decoration.md), [`27`](./tactical/27-biome-decoration-parity-follow-through.md) |
| Ore generation / underground decoration | `0-10%` | Not meaningfully started | target bucket only |
| Structures | `0-5%` | Not meaningfully started | target bucket only |

If you compress all of that to one number, the project is roughly `60-70%` of the way to “recognizable vanilla-overworld worldgen,” but much less complete than that for broad biome/decor/structure parity.

## What is concretely landed

### Terrain backbone

- `NoiseBasedChunkGenerator` is live and feeds the generated render level.
- `OverworldBiomeSource` and the layered biome area pipeline are live.
- Surface builders are live for the current overworld path.
- Classic air carvers are live and called from `NoiseBasedChunkGenerator.applyCarvers(...)`.

### Feature plumbing

The project now has translated support for:

- `Feature`, `ConfiguredFeature`, `DecoratedFeature`
- common decorator chains such as `count`, `count_extra`, `square`, `heightmap`, `water_depth_threshold`, `range`, `spread_32_above`, and `count_noise`
- `RandomPatchFeature`
- `SimpleBlockFeature`
- `TreeFeature`
- `LakeFeature`
- `SpringFeature`
- `SeagrassFeature`
- flower-provider-backed flower placement

### Current tree / plant / water feature families

The current worldgen path covers a meaningful first-pass overworld set:

- trees: oak, swamp oak, fancy oak, spruce, pine, birch, tall birch
- plants: grass, tall grass, fern, large fern, flowers, double flowers, berry bushes, mushrooms, pumpkins, cactus, sugar cane, dead bush
- water/surface flora: lily pads, seagrass, tall seagrass
- water features: water lakes and water springs

### Current biome-table coverage

`src/worldgen/biome/overworld-biome-generation-settings.ts` currently has non-empty translated settings for:

- badlands, badlands plateau
- desert, desert hills, desert lakes
- forest, wooded hills, flower forest
- birch forest, birch forest hills, tall birch forest, tall birch hills
- plains, sunflower plains
- swamp, swamp hills
- mountains, wooded mountains, mountain edge
- taiga, taiga hills, taiga mountains

That is enough to produce varied generated scenes, but it is still a subset of the overworld biome matrix.

## What is still missing

### Common biome families still missing or thin

Many biome keys still fall back to `BiomeGenerationSettings.EMPTY`, which means the terrain and surfaces exist but the biome-specific decoration pass is still absent or very reduced. Important gaps include:

- dark forest
- jungle variants
- savanna variants
- snowy biomes
- giant-tree taiga variants
- mushroom fields
- beaches
- rivers
- most ocean variants beyond the biome ID layer itself

### Tree and decorator parity gaps

The current tree system is enough to render believable forests, but not enough for broad vanilla parity. Notable missing classes of work:

- dark oak tree path
- dark-forest canopy behavior
- huge mushroom features
- vine decorators
- bee-related tree decorators
- more biome-specific trunk/foliage/feature-size combinations

### Underground content gaps

The project does not yet have meaningful overworld underground decoration parity:

- ore features
- ore distribution tables
- non-ore underground feature families

### Structures

Structure generation is still effectively absent:

- no village/structure placement pipeline
- no start/piece/jigsaw system
- no generated structure injection into chunks

## Current confidence level

Not every landed bucket has the same validation strength.

| Bucket | Confidence | Why |
|---|---|---|
| PRNG / noise / terrain sampling / biome source | High | These are the oldest and best-documented worldgen slices, with tactical docs and oracle-oriented work |
| Surface path | Medium-high | Landed and visible in generated frames, with earlier tactical coverage |
| Carvers | Medium | The code is present and integrated, but there is no dedicated tactical doc or explicit oracle pass comparable to earlier terrain stages |
| Feature/decor framework | Medium | Good unit coverage on individual feature families, but not broad seed-parity coverage across many biome tables |
| Biome decoration tables | Medium-low | Several important biomes are still empty or reduced, so coverage breadth is the main limitation |

## Priorities

This is the current recommended ordering for worldgen work.

### 1. Raise carvers to first-class status

Carvers are already implemented, which means the priority is not “start carvers” but “treat carvers as a major terrain milestone instead of a hidden side-path.”

Why this is high priority:

- carvers materially change terrain recognizability more than another incremental surface-decoration slice
- the tactical index still reads as if carvers are the missing MVP terrain step
- current confidence is lower than it should be for such a central part of overworld shape

What this means in practice:

- add a dedicated worldgen doc for the carver path, even if it is backfilled after implementation
- add chunk-level oracle coverage or other strong parity checks
- capture browser shots that intentionally expose cave mouths / ravines rather than only surface vegetation

### 2. Expand biome-table coverage for common overworld families

The next broad parity win is filling out the many biomes that still resolve to `BiomeGenerationSettings.EMPTY`.

Highest-value families:

- dark forest
- jungle
- savanna
- snowy biomes
- giant-tree taiga
- river / beach / ocean follow-through

This is a larger win than adding more variants inside already-covered forest/plains/swamp paths.

### 3. Finish the missing tree/decorator ecosystems

The current vegetation set is already enough to make scenes legible. The next leverage point is the missing ecosystems that unlock whole biome identities:

- dark oak
- huge mushrooms
- vines
- bees

### 4. Start ore and underground decoration

Once terrain/carver confidence and biome breadth are in better shape, ores become the next major “this is actually Minecraft” milestone for chunk contents.

### 5. Structures after the terrain/decor core is stable

Structures are important for parity, but they should not displace the terrain/carver/biome-decor core unless priorities change.

## Update rules

When a tactical lands that changes worldgen, update this document in the same change:

- adjust the rough coverage ranges
- move buckets between “landed”, “partial”, and “missing”
- update the priority order if the leverage changed
- add links to the new tactical doc
- keep the “Current biome-table coverage” section honest about which biomes still fall back to `BiomeGenerationSettings.EMPTY`

This document should be the authoritative worldgen status page. Tactical docs are the work log; this file is the map.
