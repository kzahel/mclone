import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { BlockTags } from "../../../tags/block-tags";
import { Material } from "../../../world/level/material/material";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { BlockStateProperties } from "../../../world/level/block/state/properties/block-state-properties";
import type { LevelSimulatedReader } from "../../../world/level/level-simulated-reader";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { BitSetDiscreteVoxelShape } from "../../../world/phys/shapes/bit-set-discrete-voxel-shape";
import type { DiscreteVoxelShape } from "../../../world/phys/shapes/discrete-voxel-shape";
import { BoundingBox } from "../../../world/level/levelgen/structure/bounding-box";
import { StructureTemplate } from "../../../world/level/levelgen/structure/templatesystem/structure-template";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { Feature } from "./feature";
import type { TreeConfiguration } from "./configurations/tree-configuration";
import type { FeaturePlaceContext } from "./feature-place-context";

const TREE_BLOCK_UPDATE_FLAGS = 19;

type BlockPosMap = Map<bigint, BlockPos>;

function copyPos(pos: BlockPos): BlockPos {
  return new BlockPos(pos.getX(), pos.getY(), pos.getZ());
}

function addPos(map: BlockPosMap, pos: BlockPos): void {
  map.set(pos.asLong(), copyPos(pos));
}

function hasLocation(state: BlockState, location: string): boolean {
  return state.getBlock().getLocation()?.toString() === location;
}

export class TreeFeature extends Feature<TreeConfiguration> {
  public static isFree(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return TreeFeature.validTreePos(level, pos) || level.isStateAtPosition(pos, (state) => state.is(BlockTags.LOGS));
  }

