import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { Heightmap } from "../heightmap";
import { FossilFeatureConfiguration } from "./configurations/fossil-feature-configuration";
import { StructureTemplate, type StructureBlockInfo } from "../../../world/level/levelgen/structure/templatesystem/structure-template";
import { StructurePlaceSettings } from "../../../world/level/levelgen/structure/templatesystem/structure-place-settings";
import { BoundingBox } from "../../../world/level/levelgen/structure/bounding-box";
import { Mirror } from "../../../world/level/block/mirror";
import { Rotation } from "../../../world/level/block/rotation";
import { BlockStateProperties } from "../../../world/level/block/state/properties/block-state-properties";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { Vec3i } from "../../../core/vec3i";
import { FOSSIL_TEMPLATE_DATA, type FossilTemplateData, type FossilTemplatePaletteEntry } from "./fossil-template-data";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");
const ROTATIONS = [Rotation.NONE, Rotation.CLOCKWISE_90, Rotation.CLOCKWISE_180, Rotation.COUNTERCLOCKWISE_90] as const;
const TEMPLATE_CACHE = new Map<string, StructureTemplate>();

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

function getPaletteState(entry: FossilTemplatePaletteEntry): BlockState {
  const block = getRequiredBlock(new ResourceLocation(entry.name));
  let state = block.defaultBlockState();
  if (entry.axis !== undefined) {
    const axis = Direction.Axis.byName(entry.axis);
    if (axis === undefined) {
      throw new Error(`Unknown fossil block axis ${entry.axis}`);
    }

    state = state.setValue(BlockStateProperties.AXIS, axis);
  }

  return state;
}

function createTemplate(templateData: FossilTemplateData): StructureTemplate {
  const paletteStates = templateData.palette.map(getPaletteState);
  const blocks: StructureBlockInfo[] = templateData.blocks.map((block) => ({
    pos: new BlockPos(block.pos[0], block.pos[1], block.pos[2]),
    state: paletteStates[block.state]!,
  }));
  return new StructureTemplate(new Vec3i(templateData.size[0], templateData.size[1], templateData.size[2]), blocks);
}

function getTemplate(location: ResourceLocation): StructureTemplate {
  const key = location.toString();
  let cached = TEMPLATE_CACHE.get(key);
  if (cached !== undefined) {
    return cached;
  }

  const templateData = FOSSIL_TEMPLATE_DATA[key as keyof typeof FOSSIL_TEMPLATE_DATA] as FossilTemplateData | undefined;
  if (templateData === undefined) {
    throw new Error(`Missing fossil template data for ${location}`);
  }

  cached = createTemplate(templateData);
  TEMPLATE_CACHE.set(key, cached);
  return cached;
}

function getChunkMinCoordinate(blockCoordinate: number): number {
  return Math.floor(blockCoordinate / 16) * 16;
}

function countEmptyCorners(level: WorldGenLevel, boundingBox: BoundingBox): number {
  const waterBlock = getRequiredBlock(WATER_LOCATION);
  const lavaBlock = getRequiredBlock(LAVA_LOCATION);
  let emptyCornerCount = 0;
  boundingBox.forAllCorners((pos) => {
    const state = level.getBlockState(pos);
    if (state.isAir() || state.is(waterBlock) || state.is(lavaBlock)) {
      emptyCornerCount++;
    }
  });
  return emptyCornerCount;
}

export class FossilFeature extends Feature<FossilFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<FossilFeatureConfiguration>): boolean {
    const random = context.random();
    const level = context.level();
    const origin = context.origin();
    const rotation = ROTATIONS[random.nextInt(ROTATIONS.length)]!;
    const config = context.config();
    const templateIndex = random.nextInt(config.fossilStructures.length);
    const fossilTemplate = getTemplate(config.fossilStructures[templateIndex]!);
    const overlayTemplate = getTemplate(config.overlayStructures[templateIndex]!);
    const chunkMinX = getChunkMinCoordinate(origin.getX());
    const chunkMinZ = getChunkMinCoordinate(origin.getZ());
    const boundingBox = new BoundingBox(
      chunkMinX,
      level.getMinBuildHeight(),
      chunkMinZ,
      chunkMinX + 15,
      level.getMaxBuildHeight(),
      chunkMinZ + 15,
    );
    const settings = new StructurePlaceSettings()
      .setRotation(rotation)
      .setBoundingBox(boundingBox)
      .setRandom(random);
    const rotatedSize = fossilTemplate.getSize(rotation);
    const offsetX = random.nextInt(16 - rotatedSize.getX());
    const offsetZ = random.nextInt(16 - rotatedSize.getZ());
    let minimumHeight = level.getMaxBuildHeight();

    for (let x = 0; x < rotatedSize.getX(); x++) {
      for (let z = 0; z < rotatedSize.getZ(); z++) {
        minimumHeight = Math.min(
          minimumHeight,
          level.getHeight(Heightmap.Types.OCEAN_FLOOR_WG, origin.getX() + x + offsetX, origin.getZ() + z + offsetZ),
        );
      }
    }

    const y = Math.max(minimumHeight - 15 - random.nextInt(10), level.getMinBuildHeight() + 10);
    const anchor = new BlockPos(origin.getX() + offsetX, y, origin.getZ() + offsetZ);
    const zeroPos = fossilTemplate.getZeroPositionWithTransform(anchor, Mirror.NONE, rotation);
    if (countEmptyCorners(level, fossilTemplate.getBoundingBox(settings, zeroPos)) > config.maxEmptyCornersAllowed) {
      return false;
    }

    settings.clearProcessors();
    for (const processor of config.fossilProcessors().list()) {
      settings.addProcessor(processor);
    }
    fossilTemplate.placeInWorld(level, zeroPos, zeroPos, settings, random, 4);

    settings.clearProcessors();
    for (const processor of config.overlayProcessors().list()) {
      settings.addProcessor(processor);
    }
    overlayTemplate.placeInWorld(level, zeroPos, zeroPos, settings, random, 4);
    return true;
  }
}
