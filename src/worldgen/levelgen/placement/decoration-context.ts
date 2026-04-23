import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { Heightmap } from "../heightmap";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";

export class DecorationContext {
  public constructor(
    private readonly level: WorldGenLevel,
    private readonly chunkGenerator: NoiseBasedChunkGenerator,
  ) {}

  public getHeight(type: Heightmap.Types, x: number, z: number): number {
    return this.level.getHeight(type, x, z);
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.level.getBlockState(pos);
  }

  public getMinBuildHeight(): number {
    return this.level.getMinBuildHeight();
  }

  public getMaxBuildHeight(): number {
    return this.level.getMaxBuildHeight();
  }

  public getGenDepth(): number {
    return this.getMaxBuildHeight() - this.getMinBuildHeight();
  }

  public getLevel(): WorldGenLevel {
    return this.level;
  }

  public getChunkGenerator(): NoiseBasedChunkGenerator {
    return this.chunkGenerator;
  }
}
