import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vitest";
import { GeneratedWorldRemoteService } from "../../src/runtime/node/generated-world-http-server";
import {
  PLAYER_ENTITY_DATA_KIND,
  PLAYER_ENTITY_TYPE_ID,
  type EntitySnapshot,
  type OpenWorldRequest,
  type WorldHostMessage,
} from "../../src/runtime/protocol/world-messages";

const OPEN_WORLD_REQUEST: OpenWorldRequest = {
  type: "open_world",
  seed: 12345n,
  preset: "browser_smoke",
  storageMode: "none",
  playerProfile: { name: "Remote Player" },
};

function findSessionState(messages: readonly WorldHostMessage[]): Extract<WorldHostMessage, { type: "session_state" }>["state"] {
  const message = messages.find((candidate) => candidate.type === "session_state");
  if (message?.type !== "session_state") {
    throw new Error("expected session_state message");
  }
  return message.state;
}

function remotePlayerEntities(messages: readonly WorldHostMessage[]): readonly EntitySnapshot[] {
  return messages
    .filter((message): message is Extract<WorldHostMessage, { type: "entity_snapshot" }> => message.type === "entity_snapshot")
    .map((message) => message.entity)
    .filter((entity) => entity.typeId === PLAYER_ENTITY_TYPE_ID);
}

describe("remote player entity snapshots", () => {
  test("publishes other player slots as minecraft:player entities and filters local self", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await mkdtemp(path.join(tmpdir(), "mclone-remote-player-snapshots-")),
    });
    try {
      const firstOpen = await service.openWorldMessages({
        ...OPEN_WORLD_REQUEST,
        playerProfile: { name: "Remote Player One" },
      });
      const secondOpen = await service.openWorldMessages({
        ...OPEN_WORLD_REQUEST,
        playerProfile: { name: "Remote Player Two" },
      });
      const firstSession = findSessionState(firstOpen);
      const secondSession = findSessionState(secondOpen);

      await service.setChunkViewMessages(firstSession.sessionId, {
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      });
      await service.setChunkViewMessages(secondSession.sessionId, {
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      });

      const firstUpdates = await service.drainSessionUpdates(firstSession.sessionId, {
        type: "poll_world_updates",
        maxMessages: 200,
      });
      const secondUpdates = await service.drainSessionUpdates(secondSession.sessionId, {
        type: "poll_world_updates",
        maxMessages: 200,
      });
      const firstRemotePlayers = remotePlayerEntities(firstUpdates);
      const secondRemotePlayers = remotePlayerEntities(secondUpdates);

      expect(firstRemotePlayers.map((entity) => entity.data?.playerId)).toContain(secondSession.playerId);
      expect(firstRemotePlayers.map((entity) => entity.data?.playerId)).not.toContain(firstSession.playerId);
      expect(secondRemotePlayers.map((entity) => entity.data?.playerId)).toContain(firstSession.playerId);
      expect(secondRemotePlayers.map((entity) => entity.data?.playerId)).not.toContain(secondSession.playerId);
      expect(firstRemotePlayers[0]?.data?.kind).toBe(PLAYER_ENTITY_DATA_KIND);
      expect(firstRemotePlayers[0]?.width).toBe(0.6);
      expect(firstRemotePlayers[0]?.height).toBe(1.8);
    } finally {
      service.dispose();
      service.clearSessions();
    }
  });

  test("removes dropped player slots with explicit entity lifecycle messages", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await mkdtemp(path.join(tmpdir(), "mclone-remote-player-updates-")),
    });
    try {
      const firstOpen = await service.openWorldMessages({
        ...OPEN_WORLD_REQUEST,
        playerProfile: { name: "Remote Player One" },
      });
      const secondOpen = await service.openWorldMessages({
        ...OPEN_WORLD_REQUEST,
        playerProfile: { name: "Remote Player Two" },
      });
      const firstSession = findSessionState(firstOpen);
      const secondSession = findSessionState(secondOpen);

      await service.setChunkViewMessages(firstSession.sessionId, {
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      });
      await service.setChunkViewMessages(secondSession.sessionId, {
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      });
      const initialUpdates = await service.drainSessionUpdates(firstSession.sessionId, {
        type: "poll_world_updates",
        maxMessages: 200,
      });
      const remotePlayer = remotePlayerEntities(initialUpdates).find((entity) => entity.data?.playerId === secondSession.playerId);
      expect(remotePlayer).toBeDefined();

      service.dropSession(secondSession.sessionId);
      const removalUpdates = await service.drainSessionUpdates(firstSession.sessionId, {
        type: "poll_world_updates",
        maxMessages: 200,
      });
      expect(removalUpdates).toContainEqual({
        type: "entity_remove",
        entityId: remotePlayer!.id,
        uuid: remotePlayer!.uuid,
        reason: "unloaded_with_player",
      });
    } finally {
      service.dispose();
      service.clearSessions();
    }
  });
});
