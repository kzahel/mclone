import { BlockPos } from "../../core/block-pos";
import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import type { Block } from "./block/block";
import type { Fluid } from "./material/fluid";
import { Fluids } from "./material/fluids";

export interface ScheduledTickSnapshot {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly target: string;
  readonly delay: number;
}

export function createScheduledTickSnapshot(pos: BlockPos, target: string, delay: number): ScheduledTickSnapshot {
  return {
    x: pos.getX(),
    y: pos.getY(),
    z: pos.getZ(),
    target,
    delay,
  };
}

export function cloneScheduledTickSnapshot(tick: ScheduledTickSnapshot): ScheduledTickSnapshot {
  return {
    x: tick.x,
    y: tick.y,
    z: tick.z,
    target: tick.target,
    delay: tick.delay,
  };
}

export function serializeBlockTickTarget(target: Block): string {
  const key = Registry.BLOCK.getKey(target as unknown as object) ?? target.getLocation();
  if (key === undefined) {
    throw new Error(`Cannot serialize scheduled tick for unregistered block ${target}`);
  }

  return key.toString();
}

export function resolveBlockTickTarget(targetName: string): Block {
  const block = Registry.BLOCK.get(new ResourceLocation(targetName)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Cannot hydrate scheduled tick for missing block ${targetName}`);
  }

  return block;
}

export function serializeFluidTickTarget(target: Fluid): string {
  if (target === Fluids.WATER) {
    return "minecraft:water";
  }

  if (target === Fluids.FLOWING_WATER) {
    return "minecraft:flowing_water";
  }

  if (target === Fluids.LAVA) {
    return "minecraft:lava";
  }

  if (target === Fluids.FLOWING_LAVA) {
    return "minecraft:flowing_lava";
  }

  if (target === Fluids.EMPTY) {
    return "minecraft:empty";
  }

  throw new Error(`Cannot serialize scheduled tick for unsupported fluid ${target.constructor.name}`);
}

export function resolveFluidTickTarget(targetName: string): Fluid {
  switch (targetName) {
    case "minecraft:water":
      return Fluids.WATER;
    case "minecraft:flowing_water":
      return Fluids.FLOWING_WATER;
    case "minecraft:lava":
      return Fluids.LAVA;
    case "minecraft:flowing_lava":
      return Fluids.FLOWING_LAVA;
    case "minecraft:empty":
      return Fluids.EMPTY;
    default:
      throw new Error(`Cannot hydrate scheduled tick for missing fluid ${targetName}`);
  }
}
