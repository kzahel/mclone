# 135: Overworld Biome Palette Matrix

Status: active parent; first eight biome/tint/visible-surface probes landed
Workstream: shared native Rust worldgen, mesh tint, and deterministic vanilla visual parity

## Purpose

Create a deterministic checklist for Minecraft Java 1.17.1 overworld palette
coverage before doing more deep decorated-chunk parity. The goal is not to
judge screenshots first. The goal is to enumerate every overworld biome,
identify the visual facts that should make it read distinctly, then attach
seed/chunk fixtures and focused probes until every row has deterministic
coverage.

This matrix sits between `103-decorated-biome-fixture-matrix.md` and
`129-biome-tint-and-blending-parity.md`:

- `103` drives exact decorated `FEATURES` parity for selected biome chunks.
- `129` drives the shared grass/foliage/water tint resolver.
- This doc drives broad overworld palette coverage: biome identity, tint family,
  surface family, and high-signal visible feature families.

## Reference Source

Read before changing generation, tint, or palette fixtures:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biomes.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/newbiome/layer/LayerBiomes.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/newbiome/layer/RareBiomeLargeLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/newbiome/layer/RegionHillsLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/color/block/BlockColors.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/BiomeColors.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSpecialEffects.java`
- `native/crates/mclone-worldgen/src/biome.rs`
- `native/crates/mclone-worldgen/src/surface.rs`
- `native/crates/mclone-mesh/src/tint.rs`
- `native/crates/mclone-mesh/src/catalog.rs`

## Method

For each biome row:

1. Find a representative seed and chunk, preferably chunk `(0,0)`, with the
   existing native helper:

   ```bash
   pnpm native:worldgen:find-biome-seed -- --biome <biome> --max-seeds 10000 --count 5
   ```

2. Commit deterministic probes before relying on screenshots:
   - biome identity: block-position biome lookup resolves the expected key,
   - tint: grass, foliage, and water RGB match the expected radius-2 rule,
   - visible surface: representative `FEATURES` columns have the expected top
     material family,
   - palette features: fixture or block-family checks cover the visible feature
     family named in the matrix.

3. Use screenshots only after the probes pass, as integration sanity checks.
   Save them under `/tmp`, not in the repo.

Boundary rows should be explicit follow-ups, not hidden in the interior-biome
rows: river/forest, ocean/land, swamp/land, badlands/normal-land, and
snowy/non-snowy boundaries are the first useful set.

## Tint Groups

These are renderer-facing tint groups, not feature profiles. The color values
below are native fallback/source constants; the actual default grass and foliage
path samples the vanilla colormaps unless an override/modifier is listed.

| Tint group | Biome ids | Temperature / downfall | Grass rule | Foliage rule | Water |
|---|---|---:|---|---|---|
| ocean-default | `0`, `24` | `0.5 / 0.5` | colormap, fallback `#8eb971` | colormap, fallback `#71a74d` | `#3f76e4` |
| ocean-warm | `44`, `47` | `0.5 / 0.5` | ocean-default | ocean-default | `#43d5ee` |
| ocean-lukewarm | `45`, `48` | `0.5 / 0.5` | ocean-default | ocean-default | `#45adf2` |
| ocean-cold | `46`, `49` | `0.5 / 0.5` | ocean-default | ocean-default | `#3d57d6` |
| ocean-frozen-deep | `50` | `0.5 / 0.5` | ocean-default | ocean-default | `#3938c9` |
| plains | `1`, `16`, `129` | `0.8 / 0.4` | colormap, fallback `#91bd59` | colormap, fallback `#77ab2f` | `#3f76e4` |
| desert | `2`, `17`, `130` | `2.0 / 0.0` | colormap, fallback `#b5b755` | colormap, fallback `#aeb455` | `#3f76e4` |
| mountains | `3`, `20`, `25`, `34`, `131`, `162` | `0.2 / 0.3` | colormap, fallback `#8ab689` | colormap, fallback `#6fa078` | `#3f76e4` |
| forest | `4`, `18`, `132` | `0.7 / 0.8` | colormap, fallback `#79c05a` | colormap, fallback `#599b35` | `#3f76e4` |
| taiga | `5`, `19`, `133` | `0.25 / 0.8` | colormap, fallback `#86b783` | colormap, fallback `#689b68` | `#3f76e4` |
| river | `7` | `0.5 / 0.5` | colormap, fallback `#8eb971` | colormap, fallback `#71a74d` | `#3f76e4` |
| snowy-taiga | `30`, `31`, `158` | `-0.5 / 0.4` | colormap, fallback `#86b783` | colormap, fallback `#689b68` | `#3d57d6` |
| giant-tree-taiga | `32`, `33` | `0.3 / 0.8` | colormap, fallback `#86b783` | colormap, fallback `#689b68` | `#3f76e4` |
| giant-spruce-taiga | `160`, `161` | `0.25 / 0.8` | colormap, fallback `#86b783` | colormap, fallback `#689b68` | `#3f76e4` |
| swamp | `6`, `134` | `0.8 / 0.9` | `SWAMP` noise modifier, `#4c763c` / `#6a7039` | override `#6a7039` | `#617b64` |
| frozen-land | `12`, `13`, `140` | `0.0 / 0.5` | colormap, fallback `#80b497` | colormap, fallback `#609380` | `#3f76e4` |
| frozen-water | `10`, `11` | `0.0 / 0.5` | frozen-land | frozen-land | `#3938c9` |
| snowy-beach | `26` | `0.05 / 0.3` | colormap, fallback `#8ab689` | colormap, fallback `#6fa078` | `#3d57d6` |
| mushroom | `14`, `15` | `0.9 / 1.0` | colormap, fallback `#55c93f` | colormap, fallback `#2fb233` | `#3f76e4` |
| jungle | `21`, `22`, `149`, `168`, `169` | `0.95 / 0.9` | colormap, fallback `#59c93c` | colormap, fallback `#30bb0b` | `#3f76e4` |
| jungle-edge | `23`, `151` | `0.95 / 0.8` | colormap, fallback `#59c93c` | colormap, fallback `#30bb0b` | `#3f76e4` |
| birch | `27`, `28`, `155`, `156` | `0.6 / 0.6` | colormap, fallback `#88bb67` | colormap, fallback `#80a755` | `#3f76e4` |
| dark-forest | `29`, `157` | `0.7 / 0.8` | `DARK_FOREST` modifier over forest colormap | colormap, fallback `#599b35` | `#3f76e4` |
| savanna | `35` | `1.2 / 0.0` | colormap, fallback `#b5b755` | colormap, fallback `#aeb455` | `#3f76e4` |
| savanna-plateau | `36`, `164` | `1.0 / 0.0` | colormap, fallback `#b5b755` | colormap, fallback `#aeb455` | `#3f76e4` |
| shattered-savanna | `163` | `1.1 / 0.0` | colormap, fallback `#b5b755` | colormap, fallback `#aeb455` | `#3f76e4` |
| badlands | `37`, `38`, `39`, `165`, `166`, `167` | `2.0 / 0.0` | override `#90814d` | override `#9e814d` | `#3f76e4` |

