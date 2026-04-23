import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { BlockTags } from "../../../tags/block-tags";
import { LightLayer } from "../../../world/level/light-layer";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Material } from "../../../world/level/material/material";
import { ChunkBlockId } from "../../chunk/chunk-block-buffer";
import { getOverworldSurfaceTopMaterial } from "../../surface/surface-builders";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { BlockStateConfiguration } from "./configurations/block-state-configuration";

const AIR_LOCATION = new ResourceLocation("minecraft:air");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const MYCELIUM_LOCATION = new ResourceLocation("minecraft:mycelium");
const ICE_LOCATION = new ResourceLocation("minecraft:ice");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function isBoundary(carveMask: readonly boolean[], x: number, y: number, z: number): boolean {
  const index = ((x * 16) + z) * 8 + y;
  return !carveMask[index]!
    && ((x < 15 && carveMask[(((x + 1) * 16) + z) * 8 + y]!)
      || (x > 0 && carveMask[(((x - 1) * 16) + z) * 8 + y]!)
      || (z < 15 && carveMask[((x * 16) + (z + 1)) * 8 + y]!)
      || (z > 0 && carveMask[((x * 16) + (z - 1)) * 8 + y]!)
      || (y < 7 && carveMask[index + 1]!)
      || (y > 0 && carveMask[index - 1]!));
}

export class LakeFeature extends Feature<BlockStateConfiguration> {
  public override place(context: FeaturePlaceContext<BlockStateConfiguration>): boolean {
    let origin = context.origin();
    const level = context.level();
    const random = context.random();
    const config = context.config();

    while (origin.getY() > level.getMinBuildHeight() + 5 && level.isEmptyBlock(origin)) {
      origin = origin.below();
    }

    if (origin.getY() <= level.getMinBuildHeight() + 4) {
      return false;
    }

    origin = origin.below(4);
    const airState = getRequiredState(AIR_LOCATION);
    const grassBlockState = getRequiredState(GRASS_BLOCK_LOCATION);
    const myceliumState = getRequiredState(MYCELIUM_LOCATION);
    const iceState = getRequiredState(ICE_LOCATION);
    const carveMask = new Array<boolean>(2048).fill(false);
    const ellipsoidCount = random.nextInt(4) + 4;

    for (let ellipsoid = 0; ellipsoid < ellipsoidCount; ellipsoid++) {
      const sizeX = (random.nextDouble() * 6.0) + 3.0;
      const sizeY = (random.nextDouble() * 4.0) + 2.0;
      const sizeZ = (random.nextDouble() * 6.0) + 3.0;
      const centerX = (random.nextDouble() * (16.0 - sizeX - 2.0)) + 1.0 + (sizeX / 2.0);
      const centerY = (random.nextDouble() * (8.0 - sizeY - 4.0)) + 2.0 + (sizeY / 2.0);
      const centerZ = (random.nextDouble() * (16.0 - sizeZ - 2.0)) + 1.0 + (sizeZ / 2.0);

      for (let x = 1; x < 15; x++) {
        for (let z = 1; z < 15; z++) {
          for (let y = 1; y < 7; y++) {
            const normalizedX = (x - centerX) / (sizeX / 2.0);
            const normalizedY = (y - centerY) / (sizeY / 2.0);
            const normalizedZ = (z - centerZ) / (sizeZ / 2.0);
            if ((normalizedX * normalizedX) + (normalizedY * normalizedY) + (normalizedZ * normalizedZ) < 1.0) {
              carveMask[((x * 16) + z) * 8 + y] = true;
            }
          }
        }
      }
    }

    for (let x = 0; x < 16; x++) {
      for (let z = 0; z < 16; z++) {
        for (let y = 0; y < 8; y++) {
          if (!isBoundary(carveMask, x, y, z)) {
            continue;
          }

          const sampleState = level.getBlockState(origin.offset(x, y, z));
          const material = sampleState.getMaterial();
          if (y >= 4 && material.isLiquid()) {
            return false;
          }

          if (y < 4 && !material.isSolid() && sampleState !== config.state) {
            return false;
          }
        }
      }
    }

    for (let x = 0; x < 16; x++) {
      for (let z = 0; z < 16; z++) {
        for (let y = 0; y < 8; y++) {
          if (!carveMask[((x * 16) + z) * 8 + y]!) {
            continue;
          }

          const pos = origin.offset(x, y, z);
          const upperHalf = y >= 4;
          level.setBlock(pos, upperHalf ? airState : config.state, 2);
          if (upperHalf) {
            level.getBlockTicks().scheduleTick(pos, airState.getBlock(), 0);
          }
        }
      }
    }

    for (let x = 0; x < 16; x++) {
      for (let z = 0; z < 16; z++) {
        for (let y = 4; y < 8; y++) {
          if (!carveMask[((x * 16) + z) * 8 + y]!) {
            continue;
          }

          const pos = origin.offset(x, y - 1, z);
          if (Feature.isDirt(level.getBlockState(pos)) && level.getBrightness(LightLayer.SKY, origin.offset(x, y, z)) > 0) {
            const topMaterial = getOverworldSurfaceTopMaterial(level.getBiome(pos));
            level.setBlock(pos, topMaterial === ChunkBlockId.MYCELIUM ? myceliumState : grassBlockState, 2);
          }
        }
      }
    }

    if (config.state.getMaterial() === Material.LAVA) {
      const baseStoneSource = context.chunkGenerator().getBaseStoneSource();
      for (let x = 0; x < 16; x++) {
        for (let z = 0; z < 16; z++) {
          for (let y = 0; y < 8; y++) {
            if (!isBoundary(carveMask, x, y, z) || (y >= 4 && random.nextInt(2) === 0)) {
              continue;
            }

            const pos = origin.offset(x, y, z);
            const state = level.getBlockState(pos);
            if (state.getMaterial().isSolid() && !state.is(BlockTags.LAVA_POOL_STONE_CANNOT_REPLACE)) {
              level.setBlock(pos, baseStoneSource.getBaseBlock(pos), 2);
            }
          }
        }
      }
    }

    if (config.state.getMaterial() === Material.WATER) {
      for (let x = 0; x < 16; x++) {
        for (let z = 0; z < 16; z++) {
          const pos = origin.offset(x, 4, z);
          if (level.getBiome(pos).shouldFreeze(level, pos, false)) {
            level.setBlock(pos, iceState, 2);
          }
        }
      }
    }

    return true;
  }
}
