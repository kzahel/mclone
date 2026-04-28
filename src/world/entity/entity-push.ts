import { Vec3 } from "../phys/vec3";
import type { AABB } from "../phys/aabb";

export interface LivingEntityPushParticipant {
  readonly key: string;
  readonly x: number;
  readonly z: number;
  readonly boundingBox: AABB;
  readonly sourcePushes: boolean;
  readonly pushable: boolean;
  readonly vehicle: boolean;
  readonly noPhysics: boolean;
  isPassengerOfSameVehicle?(other: LivingEntityPushParticipant): boolean;
}

export function collectLivingEntityPushDeltas(
  participants: readonly LivingEntityPushParticipant[],
): ReadonlyMap<string, Vec3> {
  const deltas = new Map<string, Vec3>();

  for (const source of participants) {
    if (!source.sourcePushes || !source.pushable || source.noPhysics) {
      continue;
    }

    for (const target of participants) {
      if (
        source.key === target.key
        || !target.pushable
        || target.noPhysics
        || source.isPassengerOfSameVehicle?.(target) === true
        || target.isPassengerOfSameVehicle?.(source) === true
        || !source.boundingBox.intersects(target.boundingBox)
      ) {
        continue;
      }

      accumulateVanillaEntityPush(target, source, deltas);
    }
  }

  return deltas;
}

function accumulateDelta(deltas: Map<string, Vec3>, key: string, x: number, z: number): void {
  const previous = deltas.get(key) ?? Vec3.ZERO;
  deltas.set(key, previous.add(x, 0.0, z));
}

function accumulateVanillaEntityPush(
  self: LivingEntityPushParticipant,
  other: LivingEntityPushParticipant,
  deltas: Map<string, Vec3>,
): void {
  let x = other.x - self.x;
  let z = other.z - self.z;
  let max = Math.max(Math.abs(x), Math.abs(z));
  if (max < 0.01) {
    return;
  }

  max = Math.sqrt(max);
  x /= max;
  z /= max;
  let inverse = 1.0 / max;
  if (inverse > 1.0) {
    inverse = 1.0;
  }

  x *= inverse;
  z *= inverse;
  x *= 0.05;
  z *= 0.05;
  if (!self.vehicle) {
    accumulateDelta(deltas, self.key, -x, -z);
  }
  if (!other.vehicle) {
    accumulateDelta(deltas, other.key, x, z);
  }
}
