# 135: Overworld Biome Palette Matrix

Status: active parent; 68 overworld matrix rows, 66 emitted-row
biome/tint/visible-surface probes, all overworld tint IDs, twenty-four
supported feature-family groups, and all 66 emitted rows F-checked, including
cactus/sugar-cane extras, swamp/swamp-hills lily pads, blue orchids, sugar
cane, and small mushrooms, ocean water plants, warm-ocean coral blocks, live
plant/fan sidecars, and sea pickles, river seagrass,
frozen-river/beach/shore sugar cane, frozen-ocean blue ice,
ice-spikes packed ice,
dark-forest canopy/mushroom,
mushroom-field/shore huge mushrooms, birch and tall-birch trees, savanna acacia,
savanna tall grass/warm flowers/grass-density splits, jungle tree, bamboo-jungle,
taiga/snowy-taiga spruce/fern/berry, ordinary forest tree/flower/grass plus
mountain oak/spruce trees, snowy-mountain spruce/fern,
badlands-variant Java grass/dead-bush/cactus/sugar-cane plus wooded-only oak
trees, desert-variant dead bush/cactus/sugar-cane,
giant-taiga mega spruce/pine plus forest-rock mossy-cobblestone boulders, flower-forest
dense/common flower, sunflower-plains sunflower palette coverage, and
Java-shaped `PATCH_SUGAR_CANE` / `PATCH_PUMPKIN` default-extra table wiring
for the current main land builders, Java-shaped `SPRING_WATER` /
`SPRING_LAVA` default spring table wiring for the current land lanes, plus
Java-shaped `PATCH_PUMPKIN` table wiring for the currently modeled
desert/badlands/swamp lanes, plus Java-shaped
`BROWN_MUSHROOM_NORMAL` / `RED_MUSHROOM_NORMAL` table wiring for the current
forest, swamp, river/beach/shore, dark-forest, savanna, jungle,
bamboo-jungle, plains/sunflower-plains, birch/tall-birch, taiga/snowy-taiga,
giant-taiga, snowy, mountain, mushroom-field, and fallback land builders,
Java-shaped `BROWN_MUSHROOM_TAIGA` / `RED_MUSHROOM_TAIGA` and counted
giant-taiga mushroom table wiring, with Java-shaped small-mushroom
survival/light/substrate gating, and deterministic low-visibility fixtures for
plains/forest/birch/dark-forest/taiga/snowy/river/beach-shore/swamp,
mushroom-field/mushroom-shore/giant-taiga/mountain/savanna/jungle/badlands, and
sunflower-plains/ice-spikes small mushrooms plus savanna/sunflower-plains
exposed default spring water/lava placements, visible pumpkins on grass across
swamp/stone-shore, river/beach, desert/badlands, swamp-hills, and
sunflower-plains fixture rows including sparse snowy-beach/desert-lakes/modified-badlands
boundary cases, giant-taiga forest-rock mossy boulders, plus Java-shaped jungle-family `PATCH_MELON` / `VINES`
table slots, directional vine state/rendering, jungle tree cocoa, and
normal/mega jungle tree trunk/leaf vine decorators, mega-jungle 2x2
trunk/branch/foliage placer shape, Java-shaped jungle-bush foliage rows, plus
bamboo top leaf block states and multipart stem/leaf rendering,
plus Java-shaped `FOREST_FLOWER_VEGETATION`, `FOREST_FLOWER_VEGETATION_COMMON`,
`PATCH_GRASS_FOREST`, `FLOWER_WARM`, `PATCH_TALL_GRASS`,
`PATCH_GRASS_SAVANNA`, shattered-savanna `PATCH_GRASS_NORMAL`, badlands
`PATCH_GRASS_BADLANDS`, `PATCH_DEAD_BUSH_BADLANDS`, and wooded-only
`TREES_BADLANDS` table slots, plus an exact Java surface oracle for tall eroded
badlands pillar columns at seed `868` chunk `(8,-6)`.
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
| 1 | `minecraft:plains` | plains | grass surface, grass/flower patches, oak vegetation; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at chunk `(1,-1)` | seed `16`, chunk `(0,0)`; seeds `16`, `17` exist in `103` | `[x] B [x] T [x] S [x] F` |
| 2 | `minecraft:desert` | desert | sand/sandstone, dead bush plus cactus/sugar-cane family checked; `PATCH_PUMPKIN` visible pumpkin fixture at seed `327` chunk `(-6,6)`; spring table slots wired; desert-well extras gap | seed `258`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 3 | `minecraft:mountains` | mountains | grass/stone/gravel mountain surface and sparse oak/spruce tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `3` chunk `(-11,4)`; emerald/infested-stone gap | seed `33`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 4 | `minecraft:forest` | forest | forest grass tint/surface and oak/birch tree family checked; `FOREST_FLOWER_VEGETATION`, default flowers, count-2 forest grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `1` chunk `(0,-7)`; bee-side-effects gap | seed `0`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 5 | `minecraft:taiga` | taiga | spruce trees, ferns, and sweet berry bushes checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at chunk `(-1,0)` | seed `233`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 6 | `minecraft:swamp` | swamp | swamp grass/water, native oak/grass/dead-bush/clay subset plus blue orchids, mushroom blocks, sugar cane, and lily pads; small/normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `211` chunk `(-2,1)`; visible small-mushroom fixture at seed `3` chunk `(-16,-3)`; seagrass gap | seed `18918`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 7 | `minecraft:river` | river | river water plus `SEAGRASS_RIVER` seagrass/tall-seagrass water plants checked; native water-tree/default-vegetation subset present with normal mushroom, default extra, and spring table slots wired; visible pumpkin fixture at seed `58` chunk `(2,1)`; visible small-mushroom fixture at seed `1` chunk `(8,-13)`; bank-shape gap | seed `39`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 10 | `minecraft:frozen_ocean` | frozen-water | frozen water/ice plus packed/blue iceberg and blue-ice spread coverage checked; structures/exact iceberg gap | seed `779`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 11 | `minecraft:frozen_river` | frozen-water | frozen river water/ice plus default sugar-cane extras checked; normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `326` chunk `(-4,6)`; visible small-mushroom fixture at seed `66` chunk `(-11,-1)` | seed `252`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 12 | `minecraft:snowy_tundra` | frozen-land | snow over grass, native snowy spruce/fern subset; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `14` chunk `(-10,-12)` | seed `42`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 13 | `minecraft:snowy_mountains` | frozen-land | snowy mountain surface plus native snowy spruce/fern subset checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `42` chunk `(-11,-6)`; Java default grass mismatch gap | seed `326`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 14 | `minecraft:mushroom_fields` | mushroom | mycelium and huge mushroom family checked; visible small mushrooms checked; taiga-style/normal mushroom, default extra, and spring table slots wired; spawn-table gap | seed `978`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 15 | `minecraft:mushroom_field_shore` | mushroom | mycelium shore transition plus huge mushroom family checked; taiga-style/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `74` chunk `(-7,-12)`; spawn-table gap | seed `7056`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 16 | `minecraft:beach` | plains | sand beach plus default sugar-cane extras checked; normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `349` chunk `(-7,-5)`; visible small-mushroom fixture at seed `1` chunk `(-14,-11)`; buried-treasure/shipwreck gap | seed `1941`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 17 | `minecraft:desert_hills` | desert | sand/sandstone hills plus dead bush and cactus/sugar-cane family checked; `PATCH_PUMPKIN` visible pumpkin fixture at seed `2768` chunk `(-5,-7)`; spring table slots wired; structures/fossils gap | seed `348`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 18 | `minecraft:wooded_hills` | forest | forest hill grass and oak/birch tree family checked; `FOREST_FLOWER_VEGETATION`, default flowers, count-2 forest grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `1` chunk `(-1,-11)`; bee-side-effects gap | seed `2`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 19 | `minecraft:taiga_hills` | taiga | taiga hill grass plus spruce/fern and sweet berry bushes checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `2` chunk `(-12,12)` | seed `29`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 20 | `minecraft:mountain_edge` | mountains | tint checked; Java 1.17 final layered source appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 21 | `minecraft:jungle` | jungle | grass tint/surface, jungle log/leaves tree family, visible cocoa/directional vine states, and visible Java-shaped jungle-bush skirt checked; normal mushroom, default extra, spring, and jungle `PATCH_MELON`/`VINES` table slots wired; visible small-mushroom fixture at seed `1` chunk `(12,1)` | seed `61`, chunk `(1,2)` | `[x] B [x] T [x] S [x] F` |
| 22 | `minecraft:jungle_hills` | jungle | jungle hill surface/tint and jungle log/leaves tree family checked; normal/mega jungle tree vine decorators, mega-jungle 2x2 trunk/branch/foliage shape, jungle-bush foliage rows, and normal jungle cocoa decorator wired; normal mushroom, default extra, spring, and jungle `PATCH_MELON`/`VINES` table slots wired; visible small-mushroom fixture at seed `1` chunk `(15,2)`; exact tree-count gap | seed `146`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 23 | `minecraft:jungle_edge` | jungle-edge | grass tint/surface and lower-density jungle log/leaves tree family checked; normal jungle cocoa/directional-vine decorator path and jungle-bush foliage rows wired; normal mushroom, default extra, spring, and jungle `PATCH_MELON`/`VINES` table slots wired; visible small-mushroom fixture at seed `149` chunk `(4,13)`; exact low-density tree-count gap | seed `2235`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 24 | `minecraft:deep_ocean` | ocean-default | deep water checked; seagrass/kelp water plants checked | seed `4`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 25 | `minecraft:stone_shore` | mountains | stone shore plus default sugar-cane extras checked; normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `74739` chunk `(3,5)`; visible small-mushroom fixture at seed `0` chunk `(8,16)`; steep coast/structure-context gap | seed `74739`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 26 | `minecraft:snowy_beach` | snowy-beach | snowy sand beach/cold water plus default sugar-cane extras checked; normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `2805` chunk `(14,6)`; visible small-mushroom fixture at seed `54` chunk `(8,4)` | seed `5006`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 27 | `minecraft:birch_forest` | birch | birch tint/grass surface and birch log/leaves tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `7` chunk `(-5,-5)` | seed `10`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 28 | `minecraft:birch_forest_hills` | birch | birch hill surface/tint and birch log/leaves tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `7` chunk `(-9,-12)` | seed `30`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 29 | `minecraft:dark_forest` | dark-forest | dark grass modifier, dark oak canopy, and huge mushroom family checked; forest-flower vegetation, glow lichen, count-2 forest grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `0` chunk `(-7,8)` | seed `44`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 30 | `minecraft:snowy_taiga` | snowy-taiga | snowy surface/tint, spruce/ferns, and sweet berry bushes checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `14` chunk `(-12,10)` | seed `25122`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 31 | `minecraft:snowy_taiga_hills` | snowy-taiga | snowy taiga hill surface/tint plus spruce/fern and sweet berry bushes checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `14` chunk `(-12,-7)` | seed `18930`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 32 | `minecraft:giant_tree_taiga` | giant-tree-taiga | podzol/coarse dirt surface plus giant spruce/mega pine log/leaves family checked; `FOREST_ROCK` mossy-cobblestone boulders fixture at origin; counted giant-taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `19` chunk `(-12,-12)` | seed `132`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 33 | `minecraft:giant_tree_taiga_hills` | giant-tree-taiga | giant taiga hill podzol/coarse dirt plus giant spruce/mega pine log/leaves family checked; `FOREST_ROCK` table slot wired; counted giant-taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `19` chunk `(-12,-7)` | seed `305`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 34 | `minecraft:wooded_mountains` | mountains | mountain surface and wooded mountain oak/spruce tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `3` chunk `(0,-1)`; emerald/infested-stone gap | seed `58`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 35 | `minecraft:savanna` | savanna | dry grass tint/surface and acacia tree family checked; `PATCH_TALL_GRASS`, `FLOWER_WARM`, count-20 savanna grass, normal mushroom, default extra, and spring table slots wired; exposed default spring water/lava fixture at seed `62` chunk `(3,1)`; visible small-mushroom fixture at seed `33` chunk `(-10,-14)` | seed `33`, chunk `(-2,-4)` | `[x] B [x] T [x] S [x] F` |
| 36 | `minecraft:savanna_plateau` | savanna-plateau | dry plateau tint/surface and acacia tree family checked; `PATCH_TALL_GRASS`, `FLOWER_WARM`, count-20 savanna grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `38` chunk `(-3,-15)` | seed `126`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 37 | `minecraft:badlands` | badlands | red sand, terracotta bands, Java `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `2336` chunk `(-5,-7)`; visible small-mushroom fixture at seed `0` chunk `(-10,-16)` | seed `28`, chunk `(-2,-8)` | `[x] B [x] T [x] S [x] F` |
| 38 | `minecraft:wooded_badlands_plateau` | badlands | wooded plateau terracotta/red sand plus Java `TREES_BADLANDS` oak, `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `1248` chunk `(-6,8)`; visible small-mushroom fixture at seed `51` chunk `(12,-16)` | seed `4764`, chunk `(6,-3)` | `[x] B [x] T [x] S [x] F` |
| 39 | `minecraft:badlands_plateau` | badlands | plateau terracotta/red sand plus Java `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `653` chunk `(-5,6)`; visible small-mushroom fixture at seed `28` chunk `(-2,-16)` | seed `947`, chunk `(-6,-2)` | `[x] B [x] T [x] S [x] F` |
| 44 | `minecraft:warm_ocean` | ocean-warm | turquoise water/sand, seagrass plus coral blocks, live coral plants/floor fans/wall fans, and sea pickles checked | seed `2696`, chunk `(-14,-14)` | `[x] B [x] T [x] S [x] F` |
| 45 | `minecraft:lukewarm_ocean` | ocean-lukewarm | bright water/sand, seagrass/kelp water plants checked | seed `6`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 46 | `minecraft:cold_ocean` | ocean-cold | cold water checked; gravel/grass seafloor, seagrass/kelp checked | seed `5`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 47 | `minecraft:deep_warm_ocean` | ocean-warm | tint checked; Java 1.17 final ocean mixer appears not to emit this registered ID | no B/S fixture | `[ ] B [x] T [ ] S [ ] F` |
| 48 | `minecraft:deep_lukewarm_ocean` | ocean-lukewarm | deep bright water, seagrass/kelp water plants checked | seed `56`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 49 | `minecraft:deep_cold_ocean` | ocean-cold | deep cold water, seagrass/kelp water plants checked | seed `13`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 50 | `minecraft:deep_frozen_ocean` | ocean-frozen-deep | deep frozen water/ice plus packed/blue iceberg and blue-ice spread coverage checked; monuments/structures/exact iceberg gap | seed `1679`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 129 | `minecraft:sunflower_plains` | plains | plains tint/surface and sunflower patches checked; Java-order sugar-cane, normal mushroom, pumpkin, and spring table slots wired; exposed default spring water/lava fixture at seed `43` chunk `(-1,1)`; visible pumpkin fixture at seed `25` chunk `(-8,-2)`; visible small-mushroom fixture at seed `25` chunk `(-3,7)` | seed `43`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 130 | `minecraft:desert_lakes` | desert | desert surface plus dead bush and cactus/sugar-cane family checked; `PATCH_PUMPKIN` visible pumpkin fixture at seed `5012` chunk `(13,-10)`; spring table slots wired; lake/fossil gap | seed `98`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 131 | `minecraft:gravelly_mountains` | mountains | gravelly mountain surface plus sparse oak/spruce tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `123` chunk `(-4,10)`; emerald/infested-stone gap | seed `250`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 132 | `minecraft:flower_forest` | forest | forest grass/tint, dense small flowers, and common tall flowers checked; Java default grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `20` chunk `(11,0)` | seed `135`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 133 | `minecraft:taiga_mountains` | taiga | taiga mountain grass plus spruce/fern and sweet berry bushes checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `201` chunk `(1,-6)` | seed `6126`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 134 | `minecraft:swamp_hills` | swamp | swamp-hills tint/surface plus native swamp subset, blue orchids, mushroom blocks, sugar cane, and lily pads checked; small/normal mushroom, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `3407` chunk `(7,-2)`; visible small-mushroom fixture at seed `92` chunk `(-5,13)`; hill fossil/seagrass gap | seed `89335`, chunk `(5,4)` | `[x] B [x] T [x] S [x] F` |
| 140 | `minecraft:ice_spikes` | frozen-land | snow/ice-spikes surface plus packed-ice spike/patch coverage checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `59` chunk `(-12,-11)`; exact spike/patch shape gap | seed `59`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 149 | `minecraft:modified_jungle` | jungle | jungle tint/surface and jungle log/leaves tree family checked; normal/mega jungle tree vine decorators, mega-jungle 2x2 trunk/branch/foliage shape, jungle-bush foliage rows, and normal jungle cocoa decorator wired; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `102` chunk `(-13,-12)`; exact dense-jungle tree-count gap | seed `1374`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 151 | `minecraft:modified_jungle_edge` | jungle-edge | jungle-edge tint/surface and sparse jungle tree block spillover checked; normal jungle cocoa/directional-vine decorator path and jungle-bush foliage rows wired; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `8201` chunk `(6,-8)`; exact low-density tree-count gap | seed `314096`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 155 | `minecraft:tall_birch_forest` | birch | birch tint/surface and tall-birch selector log/leaves family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `39` chunk `(5,-12)` | seed `48`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 156 | `minecraft:tall_birch_hills` | birch | birch hill tint/surface and tall-birch selector log/leaves family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `32` chunk `(-6,4)` | seed `1557`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 157 | `minecraft:dark_forest_hills` | dark-forest | dark-forest tint/surface, dark oak canopy, and huge mushroom family checked; forest-flower vegetation, glow lichen, count-2 forest grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `123` chunk `(1,7)` | seed `410`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 158 | `minecraft:snowy_taiga_mountains` | snowy-taiga | snowy taiga mountain tint/surface plus spruce/fern checked; taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `14` chunk `(-5,10)`; visible berry gap | seed `12006`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 160 | `minecraft:giant_spruce_taiga` | giant-spruce-taiga | podzol/coarse dirt surface plus giant spruce log/leaves family checked; `FOREST_ROCK` table slot wired; counted giant-taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `132` chunk `(10,0)` | seed `6232`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 161 | `minecraft:giant_spruce_taiga_hills` | giant-spruce-taiga | giant spruce hill podzol/coarse dirt plus giant spruce log/leaves family checked; `FOREST_ROCK` table slot wired; counted giant-taiga/normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `93` chunk `(-11,-10)` | seed `282`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 162 | `minecraft:modified_gravelly_mountains` | mountains | modified gravelly mountain surface plus sparse oak/spruce tree family checked; normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `9` chunk `(9,9)`; emerald/infested-stone gap | seed `1831`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 163 | `minecraft:shattered_savanna` | shattered-savanna | shattered grass/coarse-dirt/stone surface and acacia tree family checked; default flowers, count-5 shattered grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `68` chunk `(-6,3)` | seed `68`, chunk `(-6,0)` | `[x] B [x] T [x] S [x] F` |
| 164 | `minecraft:shattered_savanna_plateau` | savanna-plateau | extreme dry plateau surface and acacia tree family checked; default flowers, count-5 shattered grass, normal mushroom, default extra, and spring table slots wired; visible small-mushroom fixture at seed `126` chunk `(13,14)` | seed `153`, chunk `(-8,-2)` | `[x] B [x] T [x] S [x] F` |
| 165 | `minecraft:eroded_badlands` | badlands | eroded terracotta/red-sand surface plus exact Java tall-pillar surface oracle at chunk `(8,-6)`; Java `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `85720` chunk `(11,5)`; visible small-mushroom fixture at seed `86` chunk `(-16,-2)` | seed `868`, chunk `(7,-8)`; pillar oracle chunk `(8,-6)` | `[x] B [x] T [x] S [x] F` |
| 166 | `minecraft:modified_wooded_badlands_plateau` | badlands | modified wooded plateau surface plus Java `TREES_BADLANDS` oak, `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `13518` chunk `(10,-3)`; visible small-mushroom fixture at seed `84` chunk `(-13,-2)` | seed `12115`, chunk `(0,-1)` | `[x] B [x] T [x] S [x] F` |
| 167 | `minecraft:modified_badlands_plateau` | badlands | modified badlands plateau plus Java `PATCH_GRASS_BADLANDS`, count-20 dead bush, normal mushroom, badlands sugar-cane/cactus, `PATCH_PUMPKIN`, and spring table slots wired; visible pumpkin fixture at seed `168856` chunk `(4,10)`; visible small-mushroom fixture at seed `1150` chunk `(-15,13)` | seed `1150`, chunk `(-7,1)` | `[x] B [x] T [x] S [x] F` |
| 168 | `minecraft:bamboo_jungle` | jungle | jungle tint/surface, bamboo stalks/top leaf states, jungle log/leaves vegetation, mega-jungle 2x2 trunk/branch/foliage, jungle-bush foliage rows, and vine decorator path checked; normal mushroom, default extra, spring, and jungle `PATCH_MELON`/`VINES` table slots wired; visible small-mushroom fixture at seed `71` chunk `(9,0)`; exact tree-count gap | seed `1263`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |
| 169 | `minecraft:bamboo_jungle_hills` | jungle | jungle hill tint/surface, bamboo stalks/top leaf states, jungle log/leaves vegetation, mega-jungle 2x2 trunk/branch/foliage, jungle-bush foliage rows, and vine decorator path checked; normal mushroom, default extra, spring, and jungle `PATCH_MELON`/`VINES` table slots wired; visible small-mushroom fixture at seed `102` chunk `(-1,-16)`; exact tree-count gap | seed `1000`, chunk `(0,0)` | `[x] B [x] T [x] S [x] F` |

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
  records representative `(0,0)` seeds for all 66 rows that the native/default
  Java 1.17.1 layered source currently emits.
- The worldgen probe asserts primary biome identity, block-position biome
  identity somewhere in the chunk, and at least one visible `FEATURES`
  top-surface column in the row's expected surface family.
- The same worldgen probe now asserts currently supported visible feature
  families for plains, desert, ordinary forest, swamp, base taiga, snowy
  tundra, base snowy taiga, badlands, ice-spikes packed ice, and ocean water
  plants, warm-ocean coral blocks, live coral plant/floor-fan/wall-fan
  sidecars, and sea pickles, dark-forest dark oak and
  huge mushroom block families, mushroom-field/shore huge mushrooms, birch
  log/leaves trees, savanna acacia trees, jungle log/leaves trees, and
  bamboo-jungle bamboo plus jungle log/leaves vegetation, flower-forest dense
  and common flowers, and sunflower-plains sunflower patches. Desert, desert
  hills, desert lakes, and
  non-wooded generated badlands rows now require dead bush plus cactus/sugar-cane
  family coverage; wooded badlands rows require oak logs/leaves plus dead bush
  and cactus/sugar-cane family coverage; ordinary forest and wooded hills require an oak or birch
  log/leaves tree pair; swamp and swamp hills require the native
  vegetation/clay subset plus blue orchids, small mushrooms, sugar cane, and
  lily pads; taiga, taiga hills, taiga mountains, base snowy taiga, and snowy
  taiga hills require spruce/fern vegetation plus sweet berry bushes; snowy
  mountains and snowy taiga mountains currently require the snowy spruce/fern family; generated
  non-frozen ocean rows require the seagrass/tall-seagrass/kelp block family;
  river requires the `SEAGRASS_RIVER` seagrass/tall-seagrass block family;
  frozen river, beach, stone shore, and snowy beach require default sugar-cane
  extra coverage; frozen-ocean rows require packed ice plus blue ice from the
  iceberg/blue-ice feature table;
  the warm-ocean row requires at least one live coral block, one live coral
  plant, one floor coral fan, one wall coral fan, and one sea pickle state;
  dark forest rows require both dark oak logs/leaves and huge mushroom
  cap/stem blocks; mushroom field rows require a huge mushroom cap plus stem;
  birch rows require birch logs and leaves; savanna rows require acacia logs and
  leaves; jungle rows require jungle logs and leaves; bamboo jungle rows require
  bamboo plus jungle logs and leaves; giant taiga rows require a high-signal
  spruce log/leaves family with podzol from the Java mega spruce/mega pine
  alter-ground path; mountain rows require an oak or spruce log/leaves tree
  pair; flower forest requires at least four distinct Java
  `ForestFlowerProvider` small-flower states and one
  `FOREST_FLOWER_VEGETATION_COMMON` tall-flower lower/upper pair; sunflower
  plains requires a sunflower lower/upper pair; ice spikes requires packed-ice
  spike/patch coverage. The assertions are intentionally broad block-family
  checks, not exact decorated counts.
- Native now has cactus, sugar cane, seagrass, tall seagrass, kelp, and kelp
  plant generated block IDs; live coral block IDs; four waterlogged sea-pickle
  state IDs; blue-ice generated block ID;
  lily-pad ID with Java `BlockColors` hardcoded tint; blue orchid
  ID; brown/red small mushroom IDs; dark oak log/leaves IDs; huge mushroom
  cap/stem IDs; acacia log/leaves IDs; jungle
  log/leaves IDs; bamboo trunk ID; sweet berry bush age-3 ID; allium,
  azure bluet, red/orange/white/pink tulips, oxeye daisy, cornflower, lily of
  the valley, lilac/rose-bush/peony lower/upper IDs, and sunflower lower/upper
  IDs; synthetic asset registry mappings; basic shared
  shape/material/render/light facts; Java-style reduced random-patch column
  placement; Java-style waterlily, `FLOWER_SWAMP` blue-orchid flower,
  `BROWN_MUSHROOM_SWAMP` / `RED_MUSHROOM_SWAMP`, and sweet-berry random-patch placement;
  Java-style `nextBoolean` random-boolean selector placement for mushroom fields;
  Java-style seagrass, kelp, sea-pickle, and broad coral tree/claw/mushroom
  placement; broad Java-shaped dark oak tree, huge mushroom, birch and
  tall-birch tree table placement, acacia forking-trunk/flat-canopy placement,
  jungle tree/bush/mega-jungle selector placement, and bamboo
  column/podzol-disk placement; Java-style `ForestFlowerProvider`
  `BIOME_INFO_NOISE` small-flower selection and the Java
  `FOREST_FLOWER_VEGETATION` / `FOREST_FLOWER_VEGETATION_COMMON`
  simple-random mixed flower selectors for ordinary forests, dark forests, and
  flower forests; Java-style `PATCH_GRASS_FOREST` count-2 forest grass;
  Java-style `PATCH_GRASS_BADLANDS` default grass for badlands, flower forests,
  river/frozen-river, and beach/shore rows; Java-style `PATCH_SUNFLOWER`
  placement for sunflower plains;
  Java-shaped `ICE_SPIKE` and `ICE_PATCH` packed-ice surface-structure placement;
  Java-shaped `SEAGRASS_RIVER` count/probability table placement;
  Java-shaped `TREES_WATER`, `PATCH_GRASS_BADLANDS`, `FLOWER_DEFAULT`, and
  `PATCH_SUGAR_CANE` / `PATCH_PUMPKIN` table slots for
  river/frozen-river/beach/shore rows;
  Java-shaped `PATCH_SUGAR_CANE` / `PATCH_PUMPKIN` default-extra table slots
  for plains/sunflower-plains, forest/flower-forest/dark-forest,
  birch/tall-birch, taiga/snowy-taiga/giant-taiga, snowy/ice-spikes,
  mountain, savanna, jungle/bamboo-jungle, mushroom-field, and fallback land
  builders;
  Java-shaped `SPRING_WATER` / `SPRING_LAVA` default spring table slots for the
  current land, river, beach/shore, desert/badlands, swamp, mushroom-field, and
  fallback land builders;
  Java-shaped `BROWN_MUSHROOM_NORMAL` / `RED_MUSHROOM_NORMAL`
  rarity-4/8 no-projection `HEIGHTMAP_DOUBLE_SQUARE` table slots for
  forest, flower-forest, swamp, river/frozen-river, beach/shore, dark-forest,
  savanna, jungle, bamboo-jungle, plains/sunflower-plains, birch/tall-birch,
  taiga/snowy-taiga, giant-taiga, snowy, ice-spikes, mountain,
  badlands, mushroom-field, and fallback land builders;
  Java-shaped `BROWN_MUSHROOM_TAIGA` / `RED_MUSHROOM_TAIGA` table slots for
  taiga, snowy-taiga, and mushroom-field builders, plus counted
  `BROWN_MUSHROOM_GIANT` / `RED_MUSHROOM_GIANT` table slots for giant-taiga
  builders;
  Java-shaped small-mushroom survival for random patches: podzol/mycelium
  mushroom-grow blocks bypass the brightness check, while other solid-render
  substrates require generated raw brightness below 13 using the current
  generated sky/block-light approximation;
  Java-shaped `PATCH_PUMPKIN` block, asset-registry, rarity-32
  `HEIGHTMAP_DOUBLE_SQUARE`, no-projection, and grass-block survival rules for
  the currently modeled desert/badlands/swamp and default-extra lanes;
  Java-shaped jungle-family `PATCH_MELON` table slot, melon block/asset
  registry/render facts, no-projection grass-block survival with `canReplace`,
  and broad `VINES` table slot/feature/block placement against solid neighbors
  using directional vanilla vine face states; normal jungle-tree cocoa
  decorators and normal/mega jungle trunk/leaf vine decorators;
  Java-shaped `PATCH_TALL_GRASS` tall-grass lower/upper block IDs, asset
  registry, shared shape/material/render/light facts, double-plant placement,
  and savanna count-7 table slot; Java-shaped `FLOWER_WARM`, count-20
  `PATCH_GRASS_SAVANNA`, shattered-savanna count-5 `PATCH_GRASS_NORMAL`,
  badlands `PATCH_GRASS_BADLANDS`, badlands count-20
  `PATCH_DEAD_BUSH_BADLANDS`, and wooded-only `TREES_BADLANDS` table slots;
  Java-shaped `ICEBERG_PACKED` / `ICEBERG_BLUE` local-modification placement
  and direct `BLUE_ICE` spread placement;
  Java-shaped giant taiga `MEGA_SPRUCE` / `MEGA_PINE` 2x2 trunk, mega-pine
  foliage, podzol alter-ground, and giant taiga feature-table selectors; the
  Java-shaped `FOREST_ROCK` block-blob feature and its giant-taiga
  `LOCAL_MODIFICATIONS` table slot using mossy cobblestone;
  `NoiseBasedDecorator` count path used by kelp/coral/bamboo; and
  forest/desert/badlands/swamp/river/frozen-river/beach/shore/frozen-ocean/ocean/dark-forest/birch/savanna/jungle/
  bamboo-jungle/flower-forest/sunflower-plains/taiga/snowy-taiga/giant-taiga/mountain/ice-spikes
  feature table entries, plus the mushroom-field huge mushroom table entry.
- `mclone-mesh::tint::tests::palette_matrix_tint_groups_match_java_visual_facts`
  asserts every matrix row's grass, foliage, and water tint output through the
  shared tint resolver by tint group, including the two registered rows without
  B/S fixtures. The swamp grass value is the default radius-2 blend at the
  origin, `#647139`, not the direct light swamp color.
