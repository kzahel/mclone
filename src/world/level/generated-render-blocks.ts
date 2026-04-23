import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { ItemBlockRenderTypes } from "../../renderer/item-block-render-types";
import { RenderType } from "../../renderer/render-type";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { AirBlock } from "./block/air-block";
import { Block } from "./block/block";
import { BushBlock } from "./block/bush-block";
import { CactusBlock } from "./block/cactus-block";
import { DoublePlantBlock } from "./block/double-plant-block";
import { LeavesBlock } from "./block/leaves-block";
import { LiquidBlock } from "./block/liquid-block";
import { MushroomBlock } from "./block/mushroom-block";
import { RotatedPillarBlock } from "./block/rotated-pillar-block";
import { SnowLayerBlock } from "./block/snow-layer-block";
import { SnowyDirtBlock } from "./block/snowy-dirt-block";
import { SugarCaneBlock } from "./block/sugar-cane-block";
import { SweetBerryBushBlock } from "./block/sweet-berry-bush-block";
import { WaterlilyBlock } from "./block/waterlily-block";
import { SoundType } from "./block/sound-type";
import { BlockBehaviour } from "./block/state/block-behaviour";
import type { BlockState } from "./block/state/block-state";
import { Fluids } from "./material/fluids";
import { Material } from "./material/material";
import { MaterialColor } from "./material/material-color";

const STONE_LOCATION = new ResourceLocation("minecraft:stone");
const BEDROCK_LOCATION = new ResourceLocation("minecraft:bedrock");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const GRAVEL_LOCATION = new ResourceLocation("minecraft:gravel");
const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");
const SNOW_LOCATION = new ResourceLocation("minecraft:snow");
const OAK_LOG_LOCATION = new ResourceLocation("minecraft:oak_log");
const OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:oak_leaves");
const SPRUCE_LOG_LOCATION = new ResourceLocation("minecraft:spruce_log");
const SPRUCE_LEAVES_LOCATION = new ResourceLocation("minecraft:spruce_leaves");
const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const DANDELION_LOCATION = new ResourceLocation("minecraft:dandelion");
const OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:oak_sapling");
const SPRUCE_SAPLING_LOCATION = new ResourceLocation("minecraft:spruce_sapling");
const LARGE_FERN_LOCATION = new ResourceLocation("minecraft:large_fern");
const SWEET_BERRY_BUSH_LOCATION = new ResourceLocation("minecraft:sweet_berry_bush");
const BROWN_MUSHROOM_LOCATION = new ResourceLocation("minecraft:brown_mushroom");
const RED_MUSHROOM_LOCATION = new ResourceLocation("minecraft:red_mushroom");
const BLUE_ORCHID_LOCATION = new ResourceLocation("minecraft:blue_orchid");
const LILY_PAD_LOCATION = new ResourceLocation("minecraft:lily_pad");
const TALL_GRASS_LOCATION = new ResourceLocation("minecraft:tall_grass");
const LILAC_LOCATION = new ResourceLocation("minecraft:lilac");
const ROSE_BUSH_LOCATION = new ResourceLocation("minecraft:rose_bush");
const PEONY_LOCATION = new ResourceLocation("minecraft:peony");
const LILY_OF_THE_VALLEY_LOCATION = new ResourceLocation("minecraft:lily_of_the_valley");
const PUMPKIN_LOCATION = new ResourceLocation("minecraft:pumpkin");
const CACTUS_LOCATION = new ResourceLocation("minecraft:cactus");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");

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
  OAK_LOG_LOCATION,
  OAK_LEAVES_LOCATION,
  SPRUCE_LOG_LOCATION,
  SPRUCE_LEAVES_LOCATION,
  GRASS_LOCATION,
  FERN_LOCATION,
  DANDELION_LOCATION,
  OAK_SAPLING_LOCATION,
  SPRUCE_SAPLING_LOCATION,
  LARGE_FERN_LOCATION,
  SWEET_BERRY_BUSH_LOCATION,
  BROWN_MUSHROOM_LOCATION,
  RED_MUSHROOM_LOCATION,
  BLUE_ORCHID_LOCATION,
  LILY_PAD_LOCATION,
  TALL_GRASS_LOCATION,
  LILAC_LOCATION,
  ROSE_BUSH_LOCATION,
  PEONY_LOCATION,
  LILY_OF_THE_VALLEY_LOCATION,
  PUMPKIN_LOCATION,
  CACTUS_LOCATION,
  SUGAR_CANE_LOCATION,
] as const;

