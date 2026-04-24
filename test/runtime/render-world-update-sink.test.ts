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
import type { PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";

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
  readonly unloads: Array<{ readonly chunkX: number; readonly chunkZ: number }>;
} {
  const snapshots: PackedChunkSnapshot[] = [];
  const unloads: Array<{ readonly chunkX: number; readonly chunkZ: number }> = [];
  return {
    level: {
      applyPackedChunkSnapshot(snapshot: PackedChunkSnapshot): void {
        snapshots.push(snapshot);
      },
      applyChunkUnload(chunkX: number, chunkZ: number): boolean {
        unloads.push({ chunkX, chunkZ });
        return true;
      },
    } as unknown as ClientChunkCache,
    snapshots,
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
  test("forwards chunk messages to a sink without mutating the client cache", async () => {
    const recordingLevel = createRecordingLevel();
    const recordingSink = createRecordingSink();
    const client = new TransportWorldClient(
      new StaticWorldTransport([
        { type: "session_state", state: SESSION_STATE },
        { type: "player_state", state: PLAYER_STATE },
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
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
    expect(recordingSink.batches).toEqual([
      [
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ],
    ]);
    expect(recordingLevel.snapshots).toEqual([]);
    expect(recordingLevel.unloads).toEqual([]);
  });

  test("can mirror chunk updates to the compatibility cache during renderer migration", async () => {
    const recordingLevel = createRecordingLevel();
    const recordingSink = createRecordingSink();
    const client = new TransportWorldClient(
      new StaticWorldTransport([
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ]),
      () => recordingLevel.level,
      {
        chunkUpdateSink: recordingSink.sink,
        mirrorChunkUpdatesToLevel: true,
      },
    );

    await client.openWorld({ type: "open_world", seed: 12345n, preset: "default" });
    await expect(client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).resolves.toBe(true);

    expect(recordingSink.batches).toEqual([
      [
        { type: "chunk_snapshot", snapshot: SNAPSHOT },
        { type: "chunk_unload", chunkX: 1, chunkZ: -1 },
      ],
    ]);
    expect(recordingLevel.snapshots).toEqual([SNAPSHOT]);
    expect(recordingLevel.unloads).toEqual([{ chunkX: 1, chunkZ: -1 }]);
  });
});
