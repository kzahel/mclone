import { describe, expect, test } from "vitest";
import { buildDebugOverlayLines, shouldQueuePlayerInput } from "../../src/renderer/gui/gpu-world-runtime";
import type { PlayerInputCommand } from "../../src/runtime/protocol/world-messages";

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
});
