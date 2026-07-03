# 135: Overworld Biome Palette Matrix

Status: active parent; 67 generated biome/tint/visible-surface probes, all
overworld tint IDs, and six supported feature-family probes landed, including
cactus/sugar-cane extra vegetation
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
| 0 | `minecraft:ocean` | ocean-default | water checked; default ocean seafloor, kelp/seagrass gap | seed `1`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 1 | `minecraft:plains` | plains | grass surface, grass/flower patches, oak vegetation | seed `16`, chunk `(0,0)`; seeds `16`, `17` exist in `103` | `[x] B [x] T [x] S [x] F` |
| 2 | `minecraft:desert` | desert | sand/sandstone, dead bush plus cactus/sugar-cane family checked; pumpkin/desert-well extras gap | seed `49`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 3 | `minecraft:mountains` | mountains | grass/stone/gravel mountain surface checked, sparse trees | seed `31`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 4 | `minecraft:forest` | forest | forest grass tint/surface checked, oak/birch trees/flowers | seed `0`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 5 | `minecraft:taiga` | taiga | spruce trees, ferns; berries gap | seed `125`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 6 | `minecraft:swamp` | swamp | swamp grass/water, native oak/grass/dead-bush/clay subset plus sugar cane; lily pads/seagrass gap | seed `88`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 7 | `minecraft:river` | river | river water checked, banks/seagrass gap | seed `39`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 10 | `minecraft:frozen_ocean` | frozen-water | frozen water/ice checked, icebergs/blue ice gap | seed `333`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 11 | `minecraft:frozen_river` | frozen-water | frozen river water/ice checked | seed `252`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 12 | `minecraft:snowy_tundra` | frozen-land | snow over grass, native snowy spruce/fern subset | seed `42`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 13 | `minecraft:snowy_mountains` | frozen-land | snowy mountain surface checked | seed `326`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 14 | `minecraft:mushroom_fields` | mushroom | mycelium checked; mushrooms/no-normal-hostile palette gap | seed `978`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 15 | `minecraft:mushroom_field_shore` | mushroom | mycelium shore transition checked | seed `1554`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 16 | `minecraft:beach` | plains | sand beach checked, buried-treasure/shipwreck surface context | seed `45`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 17 | `minecraft:desert_hills` | desert | sand/sandstone hills checked; extra vegetation unprobed | seed `120`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 18 | `minecraft:wooded_hills` | forest | forest hill grass checked; trees gap | seed `2`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 19 | `minecraft:taiga_hills` | taiga | taiga hill grass checked; spruce/fern gap | seed `29`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 20 | `minecraft:mountain_edge` | mountains | tint checked; Java 1.17 final layered source appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 21 | `minecraft:jungle` | jungle | grass tint/surface checked; jungle trees, vines, bamboo-light vegetation gap | seed `71`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 22 | `minecraft:jungle_hills` | jungle | jungle hill surface/tint checked; trees/vines gap | seed `146`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 23 | `minecraft:jungle_edge` | jungle-edge | grass tint/surface checked; jungle-edge lower-density trees gap | seed `2235`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 24 | `minecraft:deep_ocean` | ocean-default | deep water checked; kelp/seagrass gap | seed `4`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 25 | `minecraft:stone_shore` | mountains | stone shore checked, steep coast | seed `167`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 26 | `minecraft:snowy_beach` | snowy-beach | snowy sand beach/cold water checked | seed `330`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 27 | `minecraft:birch_forest` | birch | birch tint/grass surface checked, birch leaves/trunks | seed `10`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 28 | `minecraft:birch_forest_hills` | birch | birch hill surface/tint checked; trees gap | seed `30`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 29 | `minecraft:dark_forest` | dark-forest | dark grass modifier checked; dark oak/mushroom gap | seed `44`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 30 | `minecraft:snowy_taiga` | snowy-taiga | snowy surface/tint checked, spruce/ferns/berries | seed `14`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 31 | `minecraft:snowy_taiga_hills` | snowy-taiga | snowy taiga hill surface/tint checked; spruce gap | seed `886`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 32 | `minecraft:giant_tree_taiga` | giant-tree-taiga | podzol/coarse dirt surface checked, giant taiga trees/ferns | seed `19`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 33 | `minecraft:giant_tree_taiga_hills` | giant-tree-taiga | giant taiga hill podzol/coarse dirt checked; trees gap | seed `93`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 34 | `minecraft:wooded_mountains` | mountains | mountain surface checked; trees gap | seed `3`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 35 | `minecraft:savanna` | savanna | dry grass tint/surface checked; acacia/tall grass gap | seed `62`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 36 | `minecraft:savanna_plateau` | savanna-plateau | dry plateau tint/surface checked; acacia/grass gap | seed `126`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 37 | `minecraft:badlands` | badlands | red sand, terracotta bands, dead bush plus cactus/sugar-cane family checked; wooded variants gap | seed `2359`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 38 | `minecraft:wooded_badlands_plateau` | badlands | terracotta/red sand checked; wooded plateau trees gap | seed `86`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 39 | `minecraft:badlands_plateau` | badlands | plateau terracotta/red sand checked | seed `84`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 44 | `minecraft:warm_ocean` | ocean-warm | turquoise water/sand checked; coral/sea-pickle/seagrass gap | seed `26`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 45 | `minecraft:lukewarm_ocean` | ocean-lukewarm | bright water checked; sand, seagrass/kelp gap | seed `6`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 46 | `minecraft:cold_ocean` | ocean-cold | cold water checked; gravel/grass seafloor, kelp gap | seed `5`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 47 | `minecraft:deep_warm_ocean` | ocean-warm | tint checked; Java 1.17 final ocean mixer appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 48 | `minecraft:deep_lukewarm_ocean` | ocean-lukewarm | deep bright water checked; kelp gap | seed `56`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 49 | `minecraft:deep_cold_ocean` | ocean-cold | deep cold water checked; kelp gap | seed `13`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 50 | `minecraft:deep_frozen_ocean` | ocean-frozen-deep | deep frozen water/ice checked, icebergs gap | seed `103`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 129 | `minecraft:sunflower_plains` | plains | plains tint/surface checked; sunflower patches gap | seed `25`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 130 | `minecraft:desert_lakes` | desert | desert surface checked; lake/fossil/extra-vegetation gap | seed `98`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 131 | `minecraft:gravelly_mountains` | mountains | gravelly mountain surface checked | seed `212`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 132 | `minecraft:flower_forest` | forest | forest grass/tint checked; dense flower palette gap | seed `135`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 133 | `minecraft:taiga_mountains` | taiga | taiga mountain grass checked; spruce/fern mountain gap | seed `1326`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 134 | `minecraft:swamp_hills` | swamp | swamp tint/surface checked; hill fossil/vegetation gap | seed `1094`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 140 | `minecraft:ice_spikes` | frozen-land | snow/ice-spikes surface checked; spike feature gap | seed `59`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 149 | `minecraft:modified_jungle` | jungle | jungle tint/surface checked; dense jungle/vines gap | seed `1374`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 151 | `minecraft:modified_jungle_edge` | jungle-edge | jungle-edge tint/surface checked; vegetation gap | seed `314096`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 155 | `minecraft:tall_birch_forest` | birch | birch tint/surface checked; tall birch gap | seed `48`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 156 | `minecraft:tall_birch_hills` | birch | birch hill tint/surface checked; tall birch gap | seed `1557`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 157 | `minecraft:dark_forest_hills` | dark-forest | dark-forest tint/surface checked; canopy/mushrooms gap | seed `410`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 158 | `minecraft:snowy_taiga_mountains` | snowy-taiga | snowy taiga mountain tint/surface checked; spruce gap | seed `12006`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 160 | `minecraft:giant_spruce_taiga` | giant-spruce-taiga | podzol/coarse dirt surface checked, giant spruce gap | seed `2923`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 161 | `minecraft:giant_spruce_taiga_hills` | giant-spruce-taiga | giant spruce hill podzol/coarse dirt checked; tree gap | seed `282`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 162 | `minecraft:modified_gravelly_mountains` | mountains | modified gravelly mountain surface checked | seed `83`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 163 | `minecraft:shattered_savanna` | shattered-savanna | shattered grass/coarse-dirt/stone surface checked; acacia gap | seed `68`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 164 | `minecraft:shattered_savanna_plateau` | savanna-plateau | extreme dry plateau surface checked | seed `175`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 165 | `minecraft:eroded_badlands` | badlands | eroded terracotta/red-sand surface checked; pillar feature gap | seed `8464`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 166 | `minecraft:modified_wooded_badlands_plateau` | badlands | wooded badlands modified plateau surface checked; trees gap | seed `3823`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 167 | `minecraft:modified_badlands_plateau` | badlands | modified badlands plateau surface checked | seed `18441`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 168 | `minecraft:bamboo_jungle` | jungle | jungle tint/surface checked; bamboo-heavy vegetation gap | seed `626`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |
| 169 | `minecraft:bamboo_jungle_hills` | jungle | jungle hill tint/surface checked; bamboo vegetation gap | seed `1000`, chunk `(0,0)` | `[x] B [x] T [x] S [ ] F` |

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
  families for plains, desert, swamp, taiga, snowy tundra, and badlands. Desert
  and badlands now require dead bush plus cactus/sugar-cane family coverage;
  swamp requires the native vegetation/clay subset plus sugar cane. The
  assertions are intentionally broad block-family checks, not exact decorated
  counts.
