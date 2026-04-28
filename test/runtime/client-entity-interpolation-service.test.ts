import { describe, expect, test } from "vitest";
import { WorldClientRuntimeFacade } from "../../src/runtime/client/client-runtime";
import { ClientEntityInterpolationService } from "../../src/runtime/client";
import { HostMessageClientWorld } from "../../src/runtime/client/client-world";
import { TransportWorldClient, type WorldTransport } from "../../src/runtime/transport/local-world-transport";
import type { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import type {
  EntitySnapshot,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../../src/runtime/protocol/world-messages";

const OPENED: WorldOpenedMessage = {
  type: "world_opened",
  minBuildHeight: 0,
  height: 256,
  saveMetadata: {
    saveId: "entity-interpolation-test",
    storageVersion: 1,
    seed: "12345",
    preset: "default",
    minBuildHeight: 0,
    height: 256,
    createdAtMs: 1,
    lastOpenedAtMs: 1,
  },
};

function entity(overrides: Partial<EntitySnapshot> = {}): EntitySnapshot {
  return {
    id: 1,
    uuid: "00000000-0000-0000-0000-000000000001",
    typeId: "minecraft:sheep",
    category: "creature",
    chunkX: 0,
    chunkZ: 0,
    position: { x: 0, y: 64, z: 0 },
    rotation: { yaw: 350, pitch: 0 },
    width: 0.9,
    height: 1.3,
    onGround: true,
    ...overrides,
  };
}

class PollMessagesTransport implements WorldTransport {
  public constructor(
    private readonly openMessages: readonly WorldHostMessage[],
    private readonly pollMessages: readonly WorldHostMessage[],
  ) {}

  public openWorld(_request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve(this.openMessages);
  }

  public setChunkView(_request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public setPlayerInput(_request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public pollUpdates(_request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve(this.pollMessages);
  }
}

describe("Client entity interpolation service", () => {
  test("hydrates entity update and remove lifecycle messages", async () => {
    const first = entity();
    const clientWorld = new HostMessageClientWorld(() => ({}) as ClientChunkCache);

    await clientWorld.hydrateHostMessages([OPENED, { type: "entity_snapshot", entity: first }]);
    await clientWorld.hydrateHostMessages([{
      type: "entity_update",
      update: {
        id: first.id,
        position: { x: 32, y: 65, z: -1 },
        rotation: { yaw: 10, pitch: 5 },
        onGround: false,
        age: 3,
      },
    }]);

    expect(clientWorld.getEntitySnapshots()).toEqual([{
      ...first,
      chunkX: 2,
      chunkZ: -1,
      position: { x: 32, y: 65, z: -1 },
      rotation: { yaw: 10, pitch: 5 },
      onGround: false,
      age: 3,
    }]);

    await clientWorld.hydrateHostMessages([{ type: "entity_remove", entityId: first.id, reason: "discarded" }]);
    expect(clientWorld.getEntitySnapshots()).toEqual([]);
  });

  test("buffers authoritative entity snapshots for visual-only interpolation", () => {
    const service = new ClientEntityInterpolationService({ interpolationDurationMs: 100 });
    const first = entity();
    const second = entity({
      position: { x: 10, y: 64, z: 0 },
      rotation: { yaw: 10, pitch: 20 },
      age: 1,
    });

    service.syncClientWorldEntities({ getEntitySnapshots: () => [first] }, 1_000);
    expect(service.publishPresentationEntities(1_000)).toMatchObject([
      {
        entityId: 1,
        typeId: "minecraft:sheep",
        interpolationAlpha: 1,
        interpolatedPosition: { x: 0, y: 64, z: 0 },
        interpolatedRotation: { yaw: 350, pitch: 0 },
        aiAuthority: "host",
      },
    ]);

    service.syncClientWorldEntities({ getEntitySnapshots: () => [second] }, 1_100);
    expect(service.publishPresentationEntities(1_150)).toMatchObject([
      {
        entityId: 1,
        interpolationAlpha: 0.5,
        interpolatedPosition: { x: 5, y: 64, z: 0 },
        interpolatedRotation: { yaw: 0, pitch: 10 },
        authoritative: second,
        previousAuthoritative: first,
        aiAuthority: "host",
      },
    ]);

    expect(service.publishPresentationEntities(1_250)[0]).toMatchObject({
      interpolationAlpha: 1,
      interpolatedPosition: { x: 10, y: 64, z: 0 },
      interpolatedRotation: { yaw: 10, pitch: 20 },
    });

    service.syncClientWorldEntities({ getEntitySnapshots: () => [] }, 1_300);
    expect(service.publishPresentationEntities(1_300)).toEqual([]);
  });

  test("ClientRuntime publishes raw authoritative and interpolated entity state", async () => {
    const first = entity();
    const second = entity({
      position: { x: 10, y: 64, z: 0 },
      rotation: { yaw: 10, pitch: 20 },
      age: 1,
    });
    const client = new TransportWorldClient(
      new PollMessagesTransport(
        [OPENED, { type: "entity_snapshot", entity: first }],
        [{ type: "entity_snapshot", entity: second }],
      ),
      () => ({}) as ClientChunkCache,
    );
    const runtime = new WorldClientRuntimeFacade(client);

    await runtime.openWorld({ type: "open_world", seed: 12345n, preset: "default" });
    const initial = runtime.publishPresentationState({ entityInterpolationTimeMs: 1_000 });
    expect(initial.entities).toEqual([first]);
    expect(initial.entityPresentation).toMatchObject([
      {
        entityId: 1,
        interpolatedPosition: { x: 0, y: 64, z: 0 },
        authoritative: first,
        aiAuthority: "host",
      },
    ]);

    await expect(runtime.drainTransportUpdates()).resolves.toBe(true);
    runtime.publishPresentationState({ entityInterpolationTimeMs: 1_100 });
    const updated = runtime.publishPresentationState({ entityInterpolationTimeMs: 1_150 });

    expect(updated.entities).toEqual([second]);
    expect(updated.entityPresentation).toMatchObject([
      {
        entityId: 1,
        interpolatedPosition: { x: 5, y: 64, z: 0 },
        interpolatedRotation: { yaw: 0, pitch: 10 },
        authoritative: second,
        previousAuthoritative: first,
        aiAuthority: "host",
      },
    ]);
    expect(runtime.getEntityInterpolationService().publishPresentationEntities(1_200)[0]).toMatchObject({
      interpolatedPosition: { x: 10, y: 64, z: 0 },
    });
  });
});
