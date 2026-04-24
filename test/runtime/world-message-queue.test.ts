import { describe, expect, test } from "vitest";
import { drainWorldHostMessages } from "../../src/runtime/protocol/world-message-queue";
import type { WorldHostMessage } from "../../src/runtime/protocol/world-messages";

const CHUNK_MESSAGE = {
  type: "chunk_snapshot",
  snapshot: {
    chunkX: 0,
    chunkZ: 0,
    biomes: [],
    sections: [],
    blockTicks: [],
    liquidTicks: [],
  },
} satisfies WorldHostMessage;

const LIGHT_DELTA_MESSAGE = {
  type: "chunk_light_delta",
  chunkX: 0,
  chunkZ: 0,
  light: { block: [{ y: 4 }] },
} satisfies WorldHostMessage;

describe("world host message queue draining", () => {
  test("keeps player and session messages ahead of bulk chunk snapshots when capped", () => {
    const firstChunk = {
      ...CHUNK_MESSAGE,
      snapshot: {
        ...CHUNK_MESSAGE.snapshot,
        chunkX: 1,
      },
    } satisfies WorldHostMessage;
    const secondChunk = {
      ...CHUNK_MESSAGE,
      snapshot: {
        ...CHUNK_MESSAGE.snapshot,
        chunkX: 2,
      },
    } satisfies WorldHostMessage;
    const playerState = {
      type: "player_state",
      state: {
        playerId: "test",
        position: { x: 0, y: 0, z: 0 },
        rotation: { yaw: 0, pitch: 0 },
        acknowledgedInputSequence: 1,
        tick: 1,
        revision: 1,
      },
    } satisfies WorldHostMessage;

    const drained = drainWorldHostMessages([firstChunk, secondChunk, playerState], 2);

    expect(drained.messages.map((message) => message.type)).toEqual(["player_state", "chunk_snapshot"]);
    expect(drained.remaining).toEqual([secondChunk]);
  });

  test("preserves all pending messages when capped at zero", () => {
    const drained = drainWorldHostMessages([CHUNK_MESSAGE], 0);

    expect(drained.messages).toEqual([]);
    expect(drained.remaining).toEqual([CHUNK_MESSAGE]);
  });

  test("keeps light deltas behind session-critical messages when capped", () => {
    const playerState = {
      type: "player_state",
      state: {
        playerId: "test",
        position: { x: 0, y: 0, z: 0 },
        rotation: { yaw: 0, pitch: 0 },
        acknowledgedInputSequence: 1,
        tick: 1,
        revision: 1,
      },
    } satisfies WorldHostMessage;

    const drained = drainWorldHostMessages([LIGHT_DELTA_MESSAGE, playerState], 1);

    expect(drained.messages).toEqual([playerState]);
    expect(drained.remaining).toEqual([LIGHT_DELTA_MESSAGE]);
  });
});
