import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { type BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { LightLayer } from "../../world/level/light-layer";
import { StaticRenderLevel } from "../../world/level/static-render-level";
import { LevelChunk } from "../../world/level/chunk/level-chunk";
import { type BlockState } from "../../world/level/block/state/block-state";
import { Direction } from "../../core/direction";

export class RenderChunkRegion implements BlockAndTintGetter {
  public static createIfNotEmpty(
    level: StaticRenderLevel,
    from: BlockPos,
    to: BlockPos,
    padding: number,
  ): RenderChunkRegion | null {
    const minChunkX = SectionPos.blockToSectionCoord(from.getX() - padding);
    const minChunkZ = SectionPos.blockToSectionCoord(from.getZ() - padding);
    const maxChunkX = SectionPos.blockToSectionCoord(to.getX() + padding);
    const maxChunkZ = SectionPos.blockToSectionCoord(to.getZ() + padding);
    const chunks = new Array<Array<LevelChunk>>(maxChunkX - minChunkX + 1);

    for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
      const row = new Array<LevelChunk>(maxChunkZ - minChunkZ + 1);
      for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
        row[chunkZ - minChunkZ] = level.getChunk(chunkX, chunkZ)!;
      }

      chunks[chunkX - minChunkX] = row;
    }

    if (RenderChunkRegion.isAllEmpty(from, to, minChunkX, minChunkZ, chunks)) {
      return null;
    }

    return new RenderChunkRegion(level, minChunkX, minChunkZ, chunks, from.offset(-1, -1, -1), to.offset(1, 1, 1));
  }

  public static isAllEmpty(
    from: BlockPos,
    to: BlockPos,
    minChunkX: number,
    minChunkZ: number,
    chunks: readonly (readonly LevelChunk[])[],
  ): boolean {
    for (let chunkX = SectionPos.blockToSectionCoord(from.getX()); chunkX <= SectionPos.blockToSectionCoord(to.getX()); chunkX++) {
      for (let chunkZ = SectionPos.blockToSectionCoord(from.getZ()); chunkZ <= SectionPos.blockToSectionCoord(to.getZ()); chunkZ++) {
        const chunk = chunks[chunkX - minChunkX]![chunkZ - minChunkZ]!;
        if (!chunk.isYSpaceEmpty(from.getY(), to.getY())) {
          return false;
        }
      }
    }

    return true;
  }

  protected readonly xLength: number;
  protected readonly yLength: number;
  protected readonly zLength: number;
  protected readonly blockStates: BlockState[];

  public constructor(
    protected readonly level: StaticRenderLevel,
    protected readonly centerX: number,
    protected readonly centerZ: number,
    protected readonly chunks: readonly (readonly LevelChunk[])[],
    protected readonly start: BlockPos,
    end: BlockPos,
  ) {
    this.xLength = end.getX() - start.getX() + 1;
    this.yLength = end.getY() - start.getY() + 1;
    this.zLength = end.getZ() - start.getZ() + 1;
    this.blockStates = new Array<BlockState>(this.xLength * this.yLength * this.zLength);

    for (let z = start.getZ(); z <= end.getZ(); z++) {
      for (let y = start.getY(); y <= end.getY(); y++) {
        for (let x = start.getX(); x <= end.getX(); x++) {
          const pos = new BlockPos(x, y, z);
          const chunkX = SectionPos.blockToSectionCoord(x) - this.centerX;
          const chunkZ = SectionPos.blockToSectionCoord(z) - this.centerZ;
          const chunk = this.chunks[chunkX]![chunkZ]!;
          this.blockStates[this.index(pos)] = chunk.getBlockState(pos);
        }
      }
    }
  }

  protected index(pos: BlockPos): number {
    return this.indexAt(pos.getX(), pos.getY(), pos.getZ());
  }

  protected indexAt(x: number, y: number, z: number): number {
    const localX = x - this.start.getX();
    const localY = y - this.start.getY();
    const localZ = z - this.start.getZ();
    return (localZ * this.xLength * this.yLength) + (localY * this.xLength) + localX;
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.blockStates[this.index(pos)]!;
  }

  public getMaxLightLevel(): number {
    return this.level.getMaxLightLevel();
  }

  public getShade(direction: Direction, shade: boolean): number {
    return this.level.getShade(direction, shade);
  }

  public getBrightness(layer: LightLayer, pos: BlockPos): number {
    return this.level.getBrightness(layer, pos);
  }

  public getBlockTint(pos: BlockPos, resolver?: unknown): number {
    return this.level.getBlockTint(pos, resolver);
  }

  public getMinBuildHeight(): number {
    return this.level.getMinBuildHeight();
  }

  public getHeight(): number {
    return this.level.getHeight();
  }
}
