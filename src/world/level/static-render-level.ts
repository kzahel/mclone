import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { Direction } from "../../core/direction";
import { type BlockAndTintGetter } from "./block-and-tint-getter";
import { LightLayer } from "./light-layer";
import { LevelChunk } from "./chunk/level-chunk";
import { type BlockState } from "./block/state/block-state";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export class StaticRenderLevel implements BlockAndTintGetter {
  private readonly chunks = new Map<string, LevelChunk>();

  public constructor(
    private readonly airState: BlockState,
    private readonly skyLight = 15,
    private readonly blockLight = 15,
    private readonly minBuildHeight = 0,
    private readonly height = 16,
  ) {}

  public setBlock(pos: BlockPos, state: BlockState): void {
    this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()))!.setBlockState(pos, state);
  }

  public getChunk(chunkX: number, chunkZ: number, create = true): LevelChunk | null {
    const key = chunkKey(chunkX, chunkZ);
    const existing = this.chunks.get(key);
    if (existing) {
      return existing;
    }

    if (!create) {
      return null;
    }

    const created = new LevelChunk(chunkX, chunkZ, this.airState);
    this.chunks.set(key, created);
    return created;
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()))!.getBlockState(pos);
  }

  public getMaxLightLevel(): number {
    return 15;
  }

  public getShade(direction: Direction, shade: boolean): number {
    if (!shade) {
      return 1.0;
    }

    switch (direction) {
      case Direction.DOWN:
        return 0.5;
      case Direction.UP:
        return 1.0;
      case Direction.NORTH:
      case Direction.SOUTH:
        return 0.8;
      case Direction.WEST:
      case Direction.EAST:
        return 0.6;
      default:
        return 1.0;
    }
  }

  public getBrightness(layer: LightLayer, _pos: BlockPos): number {
    return layer === LightLayer.SKY ? this.skyLight : this.blockLight;
  }

  public getBlockTint(_pos: BlockPos, _resolver?: unknown): number {
    return -1;
  }

  public getSectionsCount(): number {
    return Math.trunc(this.height / 16);
  }

  public getMinSection(): number {
    return SectionPos.blockToSectionCoord(this.minBuildHeight);
  }

  public getMinBuildHeight(): number {
    return this.minBuildHeight;
  }

  public getHeight(): number {
    return this.height;
  }
}