- `mclone-worldgen::levelgen::tests::palette_matrix_low_visibility_feature_slots_have_deterministic_fixtures`
  adds focused low-visibility fixtures outside the one-feature-family matrix:
  plains seed `16` chunk `(1,-1)`, taiga seed `233` chunk `(-1,0)`, and
  mushroom-fields seed `978` chunk `(0,0)` require visible small mushroom
  blocks; mushroom-field shore seed `74` chunk `(-7,-12)`, giant-tree-taiga
  seed `19` chunks `(-12,-12)` and `(-12,-7)`, giant-spruce-taiga seed `132`
  chunk `(10,0)` and seed `93` chunk `(-11,-10)`, mountain seed `3` chunks
  `(-11,4)` and `(0,-1)`, modified-gravelly-mountains seed `9` chunk `(9,9)`,
  and sunflower-plains seed `25` chunk `(-3,7)` require visible small mushroom
  blocks; forest seed `1` chunks `(0,-7)` and `(-1,-11)`, flower-forest seed
  `20` chunk `(11,0)`, birch seed `7` chunks `(-5,-5)` and `(-9,-12)`,
  tall-birch seed `39` chunk `(5,-12)` and seed `32` chunk `(-6,4)`,
  dark-forest seed `0` chunk `(-7,8)` and seed `123` chunk `(1,7)`,
  taiga-hills seed `2` chunk `(-12,12)`, taiga-mountains seed `201` chunk
  `(1,-6)`, snowy seed `14` chunks `(-10,-12)`, `(-12,10)`, `(-12,-7)`,
  and `(-5,10)`, snowy-mountains seed `42` chunk `(-11,-6)`,
  gravelly-mountains seed `123` chunk `(-4,10)`, and ice-spikes seed `59`
  chunk `(-12,-11)` require visible small mushroom blocks; jungle seed `1`
  chunks `(12,1)` and `(15,2)`, jungle-edge seed `149` chunk
  `(4,13)`, modified-jungle seed `102` chunk `(-13,-12)`,
  modified-jungle-edge seed `8201` chunk `(6,-8)`, bamboo-jungle seed `71`
  chunk `(9,0)`, bamboo-jungle-hills seed `102` chunk `(-1,-16)`, savanna
  seed `33` chunk `(-10,-14)`, savanna-plateau seed `38` chunk `(-3,-15)`,
  shattered-savanna seed `68` chunk `(-6,3)`, and shattered-savanna-plateau
  seed `126` chunk `(13,14)`, river seed `1` chunk `(8,-13)`, frozen-river
  seed `66` chunk `(-11,-1)`, beach seed `1` chunk `(-14,-11)`,
  snowy-beach seed `54` chunk `(8,4)`, stone-shore seed `0` chunk `(8,16)`,
  swamp seed `3` chunk `(-16,-3)`, swamp-hills seed `92` chunk `(-5,13)`,
  badlands seed `0` chunk `(-10,-16)`, wooded-badlands-plateau seed `51`
  chunk `(12,-16)`, badlands-plateau seed `28` chunk `(-2,-16)`,
  eroded-badlands seed `86` chunk `(-16,-2)`,
  modified-wooded-badlands-plateau seed `84` chunk `(-13,-2)`, and
  modified-badlands-plateau seed `1150` chunk `(-15,13)` require visible
  small mushroom blocks; jungle
  seed `61` chunk `(1,2)` requires visible cocoa, directional
  vine states, and a jungle-log/oak-leaf bush-skirt shape; savanna seed `62`
  chunk `(3,1)` and sunflower-plains seed `43` chunk `(-1,1)` require exposed
  scheduled spring positions that still contain water and lava blocks; swamp
  seed `211` chunk `(-2,1)`, stone-shore seed `74739` chunk `(3,5)`, river
  seed `58` chunk `(2,1)`, frozen-river seed `326` chunk `(-4,6)`, beach
  seed `349` chunk `(-7,-5)`, swamp-hills seed `3407` chunk `(7,-2)`,
  sunflower-plains seed `25` chunk `(-8,-2)`, desert seed `327` chunk
  `(-6,6)`, desert-hills seed `2768` chunk `(-5,-7)`, badlands seed `2336`
  chunk `(-5,-7)`, wooded-badlands-plateau seed `1248` chunk `(-6,8)`, and
  badlands-plateau seed `653` chunk `(-5,6)`, snowy-beach seed `2805` chunk
  `(14,6)`, desert-lakes seed `5012` chunk `(13,-10)`, eroded-badlands seed
  `85720` chunk `(11,5)`, modified-wooded-badlands-plateau seed `13518`
  chunk `(10,-3)`, and modified-badlands-plateau seed `168856` chunk `(4,10)`
  require visible pumpkins on grass; giant-tree-taiga seed `132` chunk `(0,0)`
  requires visible mossy cobblestone from Java `FOREST_ROCK` block blobs.
