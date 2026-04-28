import { BlockPos } from "../../../../core/block-pos";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import { BoundingBox } from "./bounding-box";
import type { WorldGenLevel } from "../../world-gen-level";
import type { StructureFeatureManager } from "../../structure-feature-manager";
import type { WorldGenerator } from "../../../../worldgen/levelgen/world-generator";
import type { BlockState } from "../../block/state/block-state";
import type { Block } from "../../block/block";
import type { SimpleRandomSource } from "../../../../worldgen/prng/simple-random-source";
import { ChunkPos } from "../../../../core/chunk-pos";

const CHEST_LOCATION = new ResourceLocation("minecraft:chest");

function getRequiredBlockState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export abstract class StructurePiece {
  public constructor(
    protected boundingBox: BoundingBox,
    protected readonly genDepth = 0,
  ) {}

  public getBoundingBox(): BoundingBox {
    return this.boundingBox;
  }

  public getGenDepth(): number {
    return this.genDepth;
  }

  public move(x: number, y: number, z: number): void {
    this.boundingBox.move(x, y, z);
  }

  protected createChest(
    level: WorldGenLevel,
    box: BoundingBox,
    _random: SimpleRandomSource,
    pos: BlockPos,
  ): boolean {
    if (!box.isInside(pos)) {
      return false;
    }

    // TypeScript: chest loot/block-entity state is still deferred; place the chest block itself for parity scaffolding.
    return level.setBlock(pos, getRequiredBlockState(CHEST_LOCATION), 2);
  }

  public abstract postProcess(
    level: WorldGenLevel,
    structureManager: StructureFeatureManager,
    generator: WorldGenerator,
    random: SimpleRandomSource,
    box: BoundingBox,
    chunkPos: ChunkPos,
    pos: BlockPos,
  ): boolean;
}
