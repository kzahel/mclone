import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { ItemBlockRenderTypes } from "../../renderer/item-block-render-types";
import { RenderType } from "../../renderer/render-type";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { AirBlock } from "./block/air-block";
import { Block } from "./block/block";
import { BushBlock } from "./block/bush-block";
import { CactusBlock } from "./block/cactus-block";
import { CocoaBlock } from "./block/cocoa-block";
import { DeadBushBlock } from "./block/dead-bush-block";
import { DoublePlantBlock } from "./block/double-plant-block";
import { HugeMushroomBlock } from "./block/huge-mushroom-block";
import { KelpBlock } from "./block/kelp-block";
import { KelpPlantBlock } from "./block/kelp-plant-block";
import { LeavesBlock } from "./block/leaves-block";
import { LiquidBlock } from "./block/liquid-block";
import { MushroomBlock } from "./block/mushroom-block";
import { RotatedPillarBlock } from "./block/rotated-pillar-block";
import { SeagrassBlock } from "./block/seagrass-block";
import { SnowLayerBlock } from "./block/snow-layer-block";
import { SnowyDirtBlock } from "./block/snowy-dirt-block";
import { SugarCaneBlock } from "./block/sugar-cane-block";
import { SweetBerryBushBlock } from "./block/sweet-berry-bush-block";
import { TallSeagrassBlock } from "./block/tall-seagrass-block";
import { VineBlock } from "./block/vine-block";
import { WaterlilyBlock } from "./block/waterlily-block";
import { SoundType } from "./block/sound-type";
import { BlockBehaviour } from "./block/state/block-behaviour";
import type { BlockState } from "./block/state/block-state";
import { Fluids } from "./material/fluids";
import { Material } from "./material/material";
import { MaterialColor } from "./material/material-color";

