import { describe, expect, test } from "vitest";
import type { DebugInputFrame } from "../../src/renderer/debug/debug-input";
import {
  buildDebugOverlayLines,
  consumePlayerPhysicsCommandsForFrame,
  shouldQueuePlayerInput,
} from "../../src/renderer/gui/gpu-world-runtime";
import {
  buildChunkLifecycleHudLines,
  getChunkLifecycleHudBounds,
  getChunkLifecycleHudCellState,
} from "../../src/client/gui/chunk-lifecycle-hud";
import { MovementCommandClock } from "../../src/runtime/movement";
import type { PlayerInputCommand } from "../../src/runtime/protocol/world-messages";
import type { GeneratedChunkLifecycleRecord, GeneratedChunkLifecycleSnapshot } from "../../src/runtime/protocol/chunk-lifecycle";
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

function lifecycleRecord(overrides: Partial<GeneratedChunkLifecycleRecord> = {}): GeneratedChunkLifecycleRecord {
  return {
    chunkX: 0,
    chunkZ: 0,
    inAuthorityView: true,
    inFullView: true,
    inPublishView: true,
    ticketLevel: 31,
    ticketFullStatus: "entity_ticking",
    ticketSources: [],
    generatedStatus: "empty",
    hasBlockSections: false,
    chunkLoaded: false,
    statusJobs: [],
    published: false,
    dirtyForPublication: false,
    dirtyDurable: false,
    queuedForUnload: false,
    pendingUnload: false,
    pendingStorageWrite: false,
    lightInputSent: false,
    lightAccepted: false,
    publicationBlocker: { kind: "missing_materialized_chunk", chunkX: 0, chunkZ: 0, requiredStatus: "full", actualStatus: "empty" },
    ...overrides,
  };
}

function lifecycleSnapshot(records: readonly GeneratedChunkLifecycleRecord[]): GeneratedChunkLifecycleSnapshot {
  return {
    currentChunkView: { centerChunkX: 0, centerChunkZ: 0, radius: 1 },
    chunkViewJobRevision: 7,
    activeChunkViewJobRevision: 8,
    records,
    counts: {
      total: records.length,
      inAuthorityView: records.filter((record) => record.inAuthorityView).length,
      inPublishView: records.filter((record) => record.inPublishView).length,
      loaded: records.filter((record) => record.chunkLoaded).length,
      materialized: records.filter((record) => record.hasBlockSections).length,
      published: records.filter((record) => record.published).length,
      dirtyForPublication: records.filter((record) => record.dirtyForPublication).length,
      queuedForUnload: records.filter((record) => record.queuedForUnload).length,
      pendingUnload: records.filter((record) => record.pendingUnload).length,
      byGeneratedStatus: {},
      byHolderFullStatus: {},
      byPublicationBlocker: {
        missing_materialized_chunk: records.filter((record) => record.publicationBlocker.kind === "missing_materialized_chunk").length,
        already_published: records.filter((record) => record.publicationBlocker.kind === "already_published").length,
      },
    },
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

  test("classifies chunk lifecycle HUD cells by actionable state priority", () => {
    expect(getChunkLifecycleHudCellState(lifecycleRecord({ queuedForUnload: true, published: true }))).toBe("unload");
    expect(getChunkLifecycleHudCellState(lifecycleRecord({ dirtyForPublication: true, published: true }))).toBe("dirty");
    expect(getChunkLifecycleHudCellState(lifecycleRecord({ published: true }))).toBe("published");
    expect(getChunkLifecycleHudCellState(lifecycleRecord())).toBe("blocked");
    expect(getChunkLifecycleHudCellState(lifecycleRecord({ publicationBlocker: { kind: "ready_to_publish" } }))).toBe("ready");
    expect(getChunkLifecycleHudCellState(lifecycleRecord({
      inPublishView: false,
      hasBlockSections: true,
      publicationBlocker: { kind: "outside_publish_view" },
    }))).toBe("materialized");
    expect(getChunkLifecycleHudCellState(lifecycleRecord({
      inPublishView: false,
      generatedStatus: "features",
      publicationBlocker: { kind: "outside_publish_view" },
    }))).toBe("generated");
  });

  test("summarizes chunk lifecycle HUD bounds and counts", () => {
    const snapshot = lifecycleSnapshot([
      lifecycleRecord({ chunkX: -2, chunkZ: 3, published: true, publicationBlocker: { kind: "already_published" } }),
      lifecycleRecord({ chunkX: 4, chunkZ: -1 }),
    ]);
    const bounds = getChunkLifecycleHudBounds(snapshot)!;

    expect(bounds).toEqual({ minChunkX: -2, maxChunkX: 4, minChunkZ: -1, maxChunkZ: 3 });
    expect(buildChunkLifecycleHudLines(snapshot, bounds)).toEqual([
      "Chunk Lifecycle rev=7 active=8",
      "x=-2..4 z=-1..3",
      "view=0,0 r=1 pub=1/2 loaded=0 blocked=1",
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