- `mclone-worldgen::levelgen::tests::build_eroded_badlands_pillar_surface_and_bedrock_matches_java_oracle`
  pins the Java `ErodedBadlandsSurfaceBuilder` pillar path, including its
  `(int)pillarHeight` fill cutoff, with a tall-pillar surface fixture: seed
  `868` chunk `(8,-6)` reaches top Y `122`, has at least 70 columns above
  Y `100`, and exact-matches the Java `surface-chunk` oracle.

Documented gaps from this slice:

- Default-extra audit: Java `addDefaultExtraVegetation` is only
  `PATCH_SUGAR_CANE` plus rarity-32 `PATCH_PUMPKIN`; desert, badlands, and
  swamp use specialized sugar-cane/cactus variants plus the same pumpkin slot,
  while jungle extra vegetation is melon/vines rather than default extra.
  Native already table-wires the Java-shaped sugar-cane and pumpkin slots for
  the current land lanes, and the matrix has high-signal visible sugar-cane
  coverage through river/frozen-river/beach/shore, desert, badlands, and swamp
  rows. Visible pumpkin fixtures have landed for swamp, stone shore, river,
  frozen river, beach, snowy beach, swamp hills, sunflower plains, desert,
  desert hills, desert lakes, base badlands, wooded badlands plateau, badlands
  plateau, eroded badlands, modified wooded badlands plateau, and modified
  badlands plateau. Further per-row default-extra fixture hunting would mostly
  duplicate table-slot coverage and becomes exact decorated parity, not
  high-value palette proof.
