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

const ENTITY_SNAPSHOT_MESSAGE = {
  type: "entity_snapshot",
  entity: {
    id: 1,
    uuid: "test:entity/1",
    typeId: "minecraft:sheep",
    category: "creature",
    chunkX: 0,
    chunkZ: 0,
    position: { x: 0, y: 64, z: 0 },
    rotation: { yaw: 0, pitch: 0 },
    width: 0.9,
    height: 1.3,
    onGround: true,
  },
} satisfies WorldHostMessage;

const ENTITY_UPDATE_MESSAGE = {
  type: "entity_update",
  update: {
    id: 1,
    position: { x: 1, y: 64, z: 0 },
    rotation: { yaw: 5, pitch: 0 },
  },
} satisfies WorldHostMessage;

const ENTITY_REMOVE_MESSAGE = {
  type: "entity_remove",
  entityId: 1,
  uuid: "test:entity/1",
  reason: "discarded",
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

  test("keeps entity snapshots behind session-critical messages when capped", () => {
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

    const drained = drainWorldHostMessages([ENTITY_SNAPSHOT_MESSAGE, playerState], 1);

    expect(drained.messages).toEqual([playerState]);
    expect(drained.remaining).toEqual([ENTITY_SNAPSHOT_MESSAGE]);
  });

  test("keeps entity updates behind session-critical messages when capped", () => {
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

    const drained = drainWorldHostMessages([ENTITY_UPDATE_MESSAGE, playerState], 1);

    expect(drained.messages).toEqual([playerState]);
    expect(drained.remaining).toEqual([ENTITY_UPDATE_MESSAGE]);
  });

  test("keeps entity removes in the session-critical lane when capped", () => {
    const drained = drainWorldHostMessages([CHUNK_MESSAGE, ENTITY_REMOVE_MESSAGE], 1);

    expect(drained.messages).toEqual([ENTITY_REMOVE_MESSAGE]);
    expect(drained.remaining).toEqual([CHUNK_MESSAGE]);
  });

  test("drains latest progress outside the bulk message cap", () => {
    const progress = {
      type: "world_progress",
      stage: "Publishing chunks",
      current: 1,
      total: 2,
    } satisfies WorldHostMessage;

    const drained = drainWorldHostMessages([CHUNK_MESSAGE, progress], 1);

    expect(drained.messages).toEqual([progress, CHUNK_MESSAGE]);
    expect(drained.remaining).toEqual([]);
  });

  test("drains progress even when bulk messages are capped at zero", () => {
    const progress = {
      type: "world_progress",
      stage: "Publishing chunks",
      current: 1,
      total: 2,
    } satisfies WorldHostMessage;

    const drained = drainWorldHostMessages([CHUNK_MESSAGE, progress], 0);

    expect(drained.messages).toEqual([progress]);
    expect(drained.remaining).toEqual([CHUNK_MESSAGE]);
  });

  test("drops stale progress records when draining the latest progress", () => {
    const staleProgress = {
      type: "world_progress",
      stage: "Publishing chunks",
      current: 1,
      total: 3,
    } satisfies WorldHostMessage;
    const latestProgress = {
      type: "world_progress",
      stage: "Publishing chunks",
      current: 2,
      total: 3,
    } satisfies WorldHostMessage;

    const drained = drainWorldHostMessages([staleProgress, CHUNK_MESSAGE, latestProgress], 0);

    expect(drained.messages).toEqual([latestProgress]);
    expect(drained.remaining).toEqual([CHUNK_MESSAGE]);
  });
});
