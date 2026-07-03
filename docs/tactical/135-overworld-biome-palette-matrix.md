# 135: Overworld Biome Palette Matrix

Status: active parent; 67 generated biome/tint/visible-surface probes, all
overworld tint IDs, fourteen supported feature-family groups, and 31 F-checked
matrix rows landed, including cactus/sugar-cane extras, swamp lily pads, ocean
water plants, warm-ocean coral/sea-pickle, dark-forest canopy/mushroom,
mushroom-field huge mushrooms, birch and tall-birch trees, savanna acacia,
jungle tree, and bamboo-jungle palette coverage
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
   - biome identity: primary biome lookup resolves the expected key and at
     least one block-position biome sample in the chunk resolves that key,
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
`F` currently supported visible palette feature family. `F` is not exact
decorated feature parity; rows can still list missing vanilla families in the
notes.

| ID | Biome | Tint group | Surface / palette focus | Fixture | Checks |
|---:|---|---|---|---|---|
| 0 | `minecraft:ocean` | ocean-default | water checked; default ocean seafloor, seagrass/kelp water plants checked | seed `1`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 1 | `minecraft:plains` | plains | grass surface, grass/flower patches, oak vegetation | seed `16`, chunk `(0,0)`; seeds `16`, `17` exist in `103` | `[x] B [x] T [x] S [x] F` |
| 2 | `minecraft:desert` | desert | sand/sandstone, dead bush plus cactus/sugar-cane family checked; pumpkin/desert-well extras gap | seed `49`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 3 | `minecraft:mountains` | mountains | grass/stone/gravel mountain surface checked, sparse trees | seed `31`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 4 | `minecraft:forest` | forest | forest grass tint/surface checked, oak/birch trees/flowers | seed `0`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 5 | `minecraft:taiga` | taiga | spruce trees, ferns; berries gap | seed `125`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 6 | `minecraft:swamp` | swamp | swamp grass/water, native oak/grass/dead-bush/clay subset plus sugar cane and lily pads; mushrooms/seagrass/pumpkin gap | seed `376`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 7 | `minecraft:river` | river | river water checked, banks/seagrass gap | seed `39`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 10 | `minecraft:frozen_ocean` | frozen-water | frozen water/ice checked, icebergs/blue ice gap | seed `333`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 11 | `minecraft:frozen_river` | frozen-water | frozen river water/ice checked | seed `252`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 12 | `minecraft:snowy_tundra` | frozen-land | snow over grass, native snowy spruce/fern subset | seed `42`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 13 | `minecraft:snowy_mountains` | frozen-land | snowy mountain surface checked | seed `326`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 14 | `minecraft:mushroom_fields` | mushroom | mycelium and huge mushroom family checked; small mushrooms/default extras/spawn-table gap | seed `978`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 15 | `minecraft:mushroom_field_shore` | mushroom | mycelium shore transition checked | seed `1554`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 16 | `minecraft:beach` | plains | sand beach checked, buried-treasure/shipwreck surface context | seed `45`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 17 | `minecraft:desert_hills` | desert | sand/sandstone hills checked; extra vegetation unprobed | seed `120`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 18 | `minecraft:wooded_hills` | forest | forest hill grass checked; trees gap | seed `2`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 19 | `minecraft:taiga_hills` | taiga | taiga hill grass checked; spruce/fern gap | seed `29`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 20 | `minecraft:mountain_edge` | mountains | tint checked; Java 1.17 final layered source appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 21 | `minecraft:jungle` | jungle | grass tint/surface and jungle log/leaves tree family checked; vines/cocoa gap | seed `71`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 22 | `minecraft:jungle_hills` | jungle | jungle hill surface/tint and jungle log/leaves tree family checked; vines/cocoa gap | seed `146`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 23 | `minecraft:jungle_edge` | jungle-edge | grass tint/surface and lower-density jungle log/leaves tree family checked; vines/cocoa gap | seed `2235`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 24 | `minecraft:deep_ocean` | ocean-default | deep water checked; seagrass/kelp water plants checked | seed `4`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 25 | `minecraft:stone_shore` | mountains | stone shore checked, steep coast | seed `167`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 26 | `minecraft:snowy_beach` | snowy-beach | snowy sand beach/cold water checked | seed `330`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 27 | `minecraft:birch_forest` | birch | birch tint/grass surface and birch log/leaves tree family checked | seed `10`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 28 | `minecraft:birch_forest_hills` | birch | birch hill surface/tint and birch log/leaves tree family checked | seed `30`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 29 | `minecraft:dark_forest` | dark-forest | dark grass modifier, dark oak canopy, and huge mushroom family checked | seed `44`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 30 | `minecraft:snowy_taiga` | snowy-taiga | snowy surface/tint checked, spruce/ferns/berries | seed `14`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 31 | `minecraft:snowy_taiga_hills` | snowy-taiga | snowy taiga hill surface/tint checked; spruce gap | seed `886`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 32 | `minecraft:giant_tree_taiga` | giant-tree-taiga | podzol/coarse dirt surface checked, giant taiga trees/ferns | seed `19`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 33 | `minecraft:giant_tree_taiga_hills` | giant-tree-taiga | giant taiga hill podzol/coarse dirt checked; trees gap | seed `93`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 34 | `minecraft:wooded_mountains` | mountains | mountain surface checked; trees gap | seed `3`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 35 | `minecraft:savanna` | savanna | dry grass tint/surface and acacia tree family checked; tall grass/warm flowers gap | seed `62`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 36 | `minecraft:savanna_plateau` | savanna-plateau | dry plateau tint/surface and acacia tree family checked; grass/warm flowers gap | seed `126`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 37 | `minecraft:badlands` | badlands | red sand, terracotta bands, dead bush plus cactus/sugar-cane family checked; wooded variants gap | seed `2359`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 38 | `minecraft:wooded_badlands_plateau` | badlands | terracotta/red sand checked; wooded plateau trees gap | seed `86`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 39 | `minecraft:badlands_plateau` | badlands | plateau terracotta/red sand checked | seed `84`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 44 | `minecraft:warm_ocean` | ocean-warm | turquoise water/sand, seagrass plus coral blocks/sea pickles checked; coral plants/fans gap | seed `2696`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 45 | `minecraft:lukewarm_ocean` | ocean-lukewarm | bright water/sand, seagrass/kelp water plants checked | seed `6`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 46 | `minecraft:cold_ocean` | ocean-cold | cold water checked; gravel/grass seafloor, seagrass/kelp checked | seed `5`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 47 | `minecraft:deep_warm_ocean` | ocean-warm | tint checked; Java 1.17 final ocean mixer appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 48 | `minecraft:deep_lukewarm_ocean` | ocean-lukewarm | deep bright water, seagrass/kelp water plants checked | seed `56`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 49 | `minecraft:deep_cold_ocean` | ocean-cold | deep cold water, seagrass/kelp water plants checked | seed `13`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 50 | `minecraft:deep_frozen_ocean` | ocean-frozen-deep | deep frozen water/ice checked, icebergs gap | seed `103`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 129 | `minecraft:sunflower_plains` | plains | plains tint/surface checked; sunflower patches gap | seed `25`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 130 | `minecraft:desert_lakes` | desert | desert surface checked; lake/fossil/extra-vegetation gap | seed `98`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 131 | `minecraft:gravelly_mountains` | mountains | gravelly mountain surface checked | seed `212`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 132 | `minecraft:flower_forest` | forest | forest grass/tint checked; dense flower palette gap | seed `135`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 133 | `minecraft:taiga_mountains` | taiga | taiga mountain grass checked; spruce/fern mountain gap | seed `1326`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 134 | `minecraft:swamp_hills` | swamp | swamp tint/surface checked; hill fossil/vegetation gap | seed `1094`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 140 | `minecraft:ice_spikes` | frozen-land | snow/ice-spikes surface checked; spike feature gap | seed `59`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 149 | `minecraft:modified_jungle` | jungle | jungle tint/surface and jungle log/leaves tree family checked; dense vines/cocoa gap | seed `1374`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 151 | `minecraft:modified_jungle_edge` | jungle-edge | jungle-edge tint/surface and jungle log/leaves tree family checked; vegetation gap | seed `314096`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 155 | `minecraft:tall_birch_forest` | birch | birch tint/surface and tall-birch selector log/leaves family checked | seed `48`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 156 | `minecraft:tall_birch_hills` | birch | birch hill tint/surface and tall-birch selector log/leaves family checked | seed `1557`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 157 | `minecraft:dark_forest_hills` | dark-forest | dark-forest tint/surface, dark oak canopy, and huge mushroom family checked | seed `410`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 158 | `minecraft:snowy_taiga_mountains` | snowy-taiga | snowy taiga mountain tint/surface checked; spruce gap | seed `12006`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 160 | `minecraft:giant_spruce_taiga` | giant-spruce-taiga | podzol/coarse dirt surface checked, giant spruce gap | seed `2923`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 161 | `minecraft:giant_spruce_taiga_hills` | giant-spruce-taiga | giant spruce hill podzol/coarse dirt checked; tree gap | seed `282`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 162 | `minecraft:modified_gravelly_mountains` | mountains | modified gravelly mountain surface checked | seed `83`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 163 | `minecraft:shattered_savanna` | shattered-savanna | shattered grass/coarse-dirt/stone surface and acacia tree family checked | seed `68`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 164 | `minecraft:shattered_savanna_plateau` | savanna-plateau | extreme dry plateau surface and acacia tree family checked | seed `2659`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 165 | `minecraft:eroded_badlands` | badlands | eroded terracotta/red-sand surface checked; pillar feature gap | seed `8464`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 166 | `minecraft:modified_wooded_badlands_plateau` | badlands | wooded badlands modified plateau surface checked; trees gap | seed `3823`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 167 | `minecraft:modified_badlands_plateau` | badlands | modified badlands plateau surface checked | seed `18441`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 168 | `minecraft:bamboo_jungle` | jungle | jungle tint/surface, bamboo stalks, and jungle log/leaves vegetation checked; vines/cocoa/top bamboo leaf states gap | seed `1263`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 169 | `minecraft:bamboo_jungle_hills` | jungle | jungle hill tint/surface, bamboo stalks, and jungle log/leaves vegetation checked; vines/cocoa/top bamboo leaf states gap | seed `1000`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |

## Notes And Early Gaps

- `Biomes.java` includes nether, end, underground Caves & Cliffs Part 1, and
  void biomes. They are intentionally excluded here because this matrix targets
  vanilla 1.17.1 overworld palette.
- Bamboo jungle rows stay in the matrix because Java's
  `RareBiomeLargeLayer` / `RegionHillsLayer` can produce ids `168` and `169`.
  Native seed search now has fixtures for both rows; `possible_overworld_biomes()`
  still omits them from `--list-biomes`, so that helper list is incomplete.
- `minecraft:mountain_edge` and `minecraft:deep_warm_ocean` are registered
  Java 1.17.1 biomes with tint facts, but the default final layered source
  appears not to emit them. `BiomeEdgeLayer.checkEdge` preserves mountains
  instead of returning id `20`, and `OceanMixerLayer` can return deep lukewarm,
  deep cold, deep frozen, or deep ocean from deep ocean land cells but never
  id `47`.
- A row should not become checked from a screenshot alone. The first acceptance
  signal is deterministic data: biome id/key, tint RGB, surface-family block
  facts, and visible feature-family block facts.
- Exact decorated block parity remains owned by `103`; this matrix can point a
  row at a `103` fixture once that biome needs mismatch-bucket work.

## Landed Slices

Landed:

- `mclone-worldgen::levelgen::tests::palette_matrix_rows_have_expected_biome_surface_and_supported_feature_family`
  records representative `(0,0)` seeds for all 67 rows that the native/default
  Java 1.17.1 layered source currently emits.
- The worldgen probe asserts primary biome identity, block-position biome
  identity somewhere in the chunk, and at least one visible `FEATURES`
  top-surface column in the row's expected surface family.
- The same worldgen probe now asserts currently supported visible feature
  families for plains, desert, swamp, taiga, snowy tundra, badlands, and ocean
  water plants, warm-ocean coral blocks and sea pickles, dark-forest dark oak
  and huge mushroom block families, mushroom-field huge mushrooms, birch
  log/leaves trees, savanna acacia trees, jungle log/leaves trees, and
  bamboo-jungle bamboo plus jungle log/leaves vegetation. Desert and badlands
  now require dead bush plus cactus/sugar-cane family coverage; swamp requires
  the native vegetation/clay subset plus sugar cane and lily pads; generated
  non-frozen ocean rows require the seagrass/tall-seagrass/kelp block family;
  the warm-ocean row requires at least one live coral block and one sea pickle
  state; dark forest rows require both dark oak logs/leaves and huge mushroom
  cap/stem blocks; mushroom fields require a huge mushroom cap plus stem; birch
  rows require birch logs and leaves; savanna rows require acacia logs and
  leaves; jungle rows require jungle logs and leaves; bamboo jungle rows require
  bamboo plus jungle logs and leaves. The assertions are intentionally broad
  block-family checks, not exact decorated counts.