const GENERATED_SPRITE_LOCATIONS = [
  new ResourceLocation("minecraft:block/stone"),
  new ResourceLocation("minecraft:block/bedrock"),
  new ResourceLocation("minecraft:block/grass_block_top"),
  new ResourceLocation("minecraft:block/grass_block_side"),
  new ResourceLocation("minecraft:block/grass_block_side_overlay"),
  new ResourceLocation("minecraft:block/grass_block_snow"),
  new ResourceLocation("minecraft:block/dirt"),
  new ResourceLocation("minecraft:block/sand"),
  new ResourceLocation("minecraft:block/gravel"),
  new ResourceLocation("minecraft:block/water_still"),
  new ResourceLocation("minecraft:block/water_flow"),
  new ResourceLocation("minecraft:block/water_overlay"),
  new ResourceLocation("minecraft:block/lava_still"),
  new ResourceLocation("minecraft:block/lava_flow"),
  new ResourceLocation("minecraft:block/snow"),
  new ResourceLocation("minecraft:block/oak_log"),
  new ResourceLocation("minecraft:block/oak_log_top"),
  new ResourceLocation("minecraft:block/oak_leaves"),
  new ResourceLocation("minecraft:block/spruce_log"),
  new ResourceLocation("minecraft:block/spruce_log_top"),
  new ResourceLocation("minecraft:block/spruce_leaves"),
  new ResourceLocation("minecraft:block/grass"),
  new ResourceLocation("minecraft:block/fern"),
  new ResourceLocation("minecraft:block/dandelion"),
  new ResourceLocation("minecraft:block/oak_sapling"),
  new ResourceLocation("minecraft:block/spruce_sapling"),
  new ResourceLocation("minecraft:block/large_fern_bottom"),
  new ResourceLocation("minecraft:block/large_fern_top"),
  new ResourceLocation("minecraft:block/sweet_berry_bush_stage0"),
  new ResourceLocation("minecraft:block/sweet_berry_bush_stage1"),
  new ResourceLocation("minecraft:block/sweet_berry_bush_stage2"),
  new ResourceLocation("minecraft:block/sweet_berry_bush_stage3"),
  new ResourceLocation("minecraft:block/brown_mushroom"),
  new ResourceLocation("minecraft:block/red_mushroom"),
  new ResourceLocation("minecraft:block/blue_orchid"),
  new ResourceLocation("minecraft:block/lily_pad"),
  new ResourceLocation("minecraft:block/tall_grass_bottom"),
  new ResourceLocation("minecraft:block/tall_grass_top"),
  new ResourceLocation("minecraft:block/lilac_bottom"),
  new ResourceLocation("minecraft:block/lilac_top"),
  new ResourceLocation("minecraft:block/rose_bush_bottom"),
  new ResourceLocation("minecraft:block/rose_bush_top"),
  new ResourceLocation("minecraft:block/peony_bottom"),
  new ResourceLocation("minecraft:block/peony_top"),
  new ResourceLocation("minecraft:block/lily_of_the_valley"),
  new ResourceLocation("minecraft:block/pumpkin_top"),
  new ResourceLocation("minecraft:block/pumpkin_side"),
  new ResourceLocation("minecraft:block/cactus_side"),
  new ResourceLocation("minecraft:block/cactus_top"),
  new ResourceLocation("minecraft:block/cactus_bottom"),
  new ResourceLocation("minecraft:block/sugar_cane"),
] as const;

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
  registerBlock(
    OAK_LOG_LOCATION,
    new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD, MaterialColor.WOOD).strength(2.0).sound(SoundType.WOOD)),
  );
  registerBlock(
    OAK_LEAVES_LOCATION,
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
  const oakSaplingState = registerBlock(
    OAK_SAPLING_LOCATION,
    new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  const spruceSaplingState = registerBlock(
    SPRUCE_SAPLING_LOCATION,
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
  const cactusState = registerBlock(
    CACTUS_LOCATION,
    new CactusBlock(BlockBehaviour.Properties.of(Material.CACTUS, MaterialColor.PLANT).randomTicks().strength(0.4).sound(SoundType.WOOD).noOcclusion()),
  ).defaultBlockState();
  const sugarCaneState = registerBlock(
    SUGAR_CANE_LOCATION,
    new SugarCaneBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().randomTicks().instabreak().sound(SoundType.GRASS).noOcclusion()),
  ).defaultBlockState();
  registerBlock(
    GRANITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.DIRT).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  );
  registerBlock(
    DIORITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.QUARTZ).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  );
  registerBlock(
    ANDESITE_LOCATION,
    new Block(BlockBehaviour.Properties.of(Material.STONE, MaterialColor.STONE).requiresCorrectToolForDrops().strength(1.5, 6.0)),
  );

  ItemBlockRenderTypes.setFancy(true);
  ItemBlockRenderTypes.setRenderLayer(grassState.getBlock(), RenderType.cutoutMipped());
  ItemBlockRenderTypes.setRenderLayer(snowState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(waterState.getBlock(), RenderType.translucent());
  ItemBlockRenderTypes.setRenderLayer(grassPlantState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(fernState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(dandelionState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(oakSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(spruceSaplingState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(largeFernState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(sweetBerryBushState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(brownMushroomState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(redMushroomState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(blueOrchidState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilyPadState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(tallGrassState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilacState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(roseBushState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(peonyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(lilyOfTheValleyState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(pumpkinState.getBlock(), RenderType.solid());
  ItemBlockRenderTypes.setRenderLayer(cactusState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(sugarCaneState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setFluidRenderLayer(Fluids.WATER, RenderType.translucent());

  const blockStateById = new Array<BlockState>(10);
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

  return {
    airState,
    blockStateById,
    blockLocations: GENERATED_BLOCK_LOCATIONS,
    spriteLocations: GENERATED_SPRITE_LOCATIONS,
  };
}
