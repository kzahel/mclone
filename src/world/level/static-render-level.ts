import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { Direction } from "../../core/direction";
import { type BlockAndTintGetter } from "./block-and-tint-getter";
import type { ColorResolver } from "./color-resolver";
import { LightLayer } from "./light-layer";
import { LevelChunk } from "./chunk/level-chunk";
import { type BlockState } from "./block/state/block-state";
import { Vec3 } from "../phys/vec3";
import type { FluidState } from "./material/fluid-state";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { WorldGenLevel } from "./world-gen-level";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export class StaticRenderLevel implements BlockAndTintGetter, WorldGenLevel {
  private readonly chunks = new Map<string, LevelChunk>();

  public constructor(
    protected readonly airState: BlockState,
    private readonly skyLight = 15,
    private readonly blockLight = 15,
    private readonly minBuildHeight = 0,
    private readonly height = 16,
    private readonly skyColor = new Vec3(0, 128 / 255, 0),
    private readonly clearColorScale = 1,
    private readonly ambientLight = 0,
  ) {}

  public setBlock(pos: BlockPos, state: BlockState, _flags = 3): boolean {
    const chunk = this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()));
    if (chunk === null) {
      return false;
    }

    chunk.setBlockState(pos, state);
    return true;
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

  public setChunk(chunk: LevelChunk): void {
    this.chunks.set(chunkKey(chunk.chunkX, chunk.chunkZ), chunk);
  }

  public removeChunk(chunkX: number, chunkZ: number): LevelChunk | undefined {
    const key = chunkKey(chunkX, chunkZ);
    const chunk = this.chunks.get(key);
    this.chunks.delete(key);
    return chunk;
  }

  public hasChunk(chunkX: number, chunkZ: number): boolean {
    return this.chunks.has(chunkKey(chunkX, chunkZ));
  }

  public getLoadedChunks(): readonly LevelChunk[] {
    return [...this.chunks.values()];
  }

  public getLoadedChunkCount(): number {
    return this.chunks.size;
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()))?.getBlockState(pos) ?? this.airState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public isEmptyBlock(pos: BlockPos): boolean {
    return this.getBlockState(pos).isAir();
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

  public getBlockTint(_pos: BlockPos, _resolver?: ColorResolver): number {
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

  public getHeight(): number;
  public getHeight(type: Heightmap.Types, x: number, z: number): number;
  public getHeight(type?: Heightmap.Types, x?: number, z?: number): number {
    if (type === undefined || x === undefined || z === undefined) {
      return this.height;
    }

    for (let y = this.getMaxBuildHeight() - 1; y >= this.getMinBuildHeight(); y--) {
      const pos = new BlockPos(x, y, z);
      if (type.isOpaque(this.getBlockState(pos))) {
        return y + 1;
      }
    }

    return this.getMinBuildHeight();
  }

  public getMaxBuildHeight(): number {
    return this.minBuildHeight + this.height;
  }

  public getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos {
    return new BlockPos(pos.getX(), this.getHeight(type, pos.getX(), pos.getZ()), pos.getZ());
  }

  public getSkyColor(_pos: Vec3, _partialTick: number): Vec3 {
    return this.skyColor;
  }

  public getClearColorScale(): number {
    return this.clearColorScale;
  }

  public getAmbientLight(): number {
    return this.ambientLight;
  }
}
