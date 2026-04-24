import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import { SectionPos } from "../../core/section-pos";
import type { Biome } from "../../worldgen/biome/biome";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { Fluid } from "../../world/level/material/fluid";
import type { FluidState } from "../../world/level/material/fluid-state";
import { LightLayer } from "../../world/level/light-layer";
import { BlackholeTickAccess, type TickAccess } from "../../world/level/tick-access";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";

export interface LiquidSimulationLevelOptions {
  readonly level: GeneratedRenderLevel;
  readonly airState: BlockState;
  readonly liquidTicks: TickAccess<Fluid>;
  readonly onBlockChanged: (pos: BlockPos, oldState: BlockState, newState: BlockState) => void;
}

export class LiquidSimulationLevel implements WorldGenLevel {
  private readonly blockTicks = new BlackholeTickAccess<Block>();

  public constructor(private readonly options: LiquidSimulationLevelOptions) {}

  public setBlock(pos: BlockPos, state: BlockState, _flags = 3): boolean {
    if (pos.getY() < this.getMinBuildHeight() || pos.getY() >= this.getMaxBuildHeight()) {
      return false;
    }

    const chunk = this.options.level.getChunk(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getZ()),
      false,
    );
    if (chunk === null) {
      return false;
    }

    const oldState = chunk.getBlockState(pos);
    if (oldState === state) {
      return true;
    }

    chunk.setBlockState(pos, state);
    const stablePos = new BlockPos(pos.getX(), pos.getY(), pos.getZ());
    this.options.onBlockChanged(stablePos, oldState, state);
    state.getBlock().onPlace(state, this, stablePos, oldState, false);
    this.updateNeighborsAt(stablePos, state);
    return true;
  }

  private updateNeighborsAt(pos: BlockPos, state: BlockState): void {
    for (const direction of Direction.values()) {
      const neighborPos = pos.relative(direction);
      const neighborState = this.getBlockState(neighborPos);
      const updated = neighborState.updateShape(direction.getOpposite(), state, this, neighborPos, pos);
      if (updated !== neighborState) {
        this.setBlock(neighborPos, updated);
      }
      neighborState.getBlock().neighborChanged(neighborState, this, neighborPos, state.getBlock(), pos, false);
    }
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.options.level.getChunk(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getZ()),
      false,
    )?.getBlockState(pos) ?? this.options.airState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public isStateAtPosition(pos: BlockPos, predicate: (state: BlockState) => boolean): boolean {
    return predicate(this.getBlockState(pos));
  }

  public isEmptyBlock(pos: BlockPos): boolean {
    return this.getBlockState(pos).isAir();
  }

  public getHeight(type: Heightmap.Types, x: number, z: number): number {
    for (let y = this.getMaxBuildHeight() - 1; y >= this.getMinBuildHeight(); y--) {
      if (type.isOpaque(this.getBlockState(new BlockPos(x, y, z)))) {
        return y + 1;
      }
    }

    return this.getMinBuildHeight();
  }

  public getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos {
    return new BlockPos(pos.getX(), this.getHeight(type, pos.getX(), pos.getZ()), pos.getZ());
  }

  public getMinBuildHeight(): number {
    return this.options.level.getMinBuildHeight();
  }

  public getMaxBuildHeight(): number {
    return this.options.level.getMaxBuildHeight();
  }

  public getBiome(pos: BlockPos): Biome {
    return this.options.level.getBiome(pos);
  }

  public getBrightness(layer: LightLayer, pos: BlockPos): number {
    return this.options.level.getBrightness(layer, pos);
  }

  public getMaxLightLevel(): number {
    return this.options.level.getMaxLightLevel();
  }

  public getBlockTicks(): TickAccess<Block> {
    return this.blockTicks;
  }

  public getLiquidTicks(): TickAccess<Fluid> {
    return this.options.liquidTicks;
  }
}