- Native now has cactus, sugar cane, seagrass, tall seagrass, kelp, and kelp
  plant generated block IDs; live coral block IDs; four waterlogged sea-pickle
  state IDs; lily-pad ID with Java `BlockColors` hardcoded tint; dark oak
  log/leaves IDs; huge mushroom cap/stem IDs; acacia log/leaves IDs; jungle
  log/leaves IDs; bamboo trunk ID; synthetic asset registry mappings; basic shared
  shape/material/render/light facts; Java-style reduced random-patch column
  placement; Java-style waterlily random-patch placement; Java-style
  `nextBoolean` random-boolean selector placement for mushroom fields;
  Java-style seagrass, kelp, sea-pickle, and broad coral tree/claw/mushroom
  placement; broad Java-shaped dark oak tree, huge mushroom, birch and
  tall-birch tree table placement, acacia forking-trunk/flat-canopy placement,
  jungle tree/bush/mega-jungle selector placement, and bamboo
  column/podzol-disk placement; the
  `NoiseBasedDecorator` count path used by kelp/coral/bamboo; and
  desert/badlands/swamp/ocean/dark-forest/birch/savanna/jungle/bamboo-jungle
  feature table entries, plus the mushroom-field huge mushroom table entry.
- `mclone-mesh::tint::tests::palette_matrix_tint_groups_match_java_visual_facts`
  asserts every matrix row's grass, foliage, and water tint output through the
  shared tint resolver by tint group, including the two registered rows without
  B/S fixtures. The swamp grass value is the default radius-2 blend at the
  origin, `#647139`, not the direct light swamp color.

Documented gaps from this slice:

- Java default extra vegetation adds sugar cane to many other non-ocean
  overworld biomes; this slice only landed the high-signal
  desert/badlands/swamp extra-vegetation paths, and still omits pumpkin.
- Java dark forest now has the high-signal dark oak plus huge mushroom selector
  path represented, but exact parity is still incomplete: dark oak still uses a
  reduced `ThreeLayersFeatureSize` free-space approximation, huge mushrooms do
  not yet model directional cap/stem side-state booleans, and small mushroom /
  extra forest vegetation patches remain omitted.
- Java savanna now has the high-signal acacia selector path represented, but
  exact decorated parity is still incomplete: warm flower selection, normal vs
  shattered grass density, default extra vegetation, villages/outposts, and
  exact tree-count mismatch buckets remain owned by later `103` work.
- Java jungle and bamboo jungle now have high-signal jungle log/leaves and
  bamboo stalk coverage. Exact parity is still incomplete: mega jungle trees
  use a reduced straight-trunk/blob-foliage approximation instead of the 2x2
  trunk/branch/mega foliage placers, jungle bushes use the existing blob
  foliage approximation, cocoa/vines are not generated, bamboo top leaf states
  are not separate block IDs yet, and warm flower / jungle extra vegetation
  parity remains later `103` work.
- Java normal/cold/lukewarm ocean water-plant tables are represented by broad
  seagrass/kelp checks, and warm ocean now has seagrass, live coral blocks, and
  sea pickles. Native still omits the `SEAGRASS_SIMPLE` carving-mask decorator
  path, coral plants, coral fans/wall fans, exact coral mismatch parity, and
  full waterlogged/fluid-state modeling for water plants in the raw generated
  block lane.
- Java swamp now has high-signal water-lily coverage. Exact parity is still
  incomplete: brown/red mushroom patches, swamp seagrass/extras, pumpkin, and
  exact decorated counts remain later `103` work.
- Java mushroom fields now have high-signal huge mushroom coverage. Exact
  parity is still incomplete: small brown/red mushroom patches, default mushroom
  patches, default extra vegetation, exact huge mushroom side-state booleans,
  mushroom-field shore feature coverage, and spawn-table/no-normal-hostile
  behavior remain later `103` or entity-runtime work.
- Java birch forests now have high-signal birch log/leaves coverage, including
  separate normal birch and tall-birch table shapes. Exact parity is still
  incomplete: tree counts, bee-nest side effects, flower/grass/default extra
  vegetation counts, and exact decorated mismatch buckets remain later `103`
  work.

## Suggested Next Slice

Move from B/T/S coverage to visible land feature-family breadth:

1. Pick one missing block/feature family with high palette value and port it
   narrowly from Java. The strongest next row is now berry bushes for taiga:
   the taiga rows already have spruce/fern coverage, and sweet berry bushes add
   a distinct low vegetation block family visible across normal and snowy taiga
   variants. A lower-risk alternative is adding F checks for ordinary forest and
   wooded-hills trees using the existing oak/birch selector support.
2. Add `F` checks to this matrix only when the supporting block IDs/features
   exist in native and the check is a broad deterministic block-family probe.
3. Move any exact decorated mismatch-bucket work into `103`.
