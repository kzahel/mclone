import type { BlockState } from "../../world/level/block/state/block-state";
import { LeavesBlock } from "../../world/level/block/leaves-block";

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

export class Heightmap {}

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
