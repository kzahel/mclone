import { describe, expect, test } from "vitest";
import type { DebugInputFrame } from "../../src/renderer/debug/debug-input";
import { buildDebugOverlayLines, consumePlayerPhysicsCommandsForFrame, shouldQueuePlayerInput } from "../../src/renderer/gui/gpu-world-runtime";
import { MovementCommandClock } from "../../src/runtime/movement";
import type { PlayerInputCommand } from "../../src/runtime/protocol/world-messages";
import { PLAYER_COMMAND_QUANTUM_US } from "../../src/runtime/session/player-loop";

function input(overrides: Partial<PlayerInputCommand> = {}): PlayerInputCommand {
  return {
    sequence: 1,
    moveX: 0,
    moveY: 0,
    moveZ: 0,
    yaw: 0,
    pitch: 0,
    buttons: 0,
    edgeButtons: 0,
    ...overrides,
  };
}

function frame(overrides: Partial<DebugInputFrame> = {}): DebugInputFrame {
  return {
    heldKeys: new Set<string>(),
    mouseDeltaX: 0,
    mouseDeltaY: 0,
    locked: false,
    joystickX: 0,
    joystickY: 0,
    moveForward: false,
    moveBack: false,
    flyUp: false,
    flyDown: false,
    ...overrides,
  };
}

describe("gpu world runtime player input queueing", () => {
  test("formats the debug overlay lines for player movement mode", () => {
    expect(buildDebugOverlayLines({
      cameraPosition: [9.5, 65.62, -2.25],
      cameraYaw: 180,
      cameraPitch: 15.5,
      playerTick: 123,
      playerPosition: [8.25, 64, -4.5],
      playerChunkX: 0,
      playerChunkZ: -1,
      chunkViewCenterX: 1,
      chunkViewCenterZ: 2,
      loadedChunkCount: 25,
    }, "player")).toEqual([
      "XYZ: 8.250 / 64.000 / -4.500",
      "Pitch/Yaw: 15.5 / 180.0",
      "# Ticks: 123",
      "Chunk: 0, -1",
      "View Chunk: 1, 2",
      "Loaded Chunks: 25",
      "Mode: Player",
    ]);
  });

  test("queues unchanged idle commands when they advance movement time", () => {
    const previous = input({
      sequence: 10,
      commandQuantumUs: 8_333,
      stepCount: 1,
    });
    const current = input({
      sequence: 11,
      commandQuantumUs: 8_333,
      stepCount: 1,
    });

    expect(shouldQueuePlayerInput(previous, current)).toBe(true);
  });

  test("drops unchanged zero-step idle commands", () => {
    const previous = input({ sequence: 10 });
    const current = input({ sequence: 11 });

    expect(shouldQueuePlayerInput(previous, current)).toBe(false);
  });

  test("emits idle fixed-step commands when pointer lock is required but unavailable", () => {
    const commandClock = new MovementCommandClock({ commandQuantumUs: PLAYER_COMMAND_QUANTUM_US });
    const result = consumePlayerPhysicsCommandsForFrame({
      commandClock,
      inputFrame: frame({
        heldKeys: new Set(["KeyW", "Space"]),
        mouseDeltaX: 20,
        mouseDeltaY: -10,
        moveForward: true,
        flyUp: true,
      }),
      requirePointerLock: true,
      baseYaw: 45,
      basePitch: 10,
      dtSeconds: PLAYER_COMMAND_QUANTUM_US / 1_000_000,
      nowMs: 123.456,
      nextInputSequence: 5,
      lastPlayerButtonMask: 1,
    });

    expect(result.acceptsGameplayInput).toBe(false);
    expect(result.lastPlayerButtonMask).toBe(0);
    expect(result.inputCommands).toHaveLength(1);
    expect(result.inputCommands[0]).toMatchObject({
      sequence: 5,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: 45,
      pitch: 10,
      buttons: 0,
      edgeButtons: 0,
      commandQuantumUs: PLAYER_COMMAND_QUANTUM_US,
      stepCount: 1,
    });
  });

  test("consumes physics time while unlocked so relocking does not create catch-up debt", () => {
    const commandClock = new MovementCommandClock({ commandQuantumUs: PLAYER_COMMAND_QUANTUM_US });
    const unlocked = consumePlayerPhysicsCommandsForFrame({
      commandClock,
      inputFrame: frame({ locked: false }),
      requirePointerLock: true,
      baseYaw: 0,
      basePitch: 0,
      dtSeconds: PLAYER_COMMAND_QUANTUM_US / 1_000_000,
      nowMs: 1,
      nextInputSequence: 1,
      lastPlayerButtonMask: 0,
    });
    const relocked = consumePlayerPhysicsCommandsForFrame({
      commandClock,
      inputFrame: frame({ locked: true, heldKeys: new Set(["KeyW"]) }),
      requirePointerLock: true,
      baseYaw: 0,
      basePitch: 0,
      dtSeconds: PLAYER_COMMAND_QUANTUM_US / 1_000_000,
      nowMs: 2,
      nextInputSequence: 2,
      lastPlayerButtonMask: unlocked.lastPlayerButtonMask,
    });

    expect(unlocked.inputCommands.map((command) => command.stepCount)).toEqual([1]);
    expect(relocked.inputCommands.map((command) => command.stepCount)).toEqual([1]);
    expect(relocked.inputCommands[0]?.moveZ).toBe(1);
  });
});
