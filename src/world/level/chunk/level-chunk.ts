import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../block/state/block-state";
import type { FluidState } from "../material/fluid-state";

type ChunkEntry = {
  readonly pos: BlockPos;
  readonly state: BlockState;
};

export class LevelChunk {
  private readonly states = new Map<bigint, ChunkEntry>();

  public constructor(
    public readonly chunkX: number,
    public readonly chunkZ: number,
    private readonly airState: BlockState,
  ) {}

  public getBlockState(pos: BlockPos): BlockState {
    return this.states.get(pos.asLong())?.state ?? this.airState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public setBlockState(pos: BlockPos, state: BlockState): void {
    const key = pos.asLong();
    if (state.isAir()) {
      this.states.delete(key);
      return;
    }

    this.states.set(key, {
      pos: new BlockPos(pos.getX(), pos.getY(), pos.getZ()),
      state,
    });
  }

  public isYSpaceEmpty(minY: number, maxY: number): boolean {
    for (const entry of this.states.values()) {
      const y = entry.pos.getY();
      if (y >= minY && y <= maxY) {
        return false;
      }
    }

    return true;
  }
}
