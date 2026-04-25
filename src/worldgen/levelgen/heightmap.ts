import type { BlockState } from "../../world/level/block/state/block-state";
import { LeavesBlock } from "../../world/level/block/leaves-block";
import { BlockPos } from "../../core/block-pos";
import type { LevelChunk } from "../../world/level/chunk/level-chunk";

const HEIGHTMAP_SIZE = 16 * 16;

function notAir(state: BlockState): boolean {
  return !state.isAir();
}

function materialMotionBlocking(state: BlockState): boolean {
  return state.getMaterial().blocksMotion();
}

function motionBlocking(state: BlockState): boolean {
  return state.getMaterial().blocksMotion() || !state.getFluidState().isEmpty();
}

function motionBlockingNoLeaves(state: BlockState): boolean {
  return motionBlocking(state) && !(state.getBlock() instanceof LeavesBlock);
}

export class Heightmap {
  private readonly data = new Int32Array(HEIGHTMAP_SIZE);

  public constructor(
    private readonly chunk: LevelChunk,
    private readonly type: Heightmap.Types,
  ) {}

  public static primeHeightmaps(chunk: LevelChunk, types: Iterable<Heightmap.Types>): void {
    const pendingTypes = [...types];
    if (pendingTypes.length === 0) {
      return;
    }

    const pos = new BlockPos.MutableBlockPos();
    for (let localX = 0; localX < 16; localX++) {
      for (let localZ = 0; localZ < 16; localZ++) {
        const pending = pendingTypes.map((type) => chunk.getOrCreateHeightmapUnprimed(type));
        const worldX = (chunk.chunkX * 16) + localX;
        const worldZ = (chunk.chunkZ * 16) + localZ;
        for (let y = chunk.getMaxBuildHeight() - 1; y >= chunk.getMinBuildHeight(); y--) {
          pos.set(worldX, y, worldZ);
          const state = chunk.getBlockState(pos);
          if (state.isAir()) {
            continue;
          }

          for (let index = pending.length - 1; index >= 0; index--) {
            const heightmap = pending[index]!;
            if (heightmap.type.isOpaque(state)) {
              heightmap.setHeight(localX, localZ, y + 1);
              pending.splice(index, 1);
            }
          }

          if (pending.length === 0) {
            break;
          }
        }
      }
    }
  }

  public update(localX: number, y: number, localZ: number, state: BlockState): boolean {
    const firstAvailable = this.getFirstAvailable(localX, localZ);
    if (y <= firstAvailable - 2) {
      return false;
    }

    if (this.type.isOpaque(state)) {
      if (y >= firstAvailable) {
        this.setHeight(localX, localZ, y + 1);
        return true;
      }
    } else if (firstAvailable - 1 === y) {
      const worldX = (this.chunk.chunkX * 16) + localX;
      const worldZ = (this.chunk.chunkZ * 16) + localZ;
      const pos = new BlockPos.MutableBlockPos();
      for (let scanY = y - 1; scanY >= this.chunk.getMinBuildHeight(); scanY--) {
        pos.set(worldX, scanY, worldZ);
        if (this.type.isOpaque(this.chunk.getBlockState(pos))) {
          this.setHeight(localX, localZ, scanY + 1);
          return true;
        }
      }

      this.setHeight(localX, localZ, this.chunk.getMinBuildHeight());
      return true;
    }

    return false;
  }

  public getFirstAvailable(localX: number, localZ: number): number {
    return this.getFirstAvailableByIndex(Heightmap.getIndex(localX, localZ));
  }

  public getHighestTaken(localX: number, localZ: number): number {
    return this.getFirstAvailable(localX, localZ) - 1;
  }

  private getFirstAvailableByIndex(index: number): number {
    return this.data[index]! + this.chunk.getMinBuildHeight();
  }

  private setHeight(localX: number, localZ: number, height: number): void {
    this.data[Heightmap.getIndex(localX, localZ)] = height - this.chunk.getMinBuildHeight();
  }

  private static getIndex(localX: number, localZ: number): number {
    return (localX & 15) + ((localZ & 15) * 16);
  }
}

export namespace Heightmap {
  export enum Usage {
    WORLDGEN,
    LIVE_WORLD,
    CLIENT,
  }

  export class Types {
    public static readonly WORLD_SURFACE_WG = new Types("WORLD_SURFACE_WG", Usage.WORLDGEN, notAir);
    public static readonly WORLD_SURFACE = new Types("WORLD_SURFACE", Usage.CLIENT, notAir);
    public static readonly OCEAN_FLOOR_WG = new Types("OCEAN_FLOOR_WG", Usage.WORLDGEN, materialMotionBlocking);
    public static readonly OCEAN_FLOOR = new Types("OCEAN_FLOOR", Usage.LIVE_WORLD, materialMotionBlocking);
    public static readonly MOTION_BLOCKING = new Types("MOTION_BLOCKING", Usage.CLIENT, motionBlocking);
    public static readonly MOTION_BLOCKING_NO_LEAVES = new Types("MOTION_BLOCKING_NO_LEAVES", Usage.LIVE_WORLD, motionBlockingNoLeaves);
    private static readonly VALUES = [
      Types.WORLD_SURFACE_WG,
      Types.WORLD_SURFACE,
      Types.OCEAN_FLOOR_WG,
      Types.OCEAN_FLOOR,
      Types.MOTION_BLOCKING,
      Types.MOTION_BLOCKING_NO_LEAVES,
    ] as const;
    private static readonly REVERSE_LOOKUP = new Map(Types.VALUES.map((type) => [type.serializationKey, type]));

    private constructor(
      private readonly serializationKey: string,
      private readonly usage: Usage,
      private readonly opaque: (state: BlockState) => boolean,
    ) {}

    public static values(): readonly Types[] {
      return Types.VALUES;
    }

    public static getFromKey(key: string): Types | undefined {
      return Types.REVERSE_LOOKUP.get(key);
    }

    public getSerializationKey(): string {
      return this.serializationKey;
    }

    public sendToClient(): boolean {
      return this.usage === Usage.CLIENT;
    }

    public keepAfterWorldgen(): boolean {
      return this.usage !== Usage.WORLDGEN;
    }

    public isOpaque(state: BlockState): boolean {
      return this.opaque(state);
    }

    public toString(): string {
      return this.serializationKey;
    }
  }
}
