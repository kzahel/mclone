import { afterEach, describe, expect, test } from "vitest";

import cowFixture from "../fixtures/creatures/overworld-seed-12345-chunk-2--18-entities.json";
import sheepFixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId, GENERATED_WORLD_STORAGE_VERSION, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";
import { EntityTypes, GeneratedMobEntity } from "../../src/world/entity/entity-type";
import { Goal, GoalFlag } from "../../src/world/entity/ai/goal/goal";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { FlatGrassWorldGenerator, SmallIslandWorldGenerator } from "../../src/worldgen/levelgen/demo-world-generators";
import type { GenerationEntitySink, NaturalSpawnerOptions } from "../../src/worldgen/levelgen/natural-spawner";
import {
  type CreatureGenerationFixture,
} from "../../src/oracle/integration/creature-fixture";
import type { EntitySnapshot, EntitySnapshotMessage, EntityUpdateMessage } from "../../src/runtime/protocol/world-messages";

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

class MoveOnceGoal extends Goal {
  private used = false;

  public constructor(
    private readonly cow: GeneratedMobEntity,
    private readonly target: BlockPos,
  ) {
    super();
    this.setFlags([GoalFlag.MOVE]);
  }

  public canUse(): boolean {
    return !this.used;
  }

  public override canContinueToUse(): boolean {
    return !this.cow.getNavigation().isDone();
  }

  public override start(): void {
    this.used = true;
    this.cow.getNavigation().moveTo(this.target.getX() + 0.5, this.target.getY(), this.target.getZ() + 0.5, 1.0);
  }
}

class MovingCowFlatGenerator extends FlatGrassWorldGenerator {
  public override spawnOriginalMobs(
    _level: WorldGenLevel,
    chunkX: number,
    chunkZ: number,
    sink: GenerationEntitySink,
    options: NaturalSpawnerOptions = {},
  ): void {
    if (chunkX !== 0 || chunkZ !== 0) {
      return;
    }

    const id = options.nextEntityId?.() ?? 1;
    const cow = new GeneratedMobEntity({
      id,
      uuid: options.nextEntityUuid?.(id) ?? "mclone:test/moving-cow",
      entityType: EntityTypes.COW,
      x: 8.5,
      y: 64,
      z: 8.5,
      yaw: 0,
      pitch: 0,
      onGround: true,
      randomSeed: 0,
    });
    cow.goalSelector.addGoal(0, new MoveOnceGoal(cow, new BlockPos(10, 64, 8)));
    sink.addFreshEntityWithPassengers(cow);
  }
}

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

  test("publishes entity updates when a ticking generated cow moves", async () => {
    const blocks = registerGeneratedRenderBlocks();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new MovingCowFlatGenerator(12345n),
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      liquidSimulationMode: "none",
      worldTickIntervalMs: 1,
      nowMs: () => nowMs,
    });

    await host.openWorld({ ...OPEN_WORLD_REQUEST, preset: "flat_grass" });
    const initialMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    const initialCow = initialMessages.find(
      (message): message is EntitySnapshotMessage => message.type === "entity_snapshot" && message.entity.typeId === "minecraft:cow",
    );
    expect(initialCow).toBeDefined();
    expect(initialCow).toMatchObject({
      type: "entity_snapshot",
      entity: {
        position: { x: 8.5, y: 64, z: 8.5 },
        tick: 0,
      },
    });

    nowMs = 1;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });
    const cowUpdate = updates.find(
      (message): message is EntityUpdateMessage => message.type === "entity_update" && message.update.id === initialCow!.entity.id,
    );

    expect(cowUpdate).toBeDefined();
    expect(cowUpdate).toMatchObject({
      type: "entity_update",
      update: {
        id: initialCow!.entity.id,
        tick: 1,
      },
    });
    expect(cowUpdate!.update.position).toBeDefined();
    expect(cowUpdate!.update.position).not.toEqual(initialCow!.entity.position);
  });

  test("publishes small-island starter cows near the origin chunk", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new SmallIslandWorldGenerator(12345n),
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      liquidSimulationMode: "none",
    });

    await host.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "small_island",
    });
    const messages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const cows = messages
      .filter((message): message is EntitySnapshotMessage => message.type === "entity_snapshot" && message.entity.typeId === "minecraft:cow")
      .sort((left, right) => left.entity.id - right.entity.id);

    expect(cows).toHaveLength(4);
    expect(cows.map((message) => [message.entity.position.x, message.entity.position.z])).toEqual([
      [6.5, 6.5],
      [10.5, 7.5],
      [7.5, 11.5],
      [12.5, 12.5],
    ]);
    expect(cows.every((message) => message.entity.chunkX === 0 && message.entity.chunkZ === 0)).toBe(true);
  });
});