  private static isVine(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => hasLocation(state, "minecraft:vine"));
  }

  private static isBlockWater(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => state.getMaterial() === Material.WATER);
  }

  public static isAirOrLeaves(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => state.isAir() || state.is(BlockTags.LEAVES));
  }

  private static isReplaceablePlant(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return level.isStateAtPosition(pos, (state) => state.getMaterial() === Material.REPLACEABLE_PLANT);
  }

  private static setBlockKnownShape(level: WorldGenLevel, pos: BlockPos, state: BlockState): void {
    level.setBlock(pos, state, TREE_BLOCK_UPDATE_FLAGS);
  }

  public static validTreePos(level: LevelSimulatedReader, pos: BlockPos): boolean {
    return TreeFeature.isAirOrLeaves(level, pos) || TreeFeature.isReplaceablePlant(level, pos) || TreeFeature.isBlockWater(level, pos);
  }

  private doPlace(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    pos: BlockPos,
    trunkConsumer: (pos: BlockPos, state: BlockState) => void,
    leafConsumer: (pos: BlockPos, state: BlockState) => void,
    config: TreeConfiguration,
  ): boolean {
    const treeHeight = config.trunkPlacer.getTreeHeight(random);
    const foliageHeight = config.foliagePlacer.foliageHeight(random, treeHeight, config);
    const trunkHeight = treeHeight - foliageHeight;
    const foliageRadius = config.foliagePlacer.foliageRadius(random, trunkHeight);
    if (pos.getY() < level.getMinBuildHeight() + 1 || pos.getY() + treeHeight + 1 > level.getMaxBuildHeight()) {
      return false;
    }

    if (!config.saplingProvider.getState(random, pos).canSurvive(level, pos)) {
      return false;
    }

    const minClippedHeight = config.minimumSize.minClippedHeight();
    const maxFreeTreeHeight = this.getMaxFreeTreeHeight(level, treeHeight, pos, config);
    if (maxFreeTreeHeight >= treeHeight || (minClippedHeight !== undefined && maxFreeTreeHeight >= minClippedHeight)) {
      const attachments = config.trunkPlacer.placeTrunk(level, trunkConsumer, random, maxFreeTreeHeight, pos, config);
      for (const attachment of attachments) {
        config.foliagePlacer.createFoliage(level, leafConsumer, random, config, maxFreeTreeHeight, attachment, foliageHeight, foliageRadius);
      }

      return true;
    }

    return false;
  }

  private getMaxFreeTreeHeight(level: LevelSimulatedReader, treeHeight: number, pos: BlockPos, config: TreeConfiguration): number {
    const mutable = new BlockPos.MutableBlockPos();

    for (let y = 0; y <= treeHeight + 1; y++) {
      const sizeAtHeight = config.minimumSize.getSizeAtHeight(treeHeight, y);

      for (let x = -sizeAtHeight; x <= sizeAtHeight; x++) {
        for (let z = -sizeAtHeight; z <= sizeAtHeight; z++) {
          mutable.setWithOffset(pos, x, y, z);
          if (!TreeFeature.isFree(level, mutable) || (!config.ignoreVines && TreeFeature.isVine(level, mutable))) {
            return y - 2;
          }
        }
      }
    }

    return treeHeight;
  }

  public override place(context: FeaturePlaceContext<TreeConfiguration>): boolean {
    const level = context.level();
    const random = context.random();
    const pos = context.origin();
    const config = context.config();
    // TypeScript: BlockPos collections are keyed by packed longs to preserve Java value semantics.
    const trunkPositions: BlockPosMap = new Map();
    const leafPositions: BlockPosMap = new Map();
    const decoratorPositions: BlockPosMap = new Map();
    const trunkConsumer = (consumerPos: BlockPos, state: BlockState): void => {
      addPos(trunkPositions, consumerPos);
      level.setBlock(consumerPos, state, TREE_BLOCK_UPDATE_FLAGS);
    };
    const leafConsumer = (consumerPos: BlockPos, state: BlockState): void => {
      addPos(leafPositions, consumerPos);
      level.setBlock(consumerPos, state, TREE_BLOCK_UPDATE_FLAGS);
    };
    const decoratorConsumer = (consumerPos: BlockPos, state: BlockState): void => {
      addPos(decoratorPositions, consumerPos);
      level.setBlock(consumerPos, state, TREE_BLOCK_UPDATE_FLAGS);
    };
    const placed = this.doPlace(level, random, pos, trunkConsumer, leafConsumer, config);

    if (placed && (trunkPositions.size > 0 || leafPositions.size > 0)) {
      if (config.decorators.length > 0) {
        const sortedTrunks = [...trunkPositions.values()].sort((left, right) => left.getY() - right.getY());
        const sortedLeaves = [...leafPositions.values()].sort((left, right) => left.getY() - right.getY());
        for (const decorator of config.decorators) {
          decorator.place(level, decoratorConsumer, random, sortedTrunks, sortedLeaves);
        }
      }

      const box = BoundingBox.encapsulatingPositions([
        ...trunkPositions.values(),
        ...leafPositions.values(),
        ...decoratorPositions.values(),
      ]);
      if (box !== undefined) {
        const shape = TreeFeature.updateLeaves(level, box, trunkPositions, decoratorPositions);
        StructureTemplate.updateShapeAtEdge(level, 3, shape, box.minX(), box.minY(), box.minZ());
        return true;
      }
    }

    return false;
  }

  private static updateLeaves(level: WorldGenLevel, box: BoundingBox, trunkPositions: BlockPosMap, decoratorPositions: BlockPosMap): DiscreteVoxelShape {
    const distanceLevels: BlockPosMap[] = [];
    const shape = new BitSetDiscreteVoxelShape(box.getXSpan(), box.getYSpan(), box.getZSpan());

    for (let distance = 0; distance < 6; distance++) {
      distanceLevels.push(new Map());
    }

    const mutable = new BlockPos.MutableBlockPos();

    for (const pos of decoratorPositions.values()) {
      if (box.isInside(pos)) {
        shape.fill(pos.getX() - box.minX(), pos.getY() - box.minY(), pos.getZ() - box.minZ());
      }
    }

    for (const pos of trunkPositions.values()) {
      if (box.isInside(pos)) {
        shape.fill(pos.getX() - box.minX(), pos.getY() - box.minY(), pos.getZ() - box.minZ());
      }

      for (const direction of Direction.values()) {
        mutable.setWithOffset(pos, direction);
        if (!trunkPositions.has(mutable.asLong())) {
          const state = level.getBlockState(mutable);
          if (state.hasProperty(BlockStateProperties.DISTANCE)) {
            addPos(distanceLevels[0]!, mutable);
            TreeFeature.setBlockKnownShape(level, mutable, state.setValue(BlockStateProperties.DISTANCE, 1));
            if (box.isInside(mutable)) {
              shape.fill(mutable.getX() - box.minX(), mutable.getY() - box.minY(), mutable.getZ() - box.minZ());
            }
          }
        }
      }
    }

    for (let distance = 1; distance < 6; distance++) {
      const previous = distanceLevels[distance - 1]!;
      const current = distanceLevels[distance]!;

      for (const pos of previous.values()) {
        if (box.isInside(pos)) {
          shape.fill(pos.getX() - box.minX(), pos.getY() - box.minY(), pos.getZ() - box.minZ());
        }

        for (const direction of Direction.values()) {
          mutable.setWithOffset(pos, direction);
          const key = mutable.asLong();
          if (!previous.has(key) && !current.has(key)) {
            const state = level.getBlockState(mutable);
            if (state.hasProperty(BlockStateProperties.DISTANCE)) {
              const stateDistance = state.getValue(BlockStateProperties.DISTANCE);
              if (stateDistance > distance + 1) {
                const updatedState = state.setValue(BlockStateProperties.DISTANCE, distance + 1);
                TreeFeature.setBlockKnownShape(level, mutable, updatedState);
                if (box.isInside(mutable)) {
                  shape.fill(mutable.getX() - box.minX(), mutable.getY() - box.minY(), mutable.getZ() - box.minZ());
                }

                addPos(current, mutable);
              }
            }
          }
        }
      }
    }

    return shape;
  }
}
