import { describe, expect, test } from "vitest";
import { WorldClientRuntimeFacade } from "../../src/runtime/client/client-runtime";
import {
  PlayerMovementPredictionService,
  type ClientWorldPredictionView,
} from "../../src/runtime/client";
import {
  createStandingMovementBody,
  createStaticCollisionWorld,
  DEFAULT_MOVEMENT_PHYSICS,
  PlayerMovementPredictor,
  type MovementAuthoritativeState,
} from "../../src/runtime/movement";
import { TransportWorldClient, type WorldTransport } from "../../src/runtime/transport/local-world-transport";
import type { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import type {
  ClientPlayerState,
  EntitySnapshot,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../../src/runtime/protocol/world-messages";
import { AABB } from "../../src/world/phys/aabb";
import { Vec3 } from "../../src/world/phys/vec3";

const OPENED: WorldOpenedMessage = {
  type: "world_opened",
  minBuildHeight: 0,
  height: 256,
  saveMetadata: {
    saveId: "prediction-test",
    storageVersion: 1,
    seed: "12345",
    preset: "default",
    minBuildHeight: 0,
    height: 256,
    createdAtMs: 1,
    lastOpenedAtMs: 1,
  },
};

const PLAYER_STATE: ClientPlayerState = {
  playerId: "player",
  position: { x: 1, y: 64, z: 3 },
  rotation: { yaw: 90, pitch: 10 },
  acknowledgedInputSequence: 7,
  tick: 11,
  revision: 13,
  movementBody: {
    position: { x: 1, y: 64, z: 3 },
    velocity: { x: 0.25, y: 0, z: -0.5 },
    bounds: {
      minX: 0.7,
      minY: 64,
      minZ: 2.7,
      maxX: 1.3,
      maxY: 65.8,
      maxZ: 3.3,
    },
    onGround: true,
    mode: "ground",
    jumpHeld: false,
    physicsRevision: 3,
    collisionRevision: 5,
    commandQuantumUs: 8_333,
    lastProcessedCommandSequence: 7,
  },
};

const ENTITY: EntitySnapshot = {
  id: 1,
  uuid: "00000000-0000-0000-0000-000000000001",
  typeId: "minecraft:sheep",
  category: "creature",
  chunkX: 0,
  chunkZ: 0,
  position: { x: 4, y: 64, z: 5 },
  rotation: { yaw: 0, pitch: 0 },
  width: 0.9,
  height: 1.3,
  onGround: true,
};

class OpenMessagesTransport implements WorldTransport {
  public constructor(private readonly messages: readonly WorldHostMessage[]) {}

  public openWorld(_request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve(this.messages);
  }

  public setChunkView(_request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public setPlayerInput(_request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public pollUpdates(_request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }
}

function floor(): AABB {
  return new AABB(-20, -1, -20, 20, 0, 20);
}

describe("Client prediction service", () => {
  test("ClientRuntime exposes a prediction service seeded from ClientWorld facts", async () => {
    const client = new TransportWorldClient(
      new OpenMessagesTransport([
        OPENED,
        { type: "player_state", state: PLAYER_STATE },
        { type: "entity_snapshot", entity: ENTITY },
      ]),
      () => ({}) as ClientChunkCache,
    );
    const runtime = new WorldClientRuntimeFacade(client);

    await runtime.openWorld({ type: "open_world", seed: 12345n, preset: "default" });

    const predictionView = runtime.getClientWorld().getPredictionView();
    expect(predictionView.getEntitySnapshots()).toEqual([ENTITY]);
    expect(predictionView.getAuthoritativeMovementState()).toMatchObject({
      lastProcessedCommandSeq: 7,
      physicsRevision: 3,
      collisionRevision: 5,
      body: {
        position: new Vec3(1, 64, 3),
        velocity: new Vec3(0.25, 0, -0.5),
        bounds: new AABB(0.7, 64, 2.7, 1.3, 65.8, 3.3),
        onGround: true,
        mode: "ground",
        jumpHeld: false,
      },
    });

    const predictionService = runtime.getPredictionService();
    expect(runtime.getPredictionService()).toBe(predictionService);
    expect(predictionService.getLastAcknowledgedSequence()).toBe(7);
    expect(predictionService.getPredictedBody().position).toEqual(new Vec3(1, 64, 3));
  });

  test("reconciles from ClientWorldPredictionView movement and revision facts", () => {
    const initialBody = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const authoritative: MovementAuthoritativeState = {
      body: initialBody,
      lastProcessedCommandSeq: 0,
      physicsRevision: 1,
      collisionRevision: 7,
    };
    const predictionView: ClientWorldPredictionView = {
      movementPhysicsRevision: 2,
      collisionRevision: 8,
      getAuthoritativeMovementState: () => authoritative,
      getEntitySnapshots: () => [],
      createCollisionWorld: () => createStaticCollisionWorld([floor()]),
    };
    const predictionService = new PlayerMovementPredictionService(
      new PlayerMovementPredictor(initialBody),
    );

    const result = predictionService.reconcileClientWorldSnapshot({
      clientWorld: predictionView,
      physicsParams: DEFAULT_MOVEMENT_PHYSICS,
    });

    expect(result).toBeDefined();
    expect(result!.diagnostics.causeTags).toEqual(expect.arrayContaining([
      "physics_revision_mismatch",
      "collision_revision_mismatch",
    ]));
  });
});
