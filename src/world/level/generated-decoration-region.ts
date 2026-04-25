import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { Biome } from "../../worldgen/biome/biome";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import { LightLayer } from "./light-layer";
import { serializeBlockTickTarget, serializeFluidTickTarget } from "./scheduled-tick";
import { RecordingTickAccess, type TickAccess } from "./tick-access";
import type { Block } from "./block/block";
import type { BlockState } from "./block/state/block-state";
import type { LevelChunk } from "./chunk/level-chunk";
import type { Fluid } from "./material/fluid";
import type { FluidState } from "./material/fluid-state";
import type { WorldGenLevel } from "./world-gen-level";
import type { GeneratedRenderLevel } from "./generated-render-level";

export const FEATURES_CHUNK_DEPENDENCY_RADIUS = 8;
export const FEATURES_WRITE_RADIUS_CUTOFF = 1;

// Runtime: WorldGenRegion-style FEATURES wrapper over GeneratedRenderLevel without exposing on-demand decoration.
export class GeneratedDecorationRegion implements WorldGenLevel {
  private readonly blockTicks: TickAccess<Block> = new RecordingTickAccess((pos, target, delay) => {
    if (!this.ensureCanWrite(pos)) {
      return;
    }

    this.getChunkForPos(pos).recordBlockTick(pos, serializeBlockTickTarget(target), delay);
  });
  private readonly liquidTicks: TickAccess<Fluid> = new RecordingTickAccess((pos, target, delay) => {
    if (!this.ensureCanWrite(pos)) {
      return;
    }

    this.getChunkForPos(pos).recordLiquidTick(pos, serializeFluidTickTarget(target), delay);
  });

  public constructor(
    private readonly level: GeneratedRenderLevel,
    private readonly centerChunkX: number,
    private readonly centerChunkZ: number,
    private readonly dependencyRadius = FEATURES_CHUNK_DEPENDENCY_RADIUS,
    private readonly writeRadiusCutoff = FEATURES_WRITE_RADIUS_CUTOFF,
  ) {}

  public setBlock(pos: BlockPos, state: BlockState, _flags = 3): boolean {
    if (!this.ensureCanWrite(pos)) {
      return false;
    }

    this.getChunkForPos(pos).setBlockState(pos, state);
    return true;
  }

  public getBlockState(pos: BlockPos): BlockState {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    this.ensureWithinDependencyWindow(chunkX, chunkZ);
    return this.level.getAuthorityBlockState(pos);
  }

  public getFluidState(pos: BlockPos): FluidState {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    this.ensureWithinDependencyWindow(chunkX, chunkZ);
    return this.level.getAuthorityFluidState(pos);
  }

  public isEmptyBlock(pos: BlockPos): boolean {
    return this.getBlockState(pos).isAir();
  }

  public isStateAtPosition(pos: BlockPos, predicate: (state: BlockState) => boolean): boolean {
    return predicate(this.getBlockState(pos));
  }

  public getMaxLightLevel(): number {
    return this.level.getMaxLightLevel();
  }

  public getHeight(type: Heightmap.Types, x: number, z: number): number {
    const chunkX = SectionPos.blockToSectionCoord(x);
    const chunkZ = SectionPos.blockToSectionCoord(z);
    this.ensureWithinDependencyWindow(chunkX, chunkZ);
    return this.level.getAuthorityHeight(type, x, z);
  }

  public getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos {
    return new BlockPos(pos.getX(), this.getHeight(type, pos.getX(), pos.getZ()), pos.getZ());
  }

  public getMinBuildHeight(): number {
    return this.level.getMinBuildHeight();
  }

  public getMaxBuildHeight(): number {
    return this.level.getMaxBuildHeight();
  }

  public getBiome(pos: BlockPos): Biome {
    return this.level.getBiome(pos);
  }

  public getBrightness(layer: LightLayer, pos: BlockPos): number {
    return this.level.getBrightness(layer, pos);
  }

  public getBlockTicks(): TickAccess<Block> {
    return this.blockTicks;
  }

  public getLiquidTicks(): TickAccess<Fluid> {
    return this.liquidTicks;
  }

  private getChunkForPos(pos: BlockPos): LevelChunk {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    return this.getChunk(chunkX, chunkZ);
  }

  private getChunk(chunkX: number, chunkZ: number): LevelChunk {
    this.ensureWithinDependencyWindow(chunkX, chunkZ);
    const chunk = this.level.getAuthorityChunk(chunkX, chunkZ);
    if (chunk === null) {
      throw new Error(
        `Decoration region missing authority chunk (${chunkX.toString()}, ${chunkZ.toString()}) for center (${this.centerChunkX.toString()}, ${this.centerChunkZ.toString()})`,
      );
    }

    return chunk;
  }

  private ensureWithinDependencyWindow(chunkX: number, chunkZ: number): void {
    if (
      Math.abs(chunkX - this.centerChunkX) > this.dependencyRadius
      || Math.abs(chunkZ - this.centerChunkZ) > this.dependencyRadius
    ) {
      throw new Error(
        `Decoration region read outside FEATURES dependency window: center (${this.centerChunkX.toString()}, ${this.centerChunkZ.toString()}), requested (${chunkX.toString()}, ${chunkZ.toString()})`,
      );
    }
  }

  private ensureCanWrite(pos: BlockPos): boolean {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    this.ensureWithinDependencyWindow(chunkX, chunkZ);
    return Math.abs(chunkX - this.centerChunkX) <= this.writeRadiusCutoff
      && Math.abs(chunkZ - this.centerChunkZ) <= this.writeRadiusCutoff;
  }
}
