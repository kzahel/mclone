import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { ChunkPos } from "../../../core/chunk-pos";
import type { StructureFeatureManager } from "../../../world/level/structure-feature-manager";
import { BoundingBox } from "../../../world/level/levelgen/structure/bounding-box";
import { StructurePiece } from "../../../world/level/levelgen/structure/structure-piece";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { Heightmap } from "../heightmap";
import type { WorldGenerator } from "../world-generator";
import type { SimpleRandomSource } from "../../prng/simple-random-source";

const SANDSTONE_LOCATION = new ResourceLocation("minecraft:sandstone");
const STONE_LOCATION = new ResourceLocation("minecraft:stone");
const ANDESITE_LOCATION = new ResourceLocation("minecraft:andesite");
const GRANITE_LOCATION = new ResourceLocation("minecraft:granite");
const DIORITE_LOCATION = new ResourceLocation("minecraft:diorite");
const SAND_LOCATION = new ResourceLocation("minecraft:sand");
const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");

function getRequiredBlockState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class BuriedTreasurePiece extends StructurePiece {
  private readonly sandstoneState = getRequiredBlockState(SANDSTONE_LOCATION);
  private readonly stoneState = getRequiredBlockState(STONE_LOCATION);
  private readonly andesiteState = getRequiredBlockState(ANDESITE_LOCATION);
  private readonly graniteState = getRequiredBlockState(GRANITE_LOCATION);
  private readonly dioriteState = getRequiredBlockState(DIORITE_LOCATION);
  private readonly sandState = getRequiredBlockState(SAND_LOCATION);
  private readonly waterBlock = getRequiredBlockState(WATER_LOCATION).getBlock();
  private readonly lavaBlock = getRequiredBlockState(LAVA_LOCATION).getBlock();

  public constructor(pos: BlockPos) {
    super(BoundingBox.fromBlockPos(pos));
  }

  public override postProcess(
    level: WorldGenLevel,
    structureManager: StructureFeatureManager,
    generator: WorldGenerator,
    random: SimpleRandomSource,
    box: BoundingBox,
    chunkPos: ChunkPos,
    pos: BlockPos,
  ): boolean {
    void structureManager;
    void generator;
    void chunkPos;
    void pos;

    const oceanFloorY = level.getHeight(Heightmap.Types.OCEAN_FLOOR_WG, this.boundingBox.minX(), this.boundingBox.minZ());
    const cursor = new BlockPos.MutableBlockPos(this.boundingBox.minX(), oceanFloorY, this.boundingBox.minZ());

    while (cursor.getY() > level.getMinBuildHeight()) {
      const currentState = level.getBlockState(cursor);
      const belowState = level.getBlockState(cursor.below());
      if (this.isTreasureBase(belowState)) {
        const fillState = !currentState.isAir() && !this.isLiquid(currentState) ? currentState : this.sandState;

        for (const direction of Direction.values()) {
          const neighborPos = cursor.relative(direction);
          const neighborState = level.getBlockState(neighborPos);
          if (!neighborState.isAir() && !this.isLiquid(neighborState)) {
            continue;
          }

          const belowNeighborPos = neighborPos.below();
          const belowNeighborState = level.getBlockState(belowNeighborPos);
          if ((belowNeighborState.isAir() || this.isLiquid(belowNeighborState)) && direction !== Direction.UP) {
            level.setBlock(neighborPos, belowState, 3);
          } else {
            level.setBlock(neighborPos, fillState, 3);
          }
        }

        this.boundingBox = BoundingBox.fromBlockPos(cursor);
        return this.createChest(level, box, random, cursor);
      }

      cursor.move(0, -1, 0);
    }

    return false;
  }

  private isTreasureBase(state: BlockState): boolean {
    return (
      state === this.sandstoneState
      || state === this.stoneState
      || state === this.andesiteState
      || state === this.graniteState
      || state === this.dioriteState
    );
  }

  private isLiquid(state: BlockState): boolean {
    const block = state.getBlock();
    return block === this.waterBlock || block === this.lavaBlock;
  }
}