- Java default springs now have table-slot coverage for the current land,
  river, beach/shore, desert/badlands, swamp, mushroom-field, and fallback land
  builders, plus deterministic exposed water/lava spring block fixtures for
  savanna and sunflower plains. These fixtures assert generation-time placement
  at scheduled spring positions, not exact flowing-water/lava tick-shape parity.
- Java dark forest now has the high-signal dark oak plus huge mushroom selector
  path represented, `FOREST_FLOWER_VEGETATION`, glow lichen, count-2 forest
  grass, normal mushroom table slots, and visible small-mushroom fixtures for
  both rows. Exact parity is still
  incomplete: dark oak still uses a reduced `ThreeLayersFeatureSize`
  free-space approximation, huge mushrooms do not yet model directional
  cap/stem side-state booleans, exact normal mushroom/default-extra count
  parity, full light-engine parity, and exact tree-count mismatch buckets
  remain owned by later `103` work.
- Java savanna now has the high-signal acacia selector path represented, plus
  `PATCH_TALL_GRASS`, `FLOWER_WARM`, count-20 `PATCH_GRASS_SAVANNA`, shattered
  count-5 `PATCH_GRASS_NORMAL`, normal mushroom table slots, and visible
  small-mushroom fixtures for all four savanna rows. Exact decorated parity is
  still incomplete: exact warm-flower/tall-grass visibility fixtures, exact
  normal mushroom/default-extra count parity, full light-engine parity,
  villages/outposts, and exact tree-count mismatch buckets remain owned by
  later `103` work.
