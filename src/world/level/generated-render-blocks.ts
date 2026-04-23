import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { ItemBlockRenderTypes } from "../../renderer/item-block-render-types";
import { RenderType } from "../../renderer/render-type";
import { ChunkBlockId } from "../../worldgen/chunk/chunk-block-buffer";
import { AirBlock } from "./block/air-block";
import { Block } from "./block/block";
import { LiquidBlock } from "./block/liquid-block";
import { SnowLayerBlock } from "./block/snow-layer-block";
import { SnowyDirtBlock } from "./block/snowy-dirt-block";
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
  const lavaState = registerBlock(
    LAVA_LOCATION,
    new LiquidBlock(Fluids.LAVA, BlockBehaviour.Properties.of(Material.LAVA).noCollission().randomTicks().strength(100.0).lightLevel(() => 15)),
  ).defaultBlockState();
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

  ItemBlockRenderTypes.setRenderLayer(grassState.getBlock(), RenderType.cutoutMipped());
  ItemBlockRenderTypes.setRenderLayer(snowState.getBlock(), RenderType.cutout());
  ItemBlockRenderTypes.setRenderLayer(waterState.getBlock(), RenderType.translucent());
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
