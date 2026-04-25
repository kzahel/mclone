import { describe, expect, test } from "vitest";
import {
  createInitialPlayerState,
  createPlayerCommandQueue,
  enqueuePlayerInputCommand,
  PLAYER_COMMAND_QUANTUM_US,
  PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND,
  tickPlayerStateWithCommandQueue,
} from "../../src/runtime/session/player-loop";

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

    const next = tickPlayerStateWithCommandQueue(initial, queue, 1);

    expect(queue.commands).toEqual([]);
    expect(next.acknowledgedInputSequence).toBe(2);
    expect(next.rotation).toEqual({ yaw: 95, pitch: 12 });
    expect(next.position.x - initial.position.x).toBeCloseTo(0.4);
    expect(next.position.z - initial.position.z).toBeCloseTo(0.4);
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

    const moved = tickPlayerStateWithCommandQueue(initial, queue, 1);
    const idle = tickPlayerStateWithCommandQueue(moved, queue, 2);

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

    const next = tickPlayerStateWithCommandQueue(initial, queue, 1);
    const expectedDelta = PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND * ((PLAYER_COMMAND_QUANTUM_US * 2) / 1_000_000.0);

    expect(next.position.x - initial.position.x).toBeCloseTo(expectedDelta);
    expect(next.movementBody?.velocity.x).toBeCloseTo(PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND);
    expect(next.movementBody?.lastProcessedCommandSequence).toBe(1);
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

    const acknowledged = tickPlayerStateWithCommandQueue(initial, queue, 1);
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