## Biome Matrix

Legend for `Checks`: `B` biome identity, `T` tint RGB, `S` surface family,
`F` visible palette feature family.

| ID | Biome | Tint group | Surface / palette focus | Fixture | Checks |
|---:|---|---|---|---|---|
| 0 | `minecraft:ocean` | ocean-default | water, default ocean seafloor, kelp/seagrass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 1 | `minecraft:plains` | plains | grass surface, grass/flower patches, oak/birch mix | seed `16`, chunk `(0,0)`; seeds `16`, `17` exist in `103` | `[x] B [x] T [x] S [ ] F` |
| 2 | `minecraft:desert` | desert | sand/sandstone, cactus, dead bush, desert lakes | seed `38`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 3 | `minecraft:mountains` | mountains | grass/stone mountain surface, sparse trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 4 | `minecraft:forest` | forest | forest grass, oak/birch trees, flowers | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 5 | `minecraft:taiga` | taiga | spruce trees, ferns, berries | seed `125`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 6 | `minecraft:swamp` | swamp | swamp grass/water, clay, swamp trees, lily pads/seagrass | seed `7`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 7 | `minecraft:river` | river | river water, banks, seagrass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 10 | `minecraft:frozen_ocean` | frozen-water | frozen water, icebergs, blue ice | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 11 | `minecraft:frozen_river` | frozen-water | frozen river water/ice | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 12 | `minecraft:snowy_tundra` | frozen-land | snow over grass, snowy vegetation | seed `42`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 13 | `minecraft:snowy_mountains` | frozen-land | snowy mountain surface | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 14 | `minecraft:mushroom_fields` | mushroom | mycelium, mushrooms, no normal hostile palette | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 15 | `minecraft:mushroom_field_shore` | mushroom | mycelium shore transition | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 16 | `minecraft:beach` | plains | sand beach, buried-treasure/shipwreck surface context | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 17 | `minecraft:desert_hills` | desert | sand/sandstone hills, cactus/dead bush | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 18 | `minecraft:wooded_hills` | forest | forest hill trees and grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 19 | `minecraft:taiga_hills` | taiga | spruce/fern hill variant | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 20 | `minecraft:mountain_edge` | mountains | mountain-edge grass/stone transition | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 21 | `minecraft:jungle` | jungle | jungle trees, vines, bamboo-light vegetation | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 22 | `minecraft:jungle_hills` | jungle | jungle hill trees/vines | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 23 | `minecraft:jungle_edge` | jungle-edge | jungle-edge lower-density trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 24 | `minecraft:deep_ocean` | ocean-default | deep water, kelp/seagrass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 25 | `minecraft:stone_shore` | mountains | stone shore, steep coast | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 26 | `minecraft:snowy_beach` | snowy-beach | snowy sand beach, cold water | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 27 | `minecraft:birch_forest` | birch | birch leaves/trunks, forest grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 28 | `minecraft:birch_forest_hills` | birch | birch hill trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 29 | `minecraft:dark_forest` | dark-forest | dark grass modifier, dark oak canopy, mushrooms | seed `44`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 30 | `minecraft:snowy_taiga` | snowy-taiga | snowy spruce, ferns, berries | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 31 | `minecraft:snowy_taiga_hills` | snowy-taiga | snowy spruce hill variant | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 32 | `minecraft:giant_tree_taiga` | giant-tree-taiga | podzol/coarse dirt feel, giant taiga trees, ferns | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 33 | `minecraft:giant_tree_taiga_hills` | giant-tree-taiga | giant taiga hill trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 34 | `minecraft:wooded_mountains` | mountains | mountain surface with trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 35 | `minecraft:savanna` | savanna | dry grass tint, acacia trees, tall grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 36 | `minecraft:savanna_plateau` | savanna-plateau | dry plateau, acacia/grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 37 | `minecraft:badlands` | badlands | red sand, terracotta bands, dead bush | seed `147`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 38 | `minecraft:wooded_badlands_plateau` | badlands | terracotta/red sand, wooded plateau trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 39 | `minecraft:badlands_plateau` | badlands | plateau terracotta/red sand | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 44 | `minecraft:warm_ocean` | ocean-warm | turquoise water, sand, coral/sea pickles | seed `26`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 45 | `minecraft:lukewarm_ocean` | ocean-lukewarm | bright water, sand, seagrass/kelp | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 46 | `minecraft:cold_ocean` | ocean-cold | cold water, gravel/grass seafloor, kelp | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 47 | `minecraft:deep_warm_ocean` | ocean-warm | deep turquoise water, sand | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 48 | `minecraft:deep_lukewarm_ocean` | ocean-lukewarm | deep bright water, kelp | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 49 | `minecraft:deep_cold_ocean` | ocean-cold | deep cold water, kelp | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 50 | `minecraft:deep_frozen_ocean` | ocean-frozen-deep | deep frozen water, icebergs | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 129 | `minecraft:sunflower_plains` | plains | plains tint, sunflower patches | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 130 | `minecraft:desert_lakes` | desert | desert surface plus lake/fossil variants | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 131 | `minecraft:gravelly_mountains` | mountains | gravelly mountain surface | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 132 | `minecraft:flower_forest` | forest | dense flower palette, forest grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 133 | `minecraft:taiga_mountains` | taiga | spruce/fern mountain taiga | seed `12345`, chunk `(0,0)` anchor | `[ ] B [ ] T [ ] S [ ] F` |
| 134 | `minecraft:swamp_hills` | swamp | swamp tint, hill surface, fossil/vegetation variant | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 140 | `minecraft:ice_spikes` | frozen-land | snow/ice spikes and ice patches | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 149 | `minecraft:modified_jungle` | jungle | dense jungle, vines, high vegetation | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 151 | `minecraft:modified_jungle_edge` | jungle-edge | jungle-edge modified surface/vegetation | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 155 | `minecraft:tall_birch_forest` | birch | tall birch trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 156 | `minecraft:tall_birch_hills` | birch | tall birch hill trees | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 157 | `minecraft:dark_forest_hills` | dark-forest | dark forest hills, canopy/mushrooms | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 158 | `minecraft:snowy_taiga_mountains` | snowy-taiga | snowy spruce mountain taiga | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 160 | `minecraft:giant_spruce_taiga` | giant-spruce-taiga | giant spruce, podzol/coarse dirt feel | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 161 | `minecraft:giant_spruce_taiga_hills` | giant-spruce-taiga | giant spruce hill variant | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 162 | `minecraft:modified_gravelly_mountains` | mountains | modified gravelly mountain surface | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 163 | `minecraft:shattered_savanna` | shattered-savanna | extreme dry terrain, acacia, dry grass | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 164 | `minecraft:shattered_savanna_plateau` | savanna-plateau | extreme dry plateau terrain | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 165 | `minecraft:eroded_badlands` | badlands | eroded terracotta pillars/red sand | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 166 | `minecraft:modified_wooded_badlands_plateau` | badlands | wooded badlands modified plateau | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 167 | `minecraft:modified_badlands_plateau` | badlands | modified badlands plateau | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 168 | `minecraft:bamboo_jungle` | jungle | bamboo-heavy jungle, pandas, vines | TBD | `[ ] B [ ] T [ ] S [ ] F` |
| 169 | `minecraft:bamboo_jungle_hills` | jungle | bamboo jungle hills | TBD | `[ ] B [ ] T [ ] S [ ] F` |