const STONE_LOCATION = new ResourceLocation("minecraft:stone");
const AIR_LOCATION = new ResourceLocation("minecraft:air");
const BEDROCK_LOCATION = new ResourceLocation("minecraft:bedrock");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const GRAVEL_LOCATION = new ResourceLocation("minecraft:gravel");
const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");
const SNOW_LOCATION = new ResourceLocation("minecraft:snow");
const OBSIDIAN_LOCATION = new ResourceLocation("minecraft:obsidian");
const MAGMA_BLOCK_LOCATION = new ResourceLocation("minecraft:magma_block");
const OAK_LOG_LOCATION = new ResourceLocation("minecraft:oak_log");
const OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:oak_leaves");
const ACACIA_LOG_LOCATION = new ResourceLocation("minecraft:acacia_log");
const ACACIA_LEAVES_LOCATION = new ResourceLocation("minecraft:acacia_leaves");
const SPRUCE_LOG_LOCATION = new ResourceLocation("minecraft:spruce_log");
const SPRUCE_LEAVES_LOCATION = new ResourceLocation("minecraft:spruce_leaves");
const BIRCH_LOG_LOCATION = new ResourceLocation("minecraft:birch_log");
const BIRCH_LEAVES_LOCATION = new ResourceLocation("minecraft:birch_leaves");
const DARK_OAK_LOG_LOCATION = new ResourceLocation("minecraft:dark_oak_log");
const DARK_OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:dark_oak_leaves");
const JUNGLE_LOG_LOCATION = new ResourceLocation("minecraft:jungle_log");
const JUNGLE_LEAVES_LOCATION = new ResourceLocation("minecraft:jungle_leaves");
const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const DANDELION_LOCATION = new ResourceLocation("minecraft:dandelion");
const POPPY_LOCATION = new ResourceLocation("minecraft:poppy");
const ALLIUM_LOCATION = new ResourceLocation("minecraft:allium");
const AZURE_BLUET_LOCATION = new ResourceLocation("minecraft:azure_bluet");
const RED_TULIP_LOCATION = new ResourceLocation("minecraft:red_tulip");
const ORANGE_TULIP_LOCATION = new ResourceLocation("minecraft:orange_tulip");
const WHITE_TULIP_LOCATION = new ResourceLocation("minecraft:white_tulip");
const PINK_TULIP_LOCATION = new ResourceLocation("minecraft:pink_tulip");
const OXEYE_DAISY_LOCATION = new ResourceLocation("minecraft:oxeye_daisy");
const CORNFLOWER_LOCATION = new ResourceLocation("minecraft:cornflower");
const OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:oak_sapling");
const ACACIA_SAPLING_LOCATION = new ResourceLocation("minecraft:acacia_sapling");
const SPRUCE_SAPLING_LOCATION = new ResourceLocation("minecraft:spruce_sapling");
const BIRCH_SAPLING_LOCATION = new ResourceLocation("minecraft:birch_sapling");
const DARK_OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:dark_oak_sapling");
const JUNGLE_SAPLING_LOCATION = new ResourceLocation("minecraft:jungle_sapling");
const LARGE_FERN_LOCATION = new ResourceLocation("minecraft:large_fern");
const SWEET_BERRY_BUSH_LOCATION = new ResourceLocation("minecraft:sweet_berry_bush");
const BROWN_MUSHROOM_LOCATION = new ResourceLocation("minecraft:brown_mushroom");
const RED_MUSHROOM_LOCATION = new ResourceLocation("minecraft:red_mushroom");
const BLUE_ORCHID_LOCATION = new ResourceLocation("minecraft:blue_orchid");
const DEAD_BUSH_LOCATION = new ResourceLocation("minecraft:dead_bush");
const BROWN_MUSHROOM_BLOCK_LOCATION = new ResourceLocation("minecraft:brown_mushroom_block");
const RED_MUSHROOM_BLOCK_LOCATION = new ResourceLocation("minecraft:red_mushroom_block");
const MUSHROOM_STEM_LOCATION = new ResourceLocation("minecraft:mushroom_stem");
const SEAGRASS_LOCATION = new ResourceLocation("minecraft:seagrass");
const TALL_SEAGRASS_LOCATION = new ResourceLocation("minecraft:tall_seagrass");
const KELP_LOCATION = new ResourceLocation("minecraft:kelp");
const KELP_PLANT_LOCATION = new ResourceLocation("minecraft:kelp_plant");
const LILY_PAD_LOCATION = new ResourceLocation("minecraft:lily_pad");
const TALL_GRASS_LOCATION = new ResourceLocation("minecraft:tall_grass");
const LILAC_LOCATION = new ResourceLocation("minecraft:lilac");
const ROSE_BUSH_LOCATION = new ResourceLocation("minecraft:rose_bush");
const PEONY_LOCATION = new ResourceLocation("minecraft:peony");
const LILY_OF_THE_VALLEY_LOCATION = new ResourceLocation("minecraft:lily_of_the_valley");
const PUMPKIN_LOCATION = new ResourceLocation("minecraft:pumpkin");
const MELON_LOCATION = new ResourceLocation("minecraft:melon");
const CACTUS_LOCATION = new ResourceLocation("minecraft:cactus");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");
const VINE_LOCATION = new ResourceLocation("minecraft:vine");
const COCOA_LOCATION = new ResourceLocation("minecraft:cocoa");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");
const COARSE_DIRT_LOCATION = new ResourceLocation("minecraft:coarse_dirt");
const PODZOL_LOCATION = new ResourceLocation("minecraft:podzol");
const MYCELIUM_LOCATION = new ResourceLocation("minecraft:mycelium");
const TERRACOTTA_LOCATION = new ResourceLocation("minecraft:terracotta");
const WHITE_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:white_terracotta");
const ORANGE_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:orange_terracotta");
const MAGENTA_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:magenta_terracotta");
const LIGHT_BLUE_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:light_blue_terracotta");
const YELLOW_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:yellow_terracotta");
const LIME_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:lime_terracotta");
const PINK_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:pink_terracotta");
const GRAY_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:gray_terracotta");
const LIGHT_GRAY_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:light_gray_terracotta");
const CYAN_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:cyan_terracotta");
const PURPLE_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:purple_terracotta");
const BLUE_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:blue_terracotta");
const BROWN_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:brown_terracotta");
const GREEN_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:green_terracotta");
const RED_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:red_terracotta");
const BLACK_TERRACOTTA_LOCATION = new ResourceLocation("minecraft:black_terracotta");
const SANDSTONE_LOCATION = new ResourceLocation("minecraft:sandstone");
const RED_SANDSTONE_LOCATION = new ResourceLocation("minecraft:red_sandstone");
const PACKED_ICE_LOCATION = new ResourceLocation("minecraft:packed_ice");
const RED_SAND_LOCATION = new ResourceLocation("minecraft:red_sand");
const ICE_LOCATION = new ResourceLocation("minecraft:ice");
const SNOW_BLOCK_LOCATION = new ResourceLocation("minecraft:snow_block");

function blockTexture(path: string): ResourceLocation {
  return new ResourceLocation(`minecraft:block/${path}`);
}