- Java jungle and bamboo jungle now have high-signal jungle log/leaves and
  bamboo stalk coverage plus normal mushroom table slots, and the Java
  `PATCH_MELON` / `VINES` jungle extra vegetation slots are wired with broad
  native block placement. Native also carries directional vine face states,
  normal jungle-tree cocoa decorators, and normal/mega jungle trunk/leaf vine
  decorators. Mega jungle trees now use the Java-shaped 2x2 trunk, lateral
  branch logs, and `MegaJungleFoliagePlacer` row/radius rules. Jungle bushes now
  use `BushFoliagePlacer` row/radius and random-corner rules, with a visible
  bush-skirt fixture at seed `61` chunk `(1,2)`. Bamboo generation now emits
  Java-shaped `leaves=small`, `leaves=large`, and final `stage=1` large top
  states, and the mesh catalog composes the stem plus leaf multipart models for
  those states. All seven jungle-family rows now also have visible
  small-mushroom fixtures. Exact parity is still incomplete: exact
  tree-count/selector visibility buckets are not checked here, exact normal
  mushroom/default-extra count parity and full light-engine parity are not
  checked here, and exact decorated mismatch buckets remain later `103` work.
- Java normal/cold/lukewarm ocean water-plant tables are represented by broad
  seagrass/kelp checks, and warm ocean now has seagrass, live coral blocks,
  live coral plant/floor-fan/wall-fan sidecars, and sea pickles. Native still
  omits the `SEAGRASS_SIMPLE` carving-mask decorator path, exact coral mismatch
  parity, and full waterlogged/fluid-state modeling for water plants in the raw
  generated block lane.
