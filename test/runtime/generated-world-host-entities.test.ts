import { afterEach, describe, expect, test } from "vitest";

import cowFixture from "../fixtures/creatures/overworld-seed-12345-chunk-2--18-entities.json";
import sheepFixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId, GENERATED_WORLD_STORAGE_VERSION, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import {
  type CreatureGenerationFixture,
} from "../../src/oracle/integration/creature-fixture";
import type { EntitySnapshot } from "../../src/runtime/protocol/world-messages";

const creatureFixtures = [
  ["sheep", sheepFixture as unknown as CreatureGenerationFixture, "minecraft:sheep"],
  ["cow", cowFixture as unknown as CreatureGenerationFixture, "minecraft:cow"],
] as const;
const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;
const GENERATED_WORLD_ENTITIES_TIMEOUT_MS = 30_000;

function createWorldClient(): LocalWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  return new LocalWorldClient(
    new LocalWorldTransport(
      new GeneratedWorldHost({
        seed: 12345n,
        airState: blocks.airState,
        blockStateById: blocks.blockStateById,
        blockStateIds: blocks.blockStateIds,
        lightingMode: "none",
      }),
    ),
    (worldOpened) => new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: worldOpened.minBuildHeight,
      height: worldOpened.height,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    }),
  );
}

function targetChunkEntitySnapshots(
  creatureFixture: CreatureGenerationFixture,
  entities: readonly EntitySnapshot[],
): readonly EntitySnapshot[] {
  const chunk = creatureFixture.chunks[0]!;
  return entities
    .filter((entity) => entity.chunkX === chunk.chunkX && entity.chunkZ === chunk.chunkZ)
    .sort((left, right) => left.id - right.id);
}

describe("GeneratedWorldHost entity publication", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test.each(creatureFixtures)("publishes generation-time %s entities as host-owned entity snapshots", async (_name, creatureFixture, expectedType) => {
    const client = createWorldClient();
    await expect(client.openWorld(OPEN_WORLD_REQUEST)).resolves.toMatchObject({
      type: "world_opened",
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "default"),
        storageVersion: GENERATED_WORLD_STORAGE_VERSION,
      },
    });

    const chunk = creatureFixture.chunks[0]!;
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: chunk.chunkX,
      centerChunkZ: chunk.chunkZ,
      radius: 1,
    });

    const snapshots = targetChunkEntitySnapshots(creatureFixture, client.getEntitySnapshots());

    expect(snapshots.length).toBeGreaterThan(0);
    expect(snapshots.every((entity) => entity.category === "creature")).toBe(true);
    expect(snapshots.some((entity) => entity.typeId === expectedType)).toBe(true);
    expect(snapshots.every((entity) => entity.position.x >= chunk.chunkX * 16 && entity.position.x < (chunk.chunkX + 1) * 16)).toBe(true);
    expect(snapshots.every((entity) => entity.position.z >= chunk.chunkZ * 16 && entity.position.z < (chunk.chunkZ + 1) * 16)).toBe(true);

    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: chunk.chunkX + 8,
      centerChunkZ: chunk.chunkZ + 8,
      radius: 1,
    });

    expect(targetChunkEntitySnapshots(creatureFixture, client.getEntitySnapshots())).toEqual([]);
  }, GENERATED_WORLD_ENTITIES_TIMEOUT_MS);
});
