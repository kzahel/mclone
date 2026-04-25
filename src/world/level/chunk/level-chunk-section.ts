import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../block/state/block-state";
import type { FluidState } from "../material/fluid-state";

export const LEVEL_CHUNK_SECTION_WIDTH = 16;
export const LEVEL_CHUNK_SECTION_HEIGHT = 16;
export const LEVEL_CHUNK_SECTION_SIZE = 4096;

export interface LevelChunkSectionEntry {
  readonly localX: number;
  readonly localY: number;
  readonly localZ: number;
  readonly state: BlockState;
}

function sectionIndex(localX: number, localY: number, localZ: number): number {
  return (localY << 8) | (localZ << 4) | localX;
}

export class LevelChunkSection {
  private readonly states = new Array<BlockState | undefined>(LEVEL_CHUNK_SECTION_SIZE);
  private nonEmptyBlockCount = 0;
  private tickingFluidCount = 0;
  private storedBlockCount = 0;

  public constructor(
    public readonly sectionY: number,
    private readonly airState: BlockState,
  ) {}

  public static getBottomBlockY(sectionY: number): number {
    return sectionY << 4;
  }

  public bottomBlockY(): number {
    return LevelChunkSection.getBottomBlockY(this.sectionY);
  }

  public getBlockState(localX: number, localY: number, localZ: number): BlockState {
    return this.states[sectionIndex(localX, localY, localZ)] ?? this.airState;
  }

  public getFluidState(localX: number, localY: number, localZ: number): FluidState {
    return this.getBlockState(localX, localY, localZ).getFluidState();
  }

  public setBlockState(localX: number, localY: number, localZ: number, state: BlockState): BlockState {
    const index = sectionIndex(localX, localY, localZ);
    const previous = this.states[index] ?? this.airState;
    if (previous === state) {
      return previous;
    }

    this.removeCounts(previous, this.states[index] !== undefined);
    if (state === this.airState) {
      this.states[index] = undefined;
    } else {
      this.states[index] = state;
    }
    this.addCounts(state, this.states[index] !== undefined);
    return previous;
  }

  public isEmpty(): boolean {
    return this.nonEmptyBlockCount === 0;
  }

  public hasStoredBlocks(): boolean {
    return this.storedBlockCount > 0;
  }

  public isYSpaceEmpty(localMinY: number, localMaxY: number): boolean {
    if (this.nonEmptyBlockCount === 0) {
      return true;
    }

    for (let localY = localMinY; localY <= localMaxY; localY++) {
      const rowStart = localY << 8;
      for (let rowIndex = 0; rowIndex < 256; rowIndex++) {
        const state = this.states[rowStart + rowIndex];
        if (state !== undefined && !state.isAir()) {
          return false;
        }
      }
    }

    return true;
  }

  public *entries(): Iterable<LevelChunkSectionEntry> {
    for (let index = 0; index < this.states.length; index++) {
      const state = this.states[index];
      if (state === undefined) {
        continue;
      }

      yield {
        localX: index & 15,
        localY: index >> 8,
        localZ: (index >> 4) & 15,
        state,
      };
    }
  }

  public *blockEntries(chunkX: number, chunkZ: number): Iterable<{ readonly pos: BlockPos; readonly state: BlockState }> {
    const worldX = chunkX << 4;
    const worldY = this.bottomBlockY();
    const worldZ = chunkZ << 4;
    for (const entry of this.entries()) {
      yield {
        pos: new BlockPos(worldX + entry.localX, worldY + entry.localY, worldZ + entry.localZ),
        state: entry.state,
      };
    }
  }

  private removeCounts(state: BlockState, wasStored: boolean): void {
    if (wasStored) {
      this.storedBlockCount--;
    }

    if (!state.isAir()) {
      this.nonEmptyBlockCount--;
    }

    if (!state.getFluidState().isEmpty()) {
      this.tickingFluidCount--;
    }
  }

  private addCounts(state: BlockState, isStored: boolean): void {
    if (isStored) {
      this.storedBlockCount++;
    }

    if (!state.isAir()) {
      this.nonEmptyBlockCount++;
    }

    if (!state.getFluidState().isEmpty()) {
      this.tickingFluidCount++;
    }
  }
}