- Java frozen-ocean and deep-frozen-ocean now have high-signal
  `ICEBERG_PACKED`, `ICEBERG_BLUE`, and `BLUE_ICE` coverage with deterministic
  blue-ice fixtures. Exact parity is still incomplete: ocean ruins, monuments,
  shipwreck/buried-treasure structure context, exact iceberg mismatch buckets,
  and full fluid-state/waterlogged behavior remain later `103` or structure
  work.
- Java river rows now have high-signal `SEAGRASS_RIVER` seagrass coverage for
  non-frozen river and deterministic default sugar-cane extras for frozen
  river. Native also wires the Java `TREES_WATER`, `FLOWER_DEFAULT`,
  `PATCH_GRASS_BADLANDS`, normal mushroom, `PATCH_PUMPKIN`, and spring slots.
  River and frozen river now also have visible pumpkin and visible
  small-mushroom fixtures. Exact parity is still incomplete: exact normal
  mushroom and water-tree counts, full light-engine parity,
  river-bank boundary shape, exact seagrass counts, and full
  waterlogged/fluid-state modeling for water plants remain later `103` or
  boundary work.
- Java beach, snowy beach, and stone shore now have deterministic default
  sugar-cane extra coverage, plus the shared default flower/grass, normal
  mushroom, and spring table slots. Beach and stone shore also have visible
  pumpkin fixtures, and all three rows have visible small-mushroom fixtures.
  Exact parity is still incomplete: exact normal mushroom counts, full
  light-engine parity,
  buried treasure, shipwrecks, mineshafts, steep shore/coast boundaries, and
  exact decorated mismatch buckets remain later `103`, structure, or boundary
  work.
