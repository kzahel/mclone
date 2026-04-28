import { describe, expect, test } from "vitest";

import { collectLivingEntityPushDeltas, type LivingEntityPushParticipant } from "../../../src/world/entity/entity-push";
import { AABB } from "../../../src/world/phys/aabb";

function participant(
  key: string,
  x: number,
  options: Partial<LivingEntityPushParticipant> = {},
): LivingEntityPushParticipant {
  return {
    key,
    x,
    z: 0.0,
    boundingBox: new AABB(x - 0.45, 0.0, -0.45, x + 0.45, 1.4, 0.45),
    sourcePushes: true,
    pushable: true,
    vehicle: false,
    noPhysics: false,
    ...options,
  };
}

describe("living entity push interactions", () => {
  test("accumulates the vanilla horizontal push impulse for overlapping living entities", () => {
    const deltas = collectLivingEntityPushDeltas([
      participant("left", 0.0),
      participant("right", 0.5),
    ]);

    expect(deltas.get("left")?.x).toBeCloseTo(-0.070710678, 9);
    expect(deltas.get("right")?.x).toBeCloseTo(0.070710678, 9);
    expect(deltas.get("left")?.z).toBe(0.0);
    expect(deltas.get("right")?.z).toBe(0.0);
  });

  test("does not let non-ticking participants initiate pushes", () => {
    const deltas = collectLivingEntityPushDeltas([
      participant("source", 0.0, { sourcePushes: false }),
      participant("target", 0.5),
    ]);

    expect(deltas.get("source")?.x).toBeCloseTo(-0.035355339, 9);
    expect(deltas.get("target")?.x).toBeCloseTo(0.035355339, 9);
  });

  test("skips non-pushable and no-physics entities", () => {
    expect(collectLivingEntityPushDeltas([
      participant("left", 0.0),
      participant("right", 0.5, { pushable: false }),
    ]).size).toBe(0);

    expect(collectLivingEntityPushDeltas([
      participant("left", 0.0),
      participant("right", 0.5, { noPhysics: true }),
    ]).size).toBe(0);
  });
});
