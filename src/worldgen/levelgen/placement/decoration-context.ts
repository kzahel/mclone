import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import { Heightmap } from "../heightmap";
import { GenerationStep } from "../generation-step";
import type { WorldGenerator } from "../world-generator";

export class DecorationContext {
  public constructor(
    private readonly level: WorldGenLevel,
    private readonly chunkGenerator: WorldGenerator,
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

  public getCarvingMask(step: GenerationStep.Carving, chunkX: number, chunkZ: number): Uint8Array | undefined {
    return this.level.getCarvingMask?.(step, chunkX, chunkZ);
  }

  public getChunkGenerator(): WorldGenerator {
    return this.chunkGenerator;
  }
}
