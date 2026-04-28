import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { BlockTags } from "../../../tags/block-tags";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { NoneFeatureConfiguration } from "./configurations/none-feature-configuration";

const CAVE_AIR_LOCATION = new ResourceLocation("minecraft:cave_air");
const COBBLESTONE_LOCATION = new ResourceLocation("minecraft:cobblestone");
const MOSSY_COBBLESTONE_LOCATION = new ResourceLocation("minecraft:mossy_cobblestone");
const CHEST_LOCATION = new ResourceLocation("minecraft:chest");
const SPAWNER_LOCATION = new ResourceLocation("minecraft:spawner");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class MonsterRoomFeature extends Feature<NoneFeatureConfiguration> {
  public override place(context: FeaturePlaceContext<NoneFeatureConfiguration>): boolean {
    const origin = context.origin();
    const random = context.random();
    const level = context.level();
    const airState = getRequiredState(CAVE_AIR_LOCATION);
    const cobblestoneState = getRequiredState(COBBLESTONE_LOCATION);
    const mossyCobblestoneState = getRequiredState(MOSSY_COBBLESTONE_LOCATION);
    const chestState = getRequiredState(CHEST_LOCATION);
    const spawnerState = getRequiredState(SPAWNER_LOCATION);
    const chestBlock = chestState.getBlock();
    const spawnerBlock = spawnerState.getBlock();
    const canReplace = (state: BlockState) => !state.is(BlockTags.FEATURES_CANNOT_REPLACE);
    const roomWidth = random.nextInt(2) + 2;
    const minX = -roomWidth - 1;
    const maxX = roomWidth + 1;
    const roomDepth = random.nextInt(2) + 2;
    const minZ = -roomDepth - 1;
    const maxZ = roomDepth + 1;
    let openings = 0;

    for (let offsetX = minX; offsetX <= maxX; offsetX++) {
      for (let offsetY = -1; offsetY <= 4; offsetY++) {
        for (let offsetZ = minZ; offsetZ <= maxZ; offsetZ++) {
          const pos = origin.offset(offsetX, offsetY, offsetZ);
          const solid = level.getBlockState(pos).getMaterial().isSolid();
          if (offsetY === -1 && !solid) {
            return false;
          }

          if (offsetY === 4 && !solid) {
            return false;
          }

          if (
            (offsetX === minX || offsetX === maxX || offsetZ === minZ || offsetZ === maxZ)
            && offsetY === 0
            && level.isEmptyBlock(pos)
            && level.isEmptyBlock(pos.above())
          ) {
            openings++;
          }
        }
      }
    }

    if (openings < 1 || openings > 5) {
      return false;
    }

    for (let offsetX = minX; offsetX <= maxX; offsetX++) {
      for (let offsetY = 3; offsetY >= -1; offsetY--) {
        for (let offsetZ = minZ; offsetZ <= maxZ; offsetZ++) {
          const pos = origin.offset(offsetX, offsetY, offsetZ);
          const state = level.getBlockState(pos);
          if (
            offsetX === minX
            || offsetY === -1
            || offsetZ === minZ
            || offsetX === maxX
            || offsetY === 4
            || offsetZ === maxZ
          ) {
            if (pos.getY() >= level.getMinBuildHeight() && !level.getBlockState(pos.below()).getMaterial().isSolid()) {
              level.setBlock(pos, airState, 2);
            } else if (state.getMaterial().isSolid() && !state.is(chestBlock)) {
              if (offsetY === -1 && random.nextInt(4) !== 0) {
                this.safeSetBlock(level, pos, mossyCobblestoneState, canReplace);
              } else {
                this.safeSetBlock(level, pos, cobblestoneState, canReplace);
              }
            }
          } else if (!state.is(chestBlock) && !state.is(spawnerBlock)) {
            this.safeSetBlock(level, pos, airState, canReplace);
          }
        }
      }
    }

    for (let chestIndex = 0; chestIndex < 2; chestIndex++) {
      for (let attempt = 0; attempt < 3; attempt++) {
        const chestPos = new BlockPos(
          origin.getX() + random.nextInt(roomWidth * 2 + 1) - roomWidth,
          origin.getY(),
          origin.getZ() + random.nextInt(roomDepth * 2 + 1) - roomDepth,
        );
        if (!level.isEmptyBlock(chestPos)) {
          continue;
        }

        let solidNeighbors = 0;
        for (const direction of Direction.Plane.HORIZONTAL) {
          if (level.getBlockState(chestPos.relative(direction)).getMaterial().isSolid()) {
            solidNeighbors++;
          }
        }

        if (solidNeighbors === 1) {
          this.safeSetBlock(level, chestPos, chestState, canReplace);
          break;
        }
      }
    }

    this.safeSetBlock(level, origin, spawnerState, canReplace);
    // Runtime: block-entity storage is not modeled yet, so the spawner block is placed without its mob data.
    return true;
  }
}
