import { describe, expect, test } from "vitest";
import {
  createInitialPlayerState,
  createPlayerCommandQueue,
  enqueuePlayerInputCommand,
  PLAYER_COMMAND_QUANTUM_US,
  tickPlayerStateWithCommandQueue,
} from "../../src/runtime/session/player-loop";
import { createMissingCollisionWorld, createStaticCollisionWorld } from "../../src/runtime/movement";
import { AABB } from "../../src/world/phys/aabb";

function floorAt(y: number): AABB {
  return new AABB(-64, y - 1, -64, 64, y, 64);
}

describe("authoritative player command queue", () => {
  test("drains multiple queued commands on one host tick", () => {
    const initial = createInitialPlayerState("player");
    const queue = createPlayerCommandQueue();

    expect(enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 90,
      pitch: 10,
    })).toBe(true);
    expect(enqueuePlayerInputCommand(queue, initial, {
      sequence: 2,
      moveX: 0,
      moveY: 0,
      moveZ: 1,
      yaw: 95,
      pitch: 12,
    })).toBe(true);

    expect(initial.acknowledgedInputSequence).toBe(0);

    const next = tickPlayerStateWithCommandQueue(initial, queue, 1, createStaticCollisionWorld([floorAt(initial.position.y)]));

    expect(queue.commands).toEqual([]);
    expect(next.acknowledgedInputSequence).toBe(2);
    expect(next.rotation).toEqual({ yaw: 95, pitch: 12 });
    expect(next.position.x).not.toBe(initial.position.x);
    expect(next.position.z).not.toBe(initial.position.z);
    expect(next.movementBody?.mode).toBe("ground");
  });

  test("does not move again when no command is queued", () => {
    const initial = createInitialPlayerState("player");
    const queue = createPlayerCommandQueue();
    enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 0,
      pitch: 0,
    });

    const world = createStaticCollisionWorld([floorAt(initial.position.y)]);
    const moved = tickPlayerStateWithCommandQueue(initial, queue, 1, world);
    const idle = tickPlayerStateWithCommandQueue(moved, queue, 2, world);

    expect(idle).toBe(moved);
    expect(idle.position).toEqual(moved.position);
    expect(idle.acknowledgedInputSequence).toBe(1);
  });

  test("simulates fixed command quanta inside a lower-rate host tick", () => {
    const initial = createInitialPlayerState("player");
    const queue = createPlayerCommandQueue();
    enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 0,
      pitch: 0,
      commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
      stepCount: 2,
    });

    const next = tickPlayerStateWithCommandQueue(initial, queue, 1, createStaticCollisionWorld([floorAt(initial.position.y)]));

    expect(next.position.x).toBeGreaterThan(initial.position.x);
    expect(next.movementBody?.velocity.x).toBeGreaterThan(0);
    expect(next.movementBody?.lastProcessedCommandSequence).toBe(1);
  });

  test("keeps commands queued when collision data is missing", () => {
    const initial = createInitialPlayerState("player");
    const queue = createPlayerCommandQueue();
    enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 0,
      pitch: 0,
      commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
      stepCount: 1,
    });

    const next = tickPlayerStateWithCommandQueue(initial, queue, 1, createMissingCollisionWorld("chunk not loaded"));

    expect(next).toBe(initial);
    expect(queue.commands.map((entry) => entry.command.sequence)).toEqual([1]);
    expect(next.acknowledgedInputSequence).toBe(0);
  });

  test("drops duplicate or already-acknowledged input commands", () => {
    const initial = createInitialPlayerState("player");
    const queue = createPlayerCommandQueue();

    expect(enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 0,
      pitch: 0,
    })).toBe(true);
    expect(enqueuePlayerInputCommand(queue, initial, {
      sequence: 1,
      moveX: 0,
      moveY: 0,
      moveZ: 1,
      yaw: 0,
      pitch: 0,
    })).toBe(false);

    const acknowledged = tickPlayerStateWithCommandQueue(initial, queue, 1, createStaticCollisionWorld([floorAt(initial.position.y)]));
    expect(enqueuePlayerInputCommand(queue, acknowledged, {
      sequence: 1,
      moveX: 1,
      moveY: 0,
      moveZ: 0,
      yaw: 0,
      pitch: 0,
    })).toBe(false);
  });
});