- Java swamp and swamp hills now have high-signal water-lily, blue-orchid,
  small/normal-mushroom, and sugar-cane coverage, and the `PATCH_PUMPKIN` table
  slot is wired. Base swamp and swamp hills also have visible pumpkin and
  visible small-mushroom fixtures. Exact parity is still incomplete:
  swamp-hills fossil ordering,
  swamp/swamp-hills seagrass and extras, and exact decorated counts remain
  later `103` work.
- Java mushroom fields and mushroom-field shore now have high-signal huge
  mushroom coverage, plus taiga-style/normal mushroom, default-extra, and
  spring table slots. Both rows also have visible small-mushroom fixture
  coverage. Exact parity is still incomplete: visible pumpkin fixture coverage,
  exact huge mushroom side-state booleans, and spawn-table/no-normal-hostile
  behavior remain later `103` or entity-runtime work.
- Java birch forests now have high-signal birch log/leaves coverage, including
  separate normal birch and tall-birch table shapes, plus normal mushroom,
  default-extra, spring table slots, and visible small-mushroom fixtures for all
  four birch/tall-birch rows. Exact parity is still incomplete: tree counts,
  bee-nest side effects, flower/grass/default-extra visibility counts, exact
  mushroom counts, and exact decorated mismatch buckets remain later `103`
  work.
- Java taiga now has high-signal sweet berry bush coverage for base taiga,
  taiga hills, taiga mountains, base snowy taiga, and snowy taiga hills,
  including the Java `PATCH_BERRY_SPARSE` and
  `PATCH_BERRY_DECORATED.rarity(12)` table split, and the taiga/normal
  mushroom table slots are wired for taiga and snowy-taiga builders. Snowy
  taiga mountains now has deterministic spruce/fern coverage, but the checked
  `(0,0)` candidates inspected in this slice did not surface a berry bush.
  Exact parity is still incomplete: snowy taiga mountains berry visibility,
  default-extra fixtures, exact small-mushroom counts, and exact decorated
  mismatch buckets remain later `103` work.
