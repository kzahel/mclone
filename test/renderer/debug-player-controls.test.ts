import { describe, expect, test } from "vitest";
import {
  applyPredictedCameraInput,
  buildPlayerInputCommand,
  createChunkViewRequestForCameraState,
  createChunkViewRequestForPlayerState,
  mergeDebugInputFrame,
  reconcilePredictedCameraState,
} from "../../src/renderer/debug/debug-player-controls";
import { Vec3 } from "../../src/world/phys/vec3";

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

  test("applies camera prediction immediately from local input", () => {
    const command = buildPlayerInputCommand(
      180,
      30,
      {
        heldKeys: new Set(["KeyW"]),
        mouseDeltaX: 10,
        mouseDeltaY: -4,
        locked: true,
        joystickX: 0,
        joystickY: 0,
        moveForward: false,
        moveBack: false,
        flyUp: false,
        flyDown: false,
      },
      1 / 120,
      8,
    );

    const camera = applyPredictedCameraInput(
      { position: new Vec3(8.5, 104, 40.5), xRot: 30, yRot: 180 },
      command,
      1 / 120,
    );

    expect(camera.yRot).toBeCloseTo(181.5);
    expect(camera.xRot).toBeCloseTo(29.4);
    expect(camera.position.z).toBeLessThan(40.5);
  });

  test("keeps predicted camera state until authoritative input catches up", () => {
    const predicted = { position: new Vec3(9, 105, 41), xRot: 25, yRot: 190 };
    const playerState = {
      playerId: "player",
      position: { x: 8.5, y: 104, z: 40.5 },
      rotation: { yaw: 180, pitch: 30 },
      acknowledgedInputSequence: 7,
      tick: 2,
      revision: 1,
    };

    expect(reconcilePredictedCameraState(predicted, playerState, {
      sequence: 8,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: 190,
      pitch: 25,
    })).toBe(predicted);

    const reconciled = reconcilePredictedCameraState(predicted, {
      ...playerState,
      acknowledgedInputSequence: 8,
    }, {
      sequence: 8,
      moveX: 0,
      moveY: 0,
      moveZ: 0,
      yaw: 190,
      pitch: 25,
    });

    expect(reconciled.position.x).toBe(8.5);
    expect(reconciled.yRot).toBe(180);
  });

  test("derives chunk-view requests from predicted camera positions", () => {
    const request = createChunkViewRequestForCameraState(
      { position: new Vec3(40.5, 104, 72.5), xRot: 0, yRot: 0 },
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