const GENERATED_BLOCK_LOCATIONS = [
  STONE_LOCATION,
  BEDROCK_LOCATION,
  GRASS_BLOCK_LOCATION,
  DIRT_LOCATION,
  SAND_LOCATION,
  GRAVEL_LOCATION,
  WATER_LOCATION,
  LAVA_LOCATION,
  SNOW_LOCATION,
  OBSIDIAN_LOCATION,
  MAGMA_BLOCK_LOCATION,
  OAK_LOG_LOCATION,
  OAK_LEAVES_LOCATION,
  ACACIA_LOG_LOCATION,
  ACACIA_LEAVES_LOCATION,
  SPRUCE_LOG_LOCATION,
  SPRUCE_LEAVES_LOCATION,
  BIRCH_LOG_LOCATION,
  BIRCH_LEAVES_LOCATION,
  DARK_OAK_LOG_LOCATION,
  DARK_OAK_LEAVES_LOCATION,
  JUNGLE_LOG_LOCATION,
  JUNGLE_LEAVES_LOCATION,
  GRASS_LOCATION,
  FERN_LOCATION,
  DANDELION_LOCATION,
  POPPY_LOCATION,
  ALLIUM_LOCATION,
  AZURE_BLUET_LOCATION,
  RED_TULIP_LOCATION,
  ORANGE_TULIP_LOCATION,
  WHITE_TULIP_LOCATION,
  PINK_TULIP_LOCATION,
  OXEYE_DAISY_LOCATION,
  CORNFLOWER_LOCATION,
  OAK_SAPLING_LOCATION,
  ACACIA_SAPLING_LOCATION,
  SPRUCE_SAPLING_LOCATION,
  BIRCH_SAPLING_LOCATION,
  DARK_OAK_SAPLING_LOCATION,
  JUNGLE_SAPLING_LOCATION,
  LARGE_FERN_LOCATION,
  SWEET_BERRY_BUSH_LOCATION,
  BROWN_MUSHROOM_LOCATION,
  RED_MUSHROOM_LOCATION,
  BLUE_ORCHID_LOCATION,
  DEAD_BUSH_LOCATION,
  BROWN_MUSHROOM_BLOCK_LOCATION,
  RED_MUSHROOM_BLOCK_LOCATION,
  MUSHROOM_STEM_LOCATION,
  SEAGRASS_LOCATION,
  TALL_SEAGRASS_LOCATION,
  KELP_LOCATION,
  KELP_PLANT_LOCATION,
  LILY_PAD_LOCATION,
  TALL_GRASS_LOCATION,
  LILAC_LOCATION,
  ROSE_BUSH_LOCATION,
  PEONY_LOCATION,
  LILY_OF_THE_VALLEY_LOCATION,
  PUMPKIN_LOCATION,
  MELON_LOCATION,
  CACTUS_LOCATION,
  SUGAR_CANE_LOCATION,
  VINE_LOCATION,
  COCOA_LOCATION,
  GRANITE_LOCATION,
  DIORITE_LOCATION,
  ANDESITE_LOCATION,
  COARSE_DIRT_LOCATION,
  PODZOL_LOCATION,
  MYCELIUM_LOCATION,
  TERRACOTTA_LOCATION,
  WHITE_TERRACOTTA_LOCATION,
  ORANGE_TERRACOTTA_LOCATION,
  MAGENTA_TERRACOTTA_LOCATION,
  LIGHT_BLUE_TERRACOTTA_LOCATION,
  YELLOW_TERRACOTTA_LOCATION,
  LIME_TERRACOTTA_LOCATION,
  PINK_TERRACOTTA_LOCATION,
  GRAY_TERRACOTTA_LOCATION,
  LIGHT_GRAY_TERRACOTTA_LOCATION,
  CYAN_TERRACOTTA_LOCATION,
  PURPLE_TERRACOTTA_LOCATION,
  BLUE_TERRACOTTA_LOCATION,
  BROWN_TERRACOTTA_LOCATION,
  GREEN_TERRACOTTA_LOCATION,
  RED_TERRACOTTA_LOCATION,
  BLACK_TERRACOTTA_LOCATION,
  SANDSTONE_LOCATION,
  RED_SANDSTONE_LOCATION,
  PACKED_ICE_LOCATION,
  RED_SAND_LOCATION,
  ICE_LOCATION,
  SNOW_BLOCK_LOCATION,
] as const;

