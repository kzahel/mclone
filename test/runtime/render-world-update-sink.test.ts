import { describe, expect, test } from "vitest";
import { TransportWorldClient, type RenderWorldUpdateMessage, type RenderWorldUpdateSink, type WorldTransport } from "../../src/runtime/transport/local-world-transport";
import type {
  ClientPlayerState,
  ClientSessionState,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../../src/runtime/protocol/world-messages";
import type { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { AABB } from "../../src/world/phys/aabb";
import type { PackedChunkLightDelta, PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";

const OPENED: WorldOpenedMessage = {
  type: "world_opened",
  minBuildHeight: 0,
  height: 256,
  saveMetadata: {
    saveId: "test-save",
    storageVersion: 1,
    seed: "12345",
    preset: "default",
    minBuildHeight: 0,
    height: 256,
    createdAtMs: 1,
    lastOpenedAtMs: 1,
  },
};

const SESSION_STATE: ClientSessionState = {
  sessionId: "session",
  playerId: "player",
  playerProfile: { name: "Player" },
  saveId: "test-save",
  resumed: false,
  revision: 1,
  chunkView: {
    centerChunkX: 0,
    centerChunkZ: 0,
    radius: 1,
  },
};

const PLAYER_STATE: ClientPlayerState = {
  playerId: "player",
  position: { x: 8, y: 80, z: 8 },
  rotation: { yaw: 0, pitch: 0 },
  acknowledgedInputSequence: 0,
  tick: 1,
  revision: 1,
};

const SNAPSHOT: PackedChunkSnapshot = {
  chunkX: 0,
  chunkZ: 0,
  biomes: [],
  sections: [],
  blockTicks: [],
  liquidTicks: [],
};

const LIGHT_DELTA: PackedChunkLightDelta = {
  block: [{ y: 4 }],
};

class StaticWorldTransport implements WorldTransport {
  public constructor(private readonly chunkViewMessages: readonly WorldHostMessage[]) {}

  public openWorld(_request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([OPENED]);
  }

  public setChunkView(_request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve(this.chunkViewMessages);
  }

  public setPlayerInput(_request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }

  public pollUpdates(_request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return Promise.resolve([]);
  }
}

function createRecordingLevel(): {
  readonly level: ClientChunkCache;
  readonly snapshots: PackedChunkSnapshot[];
  readonly lightDeltas: Array<{ readonly chunkX: number; readonly chunkZ: number; readonly light: PackedChunkLightDelta }>;
  readonly unloads: Array<{ readonly chunkX: number; readonly chunkZ: number }>;
} {
  const snapshots: PackedChunkSnapshot[] = [];
  const lightDeltas: Array<{ readonly chunkX: number; readonly chunkZ: number; readonly light: PackedChunkLightDelta }> = [];
  const unloads: Array<{ readonly chunkX: number; readonly chunkZ: number }> = [];
  return {
    level: {
      applyPackedChunkSnapshot(snapshot: PackedChunkSnapshot): void {
        snapshots.push(snapshot);
      },
      applyChunkLightDelta(delta: { readonly chunkX: number; readonly chunkZ: number; readonly light: PackedChunkLightDelta }): boolean {
        lightDeltas.push(delta);
        return true;
      },
      applyChunkUnload(chunkX: number, chunkZ: number): boolean {
        unloads.push({ chunkX, chunkZ });
        return true;
      },
      getChunkSnapshot() {
        return undefined;
      },
    } as unknown as ClientChunkCache,
    snapshots,
    lightDeltas,
    unloads,
  };
}

function createRecordingSink(): {
  readonly sink: RenderWorldUpdateSink;
  readonly batches: RenderWorldUpdateMessage[][];
} {
  const batches: RenderWorldUpdateMessage[][] = [];
  return {
    sink: {
      ingestUpdates(messages: readonly RenderWorldUpdateMessage[]) {
        batches.push([...messages]);
        return { chunkChanged: true };
      },
    },
    batches,
  };
}

describe("RenderWorld update sink", () => {
  test("hydrates the client cache and forwards chunk messages to a sink", async () => {
    const recordingLevel = createRecordingLevel();
    const recordingSink = createRecordingSink();
    const client = new TransportWorldClient(
      new StaticWorldTransport([
        { type: "session_state", state: SESSION_STATE },
        { type: "player_state", state: PLAYER_STATE },
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ]),
      () => recordingLevel.level,
      { chunkUpdateSink: recordingSink.sink },
    );

    await client.openWorld({ type: "open_world", seed: 12345n, preset: "default" });

    await expect(client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).resolves.toBe(true);

    expect(client.getSessionState()).toEqual(SESSION_STATE);
    expect(client.getPlayerState()).toEqual(PLAYER_STATE);
    expect(client.getClientWorld().getSessionState()).toEqual(SESSION_STATE);
    expect(client.getClientWorld().getLocalPlayerState()).toEqual(PLAYER_STATE);
    expect(recordingSink.batches).toEqual([
      [
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ],
    ]);
    expect(recordingLevel.snapshots).toEqual([SNAPSHOT]);
    expect(recordingLevel.lightDeltas).toEqual([{ type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA }]);
    expect(recordingLevel.unloads).toEqual([{ chunkX: 1, chunkZ: -1 }]);
  });

  test("can install the render sink after opening the world", async () => {
    const recordingLevel = createRecordingLevel();
    const recordingSink = createRecordingSink();
    const client = new TransportWorldClient(
      new StaticWorldTransport([
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ]),
      () => recordingLevel.level,
    );

    await client.openWorld({ type: "open_world", seed: 12345n, preset: "default" });
    client.setRenderWorldUpdateSink(recordingSink.sink);
    await expect(client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).resolves.toBe(true);

    expect(recordingSink.batches).toEqual([
      [
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ],
    ]);
    expect(recordingLevel.snapshots).toEqual([SNAPSHOT]);
    expect(recordingLevel.lightDeltas).toEqual([{ type: "chunk_light_delta", chunkX: 0, chunkZ: 0, light: LIGHT_DELTA }]);
    expect(recordingLevel.unloads).toEqual([{ chunkX: 1, chunkZ: -1 }]);
  });

  test("stores world performance snapshots without reporting visible world changes", async () => {
    const recordingLevel = createRecordingLevel();
    const client = new TransportWorldClient(
      new StaticWorldTransport([
        {
          type: "world_perf",
          performance: {},
        },
      ]),
      () => recordingLevel.level,
    );

    await client.openWorld({ type: "open_world", seed: 12345n, preset: "default" });

    await expect(client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).resolves.toBe(false);

    expect(client.getPerformanceSnapshot()).toEqual({});
    expect(recordingLevel.snapshots).toEqual([]);
  });

  test("reports missing collision data for chunks not hydrated by the host", async () => {
    const recordingLevel = createRecordingLevel();
    const client = new TransportWorldClient(
      new StaticWorldTransport([]),
      () => recordingLevel.level,
    );

    await client.openWorld({ type: "open_world", seed: 12345n, preset: "default" });

    const collisionWorld = client.getClientWorld().getPredictionView().createCollisionWorld();
    expect(collisionWorld.queryBlockCollisions(new AABB(2, 64, 2, 3, 65, 3))).toEqual({
      type: "missing",
      reason: "missing_chunk:0,0",
    });
  });
});