- Java mountain rows now have high-signal oak/spruce tree-family coverage,
  including the wooded-mountain / mountain-edge tree-density table distinction
  at the row level, plus normal mushroom table slots. Base mountains, wooded
  mountains, gravelly mountains, and modified gravelly mountains also have
  visible small-mushroom fixture coverage. Exact parity is still incomplete:
  extra emerald and infested stone are not palette-checked here, exact
  small-mushroom counts, and exact tree-count mismatch buckets remain later
  `103` work. Java 1.17.1 `FOREST_ROCK` belongs to giant taiga builders, not
  mountain builders.
- Java badlands rows now have Java-shaped `PATCH_GRASS_BADLANDS`, count-20
  `PATCH_DEAD_BUSH_BADLANDS`, normal mushrooms, badlands sugar-cane/cactus,
  pumpkin, springs, and the wooded-only `TREES_BADLANDS` split. Matrix fixtures
  cover non-wooded badlands dead-bush plus cactus/sugar-cane visibility and
  wooded badlands oak plus badlands-extra visibility across the base, plateau,
  eroded, and modified variants. Base badlands, wooded badlands plateau, and
  badlands plateau, eroded badlands, modified wooded badlands plateau, and
  modified badlands plateau now also have visible pumpkin and visible
  small-mushroom fixtures. Eroded badlands now also has an exact surface-stage
  Java oracle for tall pillar columns. Exact parity is still incomplete:
  mineshaft/structure context, exact mushroom counts, and exact decorated
  mismatch buckets remain later `103` work.
- Java desert rows now have high-signal dead-bush plus cactus/sugar-cane family
  coverage across desert, desert hills, and desert lakes. Desert and desert
  hills and desert lakes now also have visible pumpkin fixtures. Exact parity is
  still incomplete: desert wells, fossils/lake behavior, villages/outposts/pyramids,
  and exact decorated mismatch buckets remain later `103` work.
- Java giant taiga now has high-signal mega spruce / mega pine tree coverage,
  including the giant 2x2 trunk, mega-pine foliage family, podzol
  alter-ground, giant tree vs giant spruce selector weights, counted
  giant-taiga mushroom table slots, normal mushroom table slots, default-extra
  table slots, spring table slots, and Java `FOREST_ROCK`
  mossy-cobblestone block-blob placement with a deterministic visible fixture
  at seed `132`, chunk `(0,0)`. All four giant-taiga rows also have visible
  small-mushroom fixtures. Exact parity is still incomplete: default-extra
  fixtures, exact alter-ground disk shape, and exact decorated mismatch buckets
  remain later `103` work.
- Java ordinary forest now has high-signal oak/birch tree coverage for forest
  and wooded hills through the existing `BIRCH_OTHER`-shaped selector, plus
  `FOREST_FLOWER_VEGETATION`, default flowers, count-2 forest grass, normal
  mushroom, default-extra, spring table slots, and visible small-mushroom
  fixtures for both rows. Exact parity is still incomplete: exact
  forest-flower/default-extra visibility and count parity, full light-engine
  parity, bee-nest side effects, and exact tree-count mismatch buckets remain
  later `103` work.
- Java flower forest now has high-signal dense small-flower coverage through
  the Java `FLOWER_FOREST` random patch and `ForestFlowerProvider` noise family,
  plus broad common lilac/rose-bush/peony/lily-of-the-valley vegetation coverage
  through `FOREST_FLOWER_VEGETATION_COMMON`, plus Java default grass, normal
  mushroom, default-extra, spring table slots, and a visible small-mushroom
  fixture. Exact parity is still incomplete: exact normal mushroom/default-extra
  count parity, full light-engine parity, bee-nest side effects, and exact
  decorated mismatch buckets remain later `103` work.
- Java sunflower plains now has high-signal sunflower coverage through
  `PATCH_SUNFLOWER`, plus the Java-order sugar-cane, normal mushroom, pumpkin,
  and spring table slots. Sunflower plains now also has exposed default
  spring-water/lava, visible-pumpkin, and visible small-mushroom fixtures.
  Exact parity is still incomplete: villages/outposts, normal plains mismatch
  buckets, and exact decorated counts remain later `103` work.
- Java ice spikes now has high-signal packed-ice coverage through a native
  `ICE_SPIKE` / `ICE_PATCH` surface-structure path, plus visible
  small-mushroom fixture coverage. Exact parity is still incomplete: spike
  geometry, patch disk counts, and exact decorated mismatch buckets remain
  later `103` work.
- Java snowy mountains now has deterministic coverage for the native
  snowy spruce/fern family through the shared snowy feature table, and snowy
  tundra plus snowy mountains now have visible small-mushroom fixtures. Exact
  parity is still incomplete: Java `TREES_SNOWY` uses a much sparser
  `count_extra(0, 0.1, 1)` spruce table, Java `addDefaultGrass` is
  `PATCH_GRASS_BADLANDS` rather than fern-heavy vegetation, and default flowers
  plus exact mushroom/default-extra counts remain later `103` work.

## Suggested Next Slice

The emitted-row palette matrix is now full, the main land-builder
normal/taiga/giant mushroom table lanes are wired, Java default extra
vegetation and springs are table-wired for the current main land lanes, the
small-mushroom survival gate is no longer over-permissive, savanna/forest
flower and grass table distinctions are wired, low-visibility mushroom fixtures
now cover plains, forest, birch, dark-forest, taiga, snowy, mushroom-field,
mushroom-shore, giant-taiga, mountain, savanna, jungle, river, beach/shore,
swamp, badlands, sunflower-plains, and ice-spikes rows, exposed-spring fixtures
are pinned, and jungle extra melon/vine slots plus jungle cocoa/directional
vines are represented. Move the next chunk to either the remaining
low-visibility fixture breadth or a visible row-specific feature slice:

1. Re-read `BiomeDefaultFeatures` / `VanillaBiomes` and decide whether to wire
   default mushroom table slots into desert and ocean builders now, or leave
   them with `103` exact decorated parity because they are usually non-visible
   in current palette fixtures.
2. Treat broad default-extra coverage as saturated for this palette matrix:
   keep the current representative sugar-cane, pumpkin, and exposed-spring
   fixtures here, and move additional default-extra counts to `103` exact
   decorated parity.
3. Pick one missing high-signal family outside the broad table slots: exact
   jungle tree selector/count visibility buckets if staying in biome visual
   variety, or spawn-table/no-normal-hostile coverage if shifting toward biome
   behavior.