- Native now has cactus and sugar cane generated block IDs, synthetic asset
  registry mappings, basic shared shape/material facts, Java-style reduced
  random-patch column placement, and desert/badlands/swamp extra-vegetation
  table entries.
- `mclone-mesh::tint::tests::palette_matrix_tint_groups_match_java_visual_facts`
  asserts every matrix row's grass, foliage, and water tint output through the
  shared tint resolver by tint group, including the two registered rows without
  B/S fixtures. The swamp grass value is the default radius-2 blend at the
  origin, `#647139`, not the direct light swamp color.

Documented gaps from this slice:

- Java default extra vegetation adds sugar cane to many other non-ocean
  overworld biomes; this slice only landed the high-signal
  desert/badlands/swamp extra-vegetation paths, and still omits pumpkin.
- Java dark forest uses dark oak plus huge mushroom selection; native currently
  falls through to the default land feature table because dark oak and mushroom
  feature families are not modeled.
- Java warm ocean uses seagrass, sea pickles, and coral; native does not yet
  have those block IDs/features.
- Java swamp includes water lilies, mushrooms, and swamp seagrass/extras; native
  currently checks only the oak/grass/dead-bush/clay subset that exists.

## Suggested Next Slice

Move from B/T/S coverage to visible feature-family breadth:

1. Pick one missing block/feature family with high palette value and port it
   narrowly from Java: water plants plus coral/sea pickles, dark oak plus huge
   mushrooms, acacia, bamboo, jungle vines, or berry bushes.
2. Add `F` checks to this matrix only when the supporting block IDs/features
   exist in native and the check is a broad deterministic block-family probe.
3. Move any exact decorated mismatch-bucket work into `103`.
