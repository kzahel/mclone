import { describe, expect, test } from "vitest";
import {
  buildPlayerInputCommand,
  createChunkViewRequestForPlayerState,
  mergeDebugInputFrame,
} from "../../src/renderer/debug/debug-player-controls";

describe("debug player controls", () => {
  test("builds player input commands from baseline rotation and forward input", () => {
    const command = buildPlayerInputCommand(
      180,
      30,
      {
        heldKeys: new Set(["KeyW"]),
        mouseDeltaX: 0,
        mouseDeltaY: 0,
        locked: true,
        joystickX: 0,
        joystickY: 0,
        moveForward: false,
        moveBack: false,
        flyUp: false,
        flyDown: false,
      },
      1 / 60,
      7,
    );

    expect(command.sequence).toBe(7);
    expect(command.yaw).toBe(180);
    expect(command.pitch).toBe(30);
    expect(command.moveZ).toBeLessThan(0);
  });

  test("merges injected debug input over the live frame", () => {
    const merged = mergeDebugInputFrame(
      {
        heldKeys: new Set(["KeyA"]),
        mouseDeltaX: 1,
        mouseDeltaY: 2,
        locked: false,
        joystickX: 0,
        joystickY: 0,
        moveForward: false,
        moveBack: false,
        flyUp: false,
        flyDown: false,
      },
      {
        heldKeys: ["KeyW"],
        locked: true,
        moveForward: true,
      },
    );

    expect(merged.locked).toBe(true);
    expect(merged.heldKeys.has("KeyA")).toBe(true);
    expect(merged.heldKeys.has("KeyW")).toBe(true);
    expect(merged.moveForward).toBe(true);
    expect(merged.mouseDeltaX).toBe(1);
  });

  test("derives chunk-view requests from authoritative player positions", () => {
    const request = createChunkViewRequestForPlayerState(
      {
        playerId: "player",
        position: { x: 40.5, y: 104, z: 72.5 },
        rotation: { yaw: 0, pitch: 0 },
        acknowledgedInputSequence: 0,
        tick: 2,
        revision: 1,
      },
      2,
    );

    expect(request).toEqual({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 4,
      radius: 2,
    });
  });
});