const GENERATED_SPRITE_LOCATIONS: readonly ResourceLocation[] = [
  blockTexture("stone"),
  blockTexture("bedrock"),
  blockTexture("grass_block_top"),
  blockTexture("grass_block_side"),
  blockTexture("grass_block_side_overlay"),
  blockTexture("grass_block_snow"),
  blockTexture("dirt"),
  blockTexture("sand"),
  blockTexture("gravel"),
  blockTexture("water_still"),
  blockTexture("water_flow"),
  blockTexture("water_overlay"),
  blockTexture("lava_still"),
  blockTexture("lava_flow"),
  blockTexture("snow"),
  blockTexture("obsidian"),
  blockTexture("magma"),
  blockTexture("oak_log"),
  blockTexture("oak_log_top"),
  blockTexture("oak_leaves"),
  blockTexture("acacia_log"),
  blockTexture("acacia_log_top"),
  blockTexture("acacia_leaves"),
  blockTexture("spruce_log"),
  blockTexture("spruce_log_top"),
  blockTexture("spruce_leaves"),
  blockTexture("birch_log"),
  blockTexture("birch_log_top"),
  blockTexture("birch_leaves"),
  blockTexture("dark_oak_log"),
  blockTexture("dark_oak_log_top"),
  blockTexture("dark_oak_leaves"),
  blockTexture("jungle_log"),
  blockTexture("jungle_log_top"),
  blockTexture("jungle_leaves"),
  blockTexture("grass"),
  blockTexture("fern"),
  blockTexture("dandelion"),
  blockTexture("poppy"),
  blockTexture("allium"),
  blockTexture("azure_bluet"),
  blockTexture("red_tulip"),
  blockTexture("orange_tulip"),
  blockTexture("white_tulip"),
  blockTexture("pink_tulip"),
  blockTexture("oxeye_daisy"),
  blockTexture("cornflower"),
  blockTexture("oak_sapling"),
  blockTexture("acacia_sapling"),
  blockTexture("spruce_sapling"),
  blockTexture("birch_sapling"),
  blockTexture("dark_oak_sapling"),
  blockTexture("jungle_sapling"),
  blockTexture("large_fern_bottom"),
  blockTexture("large_fern_top"),
  blockTexture("sweet_berry_bush_stage0"),
  blockTexture("sweet_berry_bush_stage1"),
  blockTexture("sweet_berry_bush_stage2"),
  blockTexture("sweet_berry_bush_stage3"),
  blockTexture("brown_mushroom"),
  blockTexture("red_mushroom"),
  blockTexture("blue_orchid"),
  blockTexture("dead_bush"),
  blockTexture("brown_mushroom_block"),
  blockTexture("red_mushroom_block"),
  blockTexture("mushroom_stem"),
  blockTexture("seagrass"),
  blockTexture("tall_seagrass_bottom"),
  blockTexture("tall_seagrass_top"),
  blockTexture("kelp"),
  blockTexture("kelp_plant"),
  blockTexture("lily_pad"),
  blockTexture("tall_grass_bottom"),
  blockTexture("tall_grass_top"),
  blockTexture("lilac_bottom"),
  blockTexture("lilac_top"),
  blockTexture("rose_bush_bottom"),
  blockTexture("rose_bush_top"),
  blockTexture("peony_bottom"),
  blockTexture("peony_top"),
  blockTexture("lily_of_the_valley"),
  blockTexture("pumpkin_top"),
  blockTexture("pumpkin_side"),
  blockTexture("melon_top"),
  blockTexture("melon_side"),
  blockTexture("cactus_side"),
  blockTexture("cactus_top"),
  blockTexture("cactus_bottom"),
  blockTexture("sugar_cane"),
  blockTexture("vine"),
  blockTexture("cocoa_stage0"),
  blockTexture("cocoa_stage1"),
  blockTexture("cocoa_stage2"),
  blockTexture("granite"),
  blockTexture("diorite"),
  blockTexture("andesite"),
  blockTexture("coarse_dirt"),
  blockTexture("podzol_top"),
  blockTexture("podzol_side"),
  blockTexture("mycelium_top"),
  blockTexture("mycelium_side"),
  blockTexture("terracotta"),
  blockTexture("white_terracotta"),
  blockTexture("orange_terracotta"),
  blockTexture("magenta_terracotta"),
  blockTexture("light_blue_terracotta"),
  blockTexture("yellow_terracotta"),
  blockTexture("lime_terracotta"),
  blockTexture("pink_terracotta"),
  blockTexture("gray_terracotta"),
  blockTexture("light_gray_terracotta"),
  blockTexture("cyan_terracotta"),
  blockTexture("purple_terracotta"),
  blockTexture("blue_terracotta"),
  blockTexture("brown_terracotta"),
  blockTexture("green_terracotta"),
  blockTexture("red_terracotta"),
  blockTexture("black_terracotta"),
  blockTexture("sandstone_top"),
  blockTexture("sandstone_bottom"),
  blockTexture("sandstone"),
  blockTexture("red_sandstone_top"),
  blockTexture("red_sandstone_bottom"),
  blockTexture("red_sandstone"),
  blockTexture("packed_ice"),
  blockTexture("red_sand"),
  blockTexture("ice"),
];

