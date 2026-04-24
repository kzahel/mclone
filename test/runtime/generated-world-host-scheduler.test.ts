import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { GeneratedWorldHost, getGeneratedWorldViewChunkRadius } from "../../src/runtime/host/generated-world-host";
import type { ChunkSnapshotMessage, WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;

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

async function drainChunkSnapshots(host: GeneratedWorldHost, expectedCount: number): Promise<ChunkSnapshotMessage[]> {
  const snapshots: ChunkSnapshotMessage[] = [];
  for (let attempt = 0; attempt < 200; attempt++) {
    await sleep();
    const messages = await host.pollUpdates({ type: "poll_world_updates" });
    snapshots.push(...chunkSnapshots(messages));
    if (snapshots.length >= expectedCount) {
      return snapshots;
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
  });

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
  });
});