## Notes And Early Gaps

- `Biomes.java` includes nether, end, underground Caves & Cliffs Part 1, and
  void biomes. They are intentionally excluded here because this matrix targets
  vanilla 1.17.1 overworld palette.
- Bamboo jungle rows stay in the matrix because Java's
  `RareBiomeLargeLayer` / `RegionHillsLayer` can produce ids `168` and `169`.
  Native `possible_overworld_biomes()` currently filters those ids from that
  helper's iterator, so the first seed-finder pass should confirm whether that
  is only helper policy or a real coverage gap.
- A row should not become checked from a screenshot alone. The first acceptance
  signal is deterministic data: biome id/key, tint RGB, surface-family block
  facts, and visible feature-family block facts.
- Exact decorated block parity remains owned by `103`; this matrix can point a
  row at a `103` fixture once that biome needs mismatch-bucket work.

## First Slice

Landed:

- `mclone-worldgen::levelgen::tests::first_palette_matrix_rows_have_expected_biome_and_surface_family`
  records representative `(0,0)` seeds for plains, desert, swamp, dark forest,
  taiga, snowy tundra, badlands, and warm ocean.
- The worldgen probe asserts primary biome identity, block-position biome
  identity at the chunk center, and at least one visible `FEATURES` top-surface
  column in the row's expected surface family.
- `mclone-mesh::tint::tests::first_palette_matrix_tint_groups_match_java_visual_facts`
  asserts the first eight rows' grass, foliage, and water tint outputs through
  the shared tint resolver. The swamp grass value is the default radius-2 blend
  at the origin, `#647139`, not the direct light swamp color.

Feature-family probes are intentionally still unchecked. Add those after the
biome, tint, and visible-surface facts stay stable.

## Suggested Next Slice

Add the first `F` checks for the same eight rows without trying to reach exact
decorated chunk parity:

1. desert: cactus/dead-bush family presence,
2. taiga: spruce/fern family presence,
3. dark forest: dark-oak/mushroom family presence or documented current gap,
4. badlands: red-sand/terracotta/dead-bush family presence,
5. warm ocean: sand/water plus current coral/seagrass support or documented
   current gap.

Keep these as deterministic block-family probes first. Move any row that needs
mismatch-bucket-driven feature porting into `103`.