function registerBlock<T extends Block>(location: ResourceLocation, block: T): T {
  block.setLocation(location);
  Registry.register(Registry.BLOCK, location, block);
  return block;
}

export interface GeneratedRenderBlockPalette {
  readonly airState: BlockState;
  readonly blockStateById: readonly BlockState[];
  readonly blockLocations: readonly ResourceLocation[];
  readonly spriteLocations: readonly ResourceLocation[];
}

export function createAirState(): BlockState {
  return new AirBlock(BlockBehaviour.Properties.of(Material.AIR).noCollission().noOcclusion().air()).defaultBlockState();
}

export function registerGeneratedRenderBlocks(): GeneratedRenderBlockPalette {
  Registry.BLOCK.clear();

  const airState = createAirState();
  registerBlock(AIR_LOCATION, airState.getBlock());
  const stoneState = registerBlock(
    STONE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.STONE).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  ).defaultBlockState();
  const bedrockState = registerBlock(
    BEDROCK_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE).strength(-1.0, 3_600_000.0)),
  ).defaultBlockState();
  const grassState = registerBlock(
    GRASS_BLOCK_LOCATION,
    new SnowyDirtBlock(BlockBehaviour.Properties.of(Material.GRASS).randomTicks().strength(0.6).sound(SoundType.GRASS)),
  ).defaultBlockState();
  const dirtState = registerBlock(
    DIRT_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.DIRT, MaterialColor.DIRT).strength(0.5).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const sandState = registerBlock(
    SAND_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.SAND, MaterialColor.SAND).strength(0.5).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const gravelState = registerBlock(
    GRAVEL_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.SAND, MaterialColor.STONE).strength(0.6).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const redSandState = registerBlock(
    RED_SAND_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.SAND, MaterialColor.COLOR_ORANGE).strength(0.5).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const waterState = registerBlock(
    WATER_LOCATION,
    new LiquidBlock(Fluids.WATER, BlockBehaviour.Properties.of(Material.WATER).noCollission().strength(100.0)),
  ).defaultBlockState();
  Fluids.WATER.setLegacyBlock(waterState);
  const lavaState = registerBlock(
    LAVA_LOCATION,
    new LiquidBlock(Fluids.LAVA, BlockBehaviour.Properties.of(Material.LAVA).noCollission().randomTicks().strength(100.0).lightLevel(() => 15)),
  ).defaultBlockState();
  Fluids.LAVA.setLegacyBlock(lavaState);
  const snowState = registerBlock(
    SNOW_LOCATION,
    // WebGPU: partial-block occlusion stays disabled until voxel-shape-based meshing is ported.
    new SnowLayerBlock(
      BlockBehaviour.Properties.of(Material.TOP_SNOW)
        .randomTicks()
        .strength(0.1)
        .requiresCorrectToolForDrops()
        .sound(SoundType.SNOW)
        .noOcclusion(),
    ),
  ).defaultBlockState();
  const snowBlockState = registerBlock(
    SNOW_BLOCK_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.SNOW, MaterialColor.SNOW).strength(0.2).sound(SoundType.SNOW)),
  ).defaultBlockState();
  const iceState = registerBlock(
    ICE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.ICE, MaterialColor.ICE).strength(0.5)),
  ).defaultBlockState();
  const packedIceState = registerBlock(
    PACKED_ICE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.ICE_SOLID, MaterialColor.ICE).strength(0.5).sound(SoundType.STONE)),
  ).defaultBlockState();
  const graniteState = registerBlock(
    GRANITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.DIRT).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  ).defaultBlockState();
  const dioriteState = registerBlock(
    DIORITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.QUARTZ).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  ).defaultBlockState();
  const andesiteState = registerBlock(
    ANDESITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.STONE).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  ).defaultBlockState();
  const coarseDirtState = registerBlock(
    COARSE_DIRT_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.DIRT, MaterialColor.DIRT).strength(0.5).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const podzolState = registerBlock(
    PODZOL_LOCATION,
    new SnowyDirtBlock(BlockBehaviour.Properties.of(Material.DIRT, MaterialColor.PODZOL).strength(0.5).sound(SoundType.GRAVEL)),
  ).defaultBlockState();
  const myceliumState = registerBlock(
    MYCELIUM_LOCATION,
    new SnowyDirtBlock(BlockBehaviour.Properties.of(Material.GRASS, MaterialColor.COLOR_PURPLE).randomTicks().strength(0.6).sound(SoundType.GRASS)),
  ).defaultBlockState();
  const terracottaState = registerBlock(
    TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_WHITE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const whiteTerracottaState = registerBlock(
    WHITE_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_WHITE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const orangeTerracottaState = registerBlock(
    ORANGE_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_ORANGE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const magentaTerracottaState = registerBlock(
    MAGENTA_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_MAGENTA).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const lightBlueTerracottaState = registerBlock(
    LIGHT_BLUE_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_LIGHT_BLUE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const yellowTerracottaState = registerBlock(
    YELLOW_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_YELLOW).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const limeTerracottaState = registerBlock(
    LIME_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_LIGHT_GREEN).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const pinkTerracottaState = registerBlock(
    PINK_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_PINK).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const grayTerracottaState = registerBlock(
    GRAY_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_GRAY).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const lightGrayTerracottaState = registerBlock(
    LIGHT_GRAY_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_LIGHT_GRAY).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const cyanTerracottaState = registerBlock(
    CYAN_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_CYAN).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const purpleTerracottaState = registerBlock(
    PURPLE_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_PURPLE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const blueTerracottaState = registerBlock(
    BLUE_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_BLUE).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const brownTerracottaState = registerBlock(
    BROWN_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_BROWN).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const greenTerracottaState = registerBlock(
    GREEN_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_GREEN).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const redTerracottaState = registerBlock(
    RED_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_RED).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const blackTerracottaState = registerBlock(
    BLACK_TERRACOTTA_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.TERRACOTTA_BLACK).requiresCorrectToolForDrops().strength(1.25, 4.2)),
  ).defaultBlockState();
  const sandstoneState = registerBlock(
    SANDSTONE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.SAND).requiresCorrectToolForDrops().strength(0.8)),
  ).defaultBlockState();
  const redSandstoneState = registerBlock(
    RED_SANDSTONE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.COLOR_ORANGE).requiresCorrectToolForDrops().strength(0.8)),
  ).defaultBlockState();
  const obsidianState = registerBlock(
    OBSIDIAN_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.COLOR_BLACK).requiresCorrectToolForDrops().strength(50.0, 1200.0)),
  ).defaultBlockState();
  const magmaBlockState = registerBlock(
    MAGMA_BLOCK_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.NETHER).lightLevel(() => 3).strength(0.5).sound(SoundType.STONE)),
  ).defaultBlockState();
  registerBlock(
    OAK_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.WOOD).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    OAK_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  registerBlock(
    ACACIA_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.COLOR_ORANGE).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    ACACIA_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  registerBlock(
    SPRUCE_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.WOOD).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    SPRUCE_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  registerBlock(
    BIRCH_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.WOOD).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    BIRCH_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  registerBlock(
    DARK_OAK_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.COLOR_BROWN).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    DARK_OAK_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  registerBlock(
    JUNGLE_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.DIRT).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    JUNGLE_LEAVES_LOCATION,
    new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES, MaterialColor.PLANT).strength(0.2).randomTicks().sound(SoundType.GRASS)),
  );
  const grassPlantState = registerBlock(
    GRASS_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const fernState = registerBlock(
    FERN_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const dandelionState = registerBlock(
    DANDELION_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const poppyState = registerBlock(
    POPPY_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const alliumState = registerBlock(
    ALLIUM_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const azureBluetState = registerBlock(
    AZURE_BLUET_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const redTulipState = registerBlock(
    RED_TULIP_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const orangeTulipState = registerBlock(
    ORANGE_TULIP_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const whiteTulipState = registerBlock(
    WHITE_TULIP_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const pinkTulipState = registerBlock(
    PINK_TULIP_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const oxeyeDaisyState = registerBlock(
    OXEYE_DAISY_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const cornflowerState = registerBlock(
    CORNFLOWER_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const oakSaplingState = registerBlock(
    OAK_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const acaciaSaplingState = registerBlock(
    ACACIA_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const spruceSaplingState = registerBlock(
    SPRUCE_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const birchSaplingState = registerBlock(
    BIRCH_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const darkOakSaplingState = registerBlock(
    DARK_OAK_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const jungleSaplingState = registerBlock(
    JUNGLE_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const largeFernState = registerBlock(
    LARGE_FERN_LOCATION,
    new DoublePlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const sweetBerryBushState = registerBlock(
    SWEET_BERRY_BUSH_LOCATION,
    new SweetBerryBushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const brownMushroomState = registerBlock(
    BROWN_MUSHROOM_LOCATION,
    new MushroomBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const redMushroomState = registerBlock(
    RED_MUSHROOM_LOCATION,
    new MushroomBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const blueOrchidState = registerBlock(
    BLUE_ORCHID_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const deadBushState = registerBlock(
    DEAD_BUSH_LOCATION,
    new DeadBushBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const brownMushroomBlockState = registerBlock(
    BROWN_MUSHROOM_BLOCK_LOCATION,
    new HugeMushroomBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.COLOR_BROWN).strength(0.2).sound(SoundType.WOOD)),
  ).defaultBlockState();
  const redMushroomBlockState = registerBlock(
    RED_MUSHROOM_BLOCK_LOCATION,
    new HugeMushroomBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.COLOR_RED).strength(0.2).sound(SoundType.WOOD)),
  ).defaultBlockState();
  const mushroomStemState = registerBlock(
    MUSHROOM_STEM_LOCATION,
    new HugeMushroomBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.WOOL).strength(0.2).sound(SoundType.WOOD)),
  ).defaultBlockState();
  const seagrassState = registerBlock(
    SEAGRASS_LOCATION,
    new SeagrassBlock(BlockBehaviour.Properties.of(Material.WATER_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const tallSeagrassState = registerBlock(
    TALL_SEAGRASS_LOCATION,
    new TallSeagrassBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_WATER_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const kelpState = registerBlock(
    KELP_LOCATION,
    new KelpBlock(BlockBehaviour.Properties.of(Material.WATER_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const kelpPlantState = registerBlock(
    KELP_PLANT_LOCATION,
    new KelpPlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_WATER_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const lilyPadState = registerBlock(
    LILY_PAD_LOCATION,
    new WaterlilyBlock(BlockBehaviour.Properties.of(Material.PLANT).instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const tallGrassState = registerBlock(
    TALL_GRASS_LOCATION,
    new DoublePlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const lilacState = registerBlock(
    LILAC_LOCATION,
    new DoublePlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const roseBushState = registerBlock(
    ROSE_BUSH_LOCATION,
    new DoublePlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const peonyState = registerBlock(
    PEONY_LOCATION,
    new DoublePlantBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const lilyOfTheValleyState = registerBlock(
    LILY_OF_THE_VALLEY_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const pumpkinState = registerBlock(
    PUMPKIN_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.VEGETABLE, MaterialColor.COLOR_ORANGE).strength(1.0).sound(SoundType.WOOD)),
  ).defaultBlockState();
  const melonState = registerBlock(
    MELON_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.VEGETABLE, MaterialColor.COLOR_LIGHT_GREEN).strength(1.0).sound(SoundType.WOOD)),
  ).defaultBlockState();
  const cactusState = registerBlock(
    CACTUS_LOCATION,
    new CactusBlock(BlockBehaviour.Properties.of(Material.CACTUS, MaterialColor.PLANT).randomTicks().strength(0.4).sound(SoundType.WOOD).noOcclusion()),
  ).defaultBlockState();
  const sugarCaneState = registerBlock(
    SUGAR_CANE_LOCATION,
    new SugarCaneBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const vineState = registerBlock(
    VINE_LOCATION,
    new VineBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const cocoaState = registerBlock(
    COCOA_LOCATION,
    new CocoaBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().instabreak().sound(SoundType.WOOD).noOcclusion()),
  ).defaultBlockState();
  ItemBlockRenderTypes.setFancy(true);
  ItemBlockRenderTypes.setRenderLayer(grassState.getBlock(), RenderType.cutoutMipped());
  ItemBlockRenderTypes.setRenderLayer(snowState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(iceState.getBlock(), RenderType.translucent());
  ItemBlockRenderTypes.setRenderLayer(obsidianState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(magmaBlockState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(waterState.getBlock(), RenderType.translucent());
  ItemBlockRenderTypes.setRenderLayer(grassPlantState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(fernState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(dandelionState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(poppyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(alliumState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(azureBluetState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(redTulipState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(orangeTulipState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(whiteTulipState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(pinkTulipState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(oxeyeDaisyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(cornflowerState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(oakSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(acaciaSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(spruceSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(birchSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(darkOakSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(jungleSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(largeFernState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(sweetBerryBushState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(brownMushroomState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(redMushroomState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(blueOrchidState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(deadBushState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(brownMushroomBlockState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(redMushroomBlockState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(mushroomStemState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(seagrassState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(tallSeagrassState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(kelpState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(kelpPlantState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilyPadState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(tallGrassState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilacState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(roseBushState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(peonyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilyOfTheValleyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(pumpkinState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(melonState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(cactusState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(sugarCaneState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(vineState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(cocoaState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setFluidRenderLayer(Fluids.WATER, RenderType.translucent());

  const blockStateById = new Array<BlockState>(ChunkBlockId.SNOW_BLOCK + 1);
  blockStateById[ChunkBlockId.AIR] = airState;
  blockStateById[ChunkBlockId.STONE] = stoneState;
  blockStateById[ChunkBlockId.WATER] = waterState;
  blockStateById[ChunkBlockId.BEDROCK] = bedrockState;
  blockStateById[ChunkBlockId.GRASS_BLOCK] = grassState;
  blockStateById[ChunkBlockId.DIRT] = dirtState;
  blockStateById[ChunkBlockId.SAND] = sandState;
  blockStateById[ChunkBlockId.GRAVEL] = gravelState;
  blockStateById[ChunkBlockId.SNOW] = snowState;
  blockStateById[ChunkBlockId.LAVA] = lavaState;
  blockStateById[ChunkBlockId.GRANITE] = graniteState;
  blockStateById[ChunkBlockId.DIORITE] = dioriteState;
  blockStateById[ChunkBlockId.ANDESITE] = andesiteState;
  blockStateById[ChunkBlockId.COARSE_DIRT] = coarseDirtState;
  blockStateById[ChunkBlockId.PODZOL] = podzolState;
  blockStateById[ChunkBlockId.MYCELIUM] = myceliumState;
  blockStateById[ChunkBlockId.TERRACOTTA] = terracottaState;
  blockStateById[ChunkBlockId.WHITE_TERRACOTTA] = whiteTerracottaState;
  blockStateById[ChunkBlockId.ORANGE_TERRACOTTA] = orangeTerracottaState;
  blockStateById[ChunkBlockId.MAGENTA_TERRACOTTA] = magentaTerracottaState;
  blockStateById[ChunkBlockId.LIGHT_BLUE_TERRACOTTA] = lightBlueTerracottaState;
  blockStateById[ChunkBlockId.YELLOW_TERRACOTTA] = yellowTerracottaState;
  blockStateById[ChunkBlockId.LIME_TERRACOTTA] = limeTerracottaState;
  blockStateById[ChunkBlockId.PINK_TERRACOTTA] = pinkTerracottaState;
  blockStateById[ChunkBlockId.GRAY_TERRACOTTA] = grayTerracottaState;
  blockStateById[ChunkBlockId.LIGHT_GRAY_TERRACOTTA] = lightGrayTerracottaState;
  blockStateById[ChunkBlockId.CYAN_TERRACOTTA] = cyanTerracottaState;
  blockStateById[ChunkBlockId.PURPLE_TERRACOTTA] = purpleTerracottaState;
  blockStateById[ChunkBlockId.BLUE_TERRACOTTA] = blueTerracottaState;
  blockStateById[ChunkBlockId.BROWN_TERRACOTTA] = brownTerracottaState;
  blockStateById[ChunkBlockId.GREEN_TERRACOTTA] = greenTerracottaState;
  blockStateById[ChunkBlockId.RED_TERRACOTTA] = redTerracottaState;
  blockStateById[ChunkBlockId.BLACK_TERRACOTTA] = blackTerracottaState;
  blockStateById[ChunkBlockId.SANDSTONE] = sandstoneState;
  blockStateById[ChunkBlockId.RED_SANDSTONE] = redSandstoneState;
  blockStateById[ChunkBlockId.PACKED_ICE] = packedIceState;
  blockStateById[ChunkBlockId.OBSIDIAN] = obsidianState;
  blockStateById[ChunkBlockId.MAGMA_BLOCK] = magmaBlockState;
  blockStateById[ChunkBlockId.RED_SAND] = redSandState;
  blockStateById[ChunkBlockId.ICE] = iceState;
  blockStateById[ChunkBlockId.SNOW_BLOCK] = snowBlockState;

  return {
    airState,
    blockStateById,
    blockLocations: GENERATED_BLOCK_LOCATIONS,
    spriteLocations: GENERATED_SPRITE_LOCATIONS,
  };
}
