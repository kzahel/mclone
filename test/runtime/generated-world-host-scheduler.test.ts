import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { GeneratedWorldHost, getGeneratedWorldViewChunkRadius } from "../../src/runtime/host/generated-world-host";
import type { ChunkSnapshotMessage, WorldHostMessage, WorldProgressMessage } from "../../src/runtime/protocol/world-messages";
import { DataLayer } from "../../src/world/level/chunk/data-layer";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;
const COOPERATIVE_CHUNK_LIGHTING_TIMEOUT_MS = 15_000;

function sleep(ms = 0): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function createCooperativeHost(): GeneratedWorldHost {
  const blocks = registerGeneratedRenderBlocks();
  return new GeneratedWorldHost({
    seed: 12345n,
    airState: blocks.airState,
    blockStateById: blocks.blockStateById,
    blockStateIds: blocks.blockStateIds,
    chunkViewScheduling: "cooperative",
  });
}

function countExpectedChunks(radius: number): number {
  const viewRadius = getGeneratedWorldViewChunkRadius(radius);
  return (viewRadius * 2 + 1) ** 2;
}

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function worldProgressMessages(messages: readonly WorldHostMessage[]): WorldProgressMessage[] {
  return messages.filter((message): message is WorldProgressMessage => message.type === "world_progress");
}

async function drainChunkSnapshots(host: GeneratedWorldHost, expectedCount: number): Promise<ChunkSnapshotMessage[]> {
  const snapshots: ChunkSnapshotMessage[] = [];
  const deadline = Date.now() + COOPERATIVE_CHUNK_LIGHTING_TIMEOUT_MS;
  while (Date.now() < deadline) {
    await sleep(10);
    const messages = await host.pollUpdates({ type: "poll_world_updates" });
    snapshots.push(...chunkSnapshots(messages));
    if (snapshots.length >= expectedCount) {
      return snapshots.slice(0, expectedCount);
    }
  }

  throw new Error(`expected ${expectedCount.toString()} chunk snapshots, got ${snapshots.length.toString()}`);
}

describe("GeneratedWorldHost cooperative chunk scheduler", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("acknowledges chunk interest before streaming snapshots through poll updates", async () => {
    const host = createCooperativeHost();
    await host.openWorld(OPEN_WORLD_REQUEST);

    const response = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    expect(response.map((message) => message.type)).toEqual(["session_state", "player_state"]);

    const initialProgress = worldProgressMessages(await host.pollUpdates({
      type: "poll_world_updates",
      maxMessages: 1,
    }));
    expect(initialProgress).toEqual([{
      type: "world_progress",
      stage: "Checking saved chunks",
      detail: "stored 0, existing 0, missing 0",
      current: 0,
      total: countExpectedChunks(1),
    }]);

    const inputResponse = await host.setPlayerInput({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 0,
      },
    });
    expect(inputResponse.map((message) => message.type)).toEqual(["session_state"]);

    const snapshots = await drainChunkSnapshots(host, countExpectedChunks(1));
    expect(snapshots).toHaveLength(countExpectedChunks(1));
    expect(snapshots.some((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0)).toBe(true);
    const center = snapshots.find((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0)!.snapshot;
    expect(center.light?.lightCorrect).toBe(true);
    expect(center.light?.sky.length).toBeGreaterThan(0);
    expect(center.light?.block.length).toBeGreaterThan(0);
    for (const section of [...center.light!.sky, ...center.light!.block]) {
      expect(section.data).toBeInstanceOf(Uint8Array);
      expect(section.data).toHaveLength(DataLayer.SIZE);
    }
    expect(center.light!.sky.some((section) => section.data.some((byte) => byte !== 0))).toBe(true);
  }, COOPERATIVE_CHUNK_LIGHTING_TIMEOUT_MS);

  test("drops stale snapshots when a newer chunk view supersedes queued work", async () => {
    const host = createCooperativeHost();
    await host.openWorld(OPEN_WORLD_REQUEST);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 8,
      centerChunkZ: 0,
      radius: 1,
    });

    const snapshots = await drainChunkSnapshots(host, countExpectedChunks(1));
    const viewRadius = getGeneratedWorldViewChunkRadius(1);
    for (const message of snapshots) {
      expect(Math.abs(message.snapshot.chunkX - 8)).toBeLessThanOrEqual(viewRadius);
      expect(Math.abs(message.snapshot.chunkZ - 0)).toBeLessThanOrEqual(viewRadius);
    }
  }, COOPERATIVE_CHUNK_LIGHTING_TIMEOUT_MS);
});
