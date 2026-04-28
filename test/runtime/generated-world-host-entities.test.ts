import { afterEach, describe, expect, test } from "vitest";

import cowFixture from "../fixtures/creatures/overworld-seed-12345-chunk-2--18-entities.json";
import sheepFixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId, GENERATED_WORLD_STORAGE_VERSION, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { createInitialPlayerState } from "../../src/runtime/session/player-loop";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { unpackChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { FullChunkStatus } from "../../src/world/level/entity/full-chunk-status";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";
import { EntityTypes, GeneratedMobEntity } from "../../src/world/entity/entity-type";
import { EatBlockGoal } from "../../src/world/entity/ai/goal/eat-block-goal";
import { Goal, GoalFlag } from "../../src/world/entity/ai/goal/goal";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { FlatGrassWorldGenerator, SmallIslandWorldGenerator } from "../../src/worldgen/levelgen/demo-world-generators";
import type { GenerationEntitySink, NaturalSpawnerOptions } from "../../src/worldgen/levelgen/natural-spawner";
import {
  type CreatureGenerationFixture,
} from "../../src/oracle/integration/creature-fixture";
import type { ChunkSnapshotMessage, ClientPlayerState, EntitySnapshot, EntitySnapshotMessage, EntityUpdateMessage } from "../../src/runtime/protocol/world-messages";

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

class ImmediateEatBlockGoal extends EatBlockGoal {
  private used = false;

  public override canUse(): boolean {
    return !this.used;
  }

  public override start(): void {
    this.used = true;
    super.start();
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

class GrassEatingSheepFlatGenerator extends FlatGrassWorldGenerator {
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
    const sheep = new GeneratedMobEntity({
      id,
      uuid: options.nextEntityUuid?.(id) ?? "mclone:test/grass-eating-sheep",
      entityType: EntityTypes.SHEEP,
      x: 8.5,
      y: 64,
      z: 8.5,
      yaw: 0,
      pitch: 0,
      onGround: true,
      randomSeed: 0,
    });
    sheep.goalSelector.addGoal(0, new ImmediateEatBlockGoal(sheep));
    sink.addFreshEntityWithPassengers(sheep);
  }
}

class EdgeMovingCowFlatGenerator extends FlatGrassWorldGenerator {
  public override spawnOriginalMobs(
    _level: WorldGenLevel,
    chunkX: number,
    chunkZ: number,
    sink: GenerationEntitySink,
    options: NaturalSpawnerOptions = {},
  ): void {
    if (chunkX !== 2 || chunkZ !== 0) {
      return;
    }

    const id = options.nextEntityId?.() ?? 1;
    const cow = new GeneratedMobEntity({
      id,
      uuid: options.nextEntityUuid?.(id) ?? "mclone:test/edge-moving-cow",
      entityType: EntityTypes.COW,
      x: 40.5,
      y: 64,
      z: 8.5,
      yaw: 0,
      pitch: 0,
      onGround: true,
      randomSeed: 0,
    });
    cow.goalSelector.addGoal(0, new MoveOnceGoal(cow, new BlockPos(42, 64, 8)));
    sink.addFreshEntityWithPassengers(cow);
  }
}

class OverlappingCowsFlatGenerator extends FlatGrassWorldGenerator {
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

    for (const [offset, slug] of [[0.0, "left"], [0.5, "right"]] as const) {
      const id = options.nextEntityId?.() ?? 1;
      sink.addFreshEntityWithPassengers(new GeneratedMobEntity({
        id,
        uuid: options.nextEntityUuid?.(id) ?? `mclone:test/overlapping-cow/${slug}`,
        entityType: EntityTypes.COW,
        x: 8.5 + offset,
        y: 64,
        z: 8.5,
        yaw: 0,
        pitch: 0,
        onGround: true,
        randomSeed: id,
      }));
    }
  }
}

class PlayerOverlappingCowFlatGenerator extends FlatGrassWorldGenerator {
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
    sink.addFreshEntityWithPassengers(new GeneratedMobEntity({
      id,
      uuid: options.nextEntityUuid?.(id) ?? "mclone:test/player-overlapping-cow",
      entityType: EntityTypes.COW,
      x: 8.9,
      y: 64,
      z: 8.5,
      yaw: 0,
      pitch: 0,
      onGround: true,
      randomSeed: id,
    }));
  }
}

function createPlayerStateAt(playerId: string, x: number, y: number, z: number): ClientPlayerState {
  const state = createInitialPlayerState(playerId);
  const body = state.movementBody!;
  const dx = x - body.position.x;
  const dy = y - body.position.y;
  const dz = z - body.position.z;
  return {
    ...state,
    position: { x, y, z },
    movementBody: {
      ...body,
      position: { x, y, z },
      bounds: {
        minX: body.bounds.minX + dx,
        minY: body.bounds.minY + dy,
        minZ: body.bounds.minZ + dz,
        maxX: body.bounds.maxX + dx,
        maxY: body.bounds.maxY + dy,
        maxZ: body.bounds.maxZ + dz,
      },
    },
  };
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

function packedSnapshotBlockNameAt(
  snapshot: ReturnType<typeof unpackChunkSnapshot>,
  x: number,
  y: number,
  z: number,
): string | undefined {
  const section = snapshot.sections.find((candidate) => candidate.y === Math.floor(y / 16));
  if (section === undefined) {
    return undefined;
  }

  const blockIndex = ((((y & 15) * 16) + (z & 15)) * 16) + (x & 15);
  return section.palette[section.blocks[blockIndex] ?? -1]?.name;
}

function statusChunkPairs(host: GeneratedWorldHost): readonly string[] {
  return host.getDebugEntityChunkStatusRecords()
    .map((record) => `${record.chunkX.toString()},${record.chunkZ.toString()}:${record.status}`);
}

function chunkFullStatusFor(host: GeneratedWorldHost, chunkX: number, chunkZ: number): FullChunkStatus | undefined {
  return host.getDebugChunkFullStatusRecords()
    .find((record) => record.chunkX === chunkX && record.chunkZ === chunkZ)
    ?.status;
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

  test("publishes dirty chunk snapshots when a generated sheep eats grass", async () => {
    const blocks = registerGeneratedRenderBlocks();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new GrassEatingSheepFlatGenerator(12345n),
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      liquidSimulationMode: "none",
      worldTickIntervalMs: 1,
      nowMs: () => nowMs,
    });

    await host.openWorld({ ...OPEN_WORLD_REQUEST, preset: "flat_grass" });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    nowMs = 36;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });
    const dirtySnapshot = updates.find((message): message is ChunkSnapshotMessage => (
      message.type === "chunk_snapshot" && message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0
    ));

    expect(dirtySnapshot).toBeDefined();
    const unpacked = unpackChunkSnapshot(dirtySnapshot!.snapshot, blocks.blockStateIds);
    expect(packedSnapshotBlockNameAt(unpacked, 8, 63, 8)).toBe("minecraft:dirt");
  });

  test("keeps entity chunk status on the explicit entity ticket after a chunk walk settles", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new FlatGrassWorldGenerator(12345n),
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      liquidSimulationMode: "none",
    });

    await host.openWorld({ ...OPEN_WORLD_REQUEST, preset: "flat_grass" });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });

    const initialStatuses = host.getDebugEntityChunkStatusRecords();
    expect(initialStatuses).toHaveLength(25);
    expect(initialStatuses.filter((record) => record.status === FullChunkStatus.ENTITY_TICKING)).toHaveLength(1);
    expect(initialStatuses.filter((record) => record.status === FullChunkStatus.TICKING)).toHaveLength(8);
    expect(initialStatuses.filter((record) => record.status === FullChunkStatus.BORDER)).toHaveLength(16);
    expect(statusChunkPairs(host)).toContain(`0,0:${FullChunkStatus.ENTITY_TICKING}`);
    expect(statusChunkPairs(host)).toContain(`1,0:${FullChunkStatus.TICKING}`);
    expect(statusChunkPairs(host)).toContain(`-2,0:${FullChunkStatus.BORDER}`);
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.BORDER}`);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 1,
      centerChunkZ: 0,
      radius: 0,
    });
    await host.flushStorageSideEffects();

    const settledStatuses = host.getDebugEntityChunkStatusRecords();
    expect(settledStatuses).toHaveLength(25);
    expect(settledStatuses.filter((record) => record.status === FullChunkStatus.ENTITY_TICKING)).toHaveLength(1);
    expect(settledStatuses.filter((record) => record.status === FullChunkStatus.TICKING)).toHaveLength(8);
    expect(settledStatuses.filter((record) => record.status === FullChunkStatus.BORDER)).toHaveLength(16);
    expect(statusChunkPairs(host).some((entry) => entry.startsWith("-2,0:"))).toBe(false);
    expect(statusChunkPairs(host)).toContain(`1,0:${FullChunkStatus.ENTITY_TICKING}`);
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.TICKING}`);
    expect(statusChunkPairs(host)).toContain(`3,0:${FullChunkStatus.BORDER}`);
  });

  test("drives entity chunk visibility from holder full-status transitions", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new FlatGrassWorldGenerator(12345n),
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      liquidSimulationMode: "none",
    });

    await host.openWorld({ ...OPEN_WORLD_REQUEST, preset: "flat_grass" });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });

    expect(chunkFullStatusFor(host, 2, 0)).toBe(FullChunkStatus.BORDER);
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.BORDER}`);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 0,
      radius: 0,
    });

    expect(chunkFullStatusFor(host, 2, 0)).toBe(FullChunkStatus.ENTITY_TICKING);
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.ENTITY_TICKING}`);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });

    expect(chunkFullStatusFor(host, 2, 0)).toBe(FullChunkStatus.BORDER);
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.BORDER}`);

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 5,
      centerChunkZ: 0,
      radius: 0,
    });

    expect(chunkFullStatusFor(host, 2, 0)).toBe(FullChunkStatus.INACCESSIBLE);
    expect(statusChunkPairs(host).some((entry) => entry.startsWith("2,0:"))).toBe(false);
  });

  test("keeps border-ring entity snapshots visible without ticking until promoted", async () => {
    const blocks = registerGeneratedRenderBlocks();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new EdgeMovingCowFlatGenerator(12345n),
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
      radius: 0,
    });
    const initialCow = initialMessages.find(
      (message): message is EntitySnapshotMessage => message.type === "entity_snapshot" && message.entity.typeId === "minecraft:cow",
    );

    expect(initialCow).toBeDefined();
    expect(initialCow).toMatchObject({
      type: "entity_snapshot",
      entity: {
        chunkX: 2,
        chunkZ: 0,
        position: { x: 40.5, y: 64, z: 8.5 },
        tick: 0,
      },
    });
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.BORDER}`);

    nowMs = 1;
    const trackedOnlyUpdates = await host.pollUpdates({ type: "poll_world_updates" });
    expect(trackedOnlyUpdates.some(
      (message): message is EntityUpdateMessage => message.type === "entity_update" && message.update.id === initialCow!.entity.id,
    )).toBe(false);

    nowMs = 2;
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 0,
      radius: 0,
    });
    expect(statusChunkPairs(host)).toContain(`2,0:${FullChunkStatus.ENTITY_TICKING}`);

    nowMs = 3;
    const tickingUpdates = await host.pollUpdates({ type: "poll_world_updates" });
    const cowUpdate = tickingUpdates.find(
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

  test("pushes overlapping generated mobs apart without treating them as path obstacles", async () => {
    const blocks = registerGeneratedRenderBlocks();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new OverlappingCowsFlatGenerator(12345n),
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
    const initialCows = initialMessages
      .filter((message): message is EntitySnapshotMessage => message.type === "entity_snapshot" && message.entity.typeId === "minecraft:cow")
      .sort((left, right) => left.entity.position.x - right.entity.position.x);

    expect(initialCows.map((message) => message.entity.position.x)).toEqual([8.5, 9.0]);

    nowMs = 1;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });
    const cowUpdates = updates
      .filter((message): message is EntityUpdateMessage => message.type === "entity_update")
      .filter((message) => initialCows.some((snapshot) => snapshot.entity.id === message.update.id))
      .sort((left, right) => left.update.id - right.update.id);

    expect(cowUpdates).toHaveLength(2);
    expect(cowUpdates[0]!.update.position?.x).toBeLessThan(8.5);
    expect(cowUpdates[1]!.update.position?.x).toBeGreaterThan(9.0);
    expect(cowUpdates.every((message) => message.update.position?.y === 64)).toBe(true);
  });

  test("pushes an overlapping generated mob and local player apart", async () => {
    const blocks = registerGeneratedRenderBlocks();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: 12345n,
      generator: new PlayerOverlappingCowFlatGenerator(12345n),
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

    (host as unknown as { playerState: ClientPlayerState }).playerState = createPlayerStateAt("local-player", 8.5, 64, 8.5);

    nowMs = 1;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });
    const playerUpdate = updates.find((message) => message.type === "player_state");
    const cowUpdate = updates.find(
      (message): message is EntityUpdateMessage => message.type === "entity_update" && message.update.id === initialCow!.entity.id,
    );

    expect(playerUpdate).toBeDefined();
    expect(playerUpdate).toMatchObject({
      type: "player_state",
      state: {
        position: {
          y: 64,
        },
      },
    });
    expect(playerUpdate!.state.position.x).toBeLessThan(8.5);
    expect(cowUpdate).toBeDefined();
    expect(cowUpdate!.update.position?.x).toBeGreaterThan(8.9);
  });

  test("publishes small-island starter farm animals near the origin chunk", async () => {
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

    const farmAnimals = messages
      .filter((message): message is EntitySnapshotMessage => message.type === "entity_snapshot"
        && (
          message.entity.typeId === "minecraft:chicken"
          || message.entity.typeId === "minecraft:cow"
          || message.entity.typeId === "minecraft:mooshroom"
          || message.entity.typeId === "minecraft:pig"
          || message.entity.typeId === "minecraft:rabbit"
          || message.entity.typeId === "minecraft:sheep"
          || message.entity.typeId === "minecraft:wolf"
        ))
      .sort((left, right) => left.entity.id - right.entity.id);

    expect(farmAnimals).toHaveLength(24);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:chicken")).toHaveLength(4);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:cow")).toHaveLength(4);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:mooshroom")).toHaveLength(2);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:pig")).toHaveLength(4);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:rabbit")).toHaveLength(4);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:sheep")).toHaveLength(4);
    expect(farmAnimals.filter((message) => message.entity.typeId === "minecraft:wolf")).toHaveLength(2);
    expect(farmAnimals
      .filter((message) => message.entity.typeId === "minecraft:chicken")
      .every((message) => typeof message.entity.data?.EggLayTime === "number")).toBe(true);
    expect(farmAnimals
      .filter((message) => message.entity.typeId === "minecraft:sheep")
      .every((message) => typeof message.entity.data?.Color === "number")).toBe(true);
    expect(farmAnimals
      .filter((message) => message.entity.typeId === "minecraft:mooshroom")
      .every((message) => message.entity.data?.Type === "red")).toBe(true);
    expect(farmAnimals
      .filter((message) => message.entity.typeId === "minecraft:rabbit")
      .every((message) => message.entity.data?.RabbitType === 0)).toBe(true);
    expect(farmAnimals
      .filter((message) => message.entity.typeId === "minecraft:wolf")
      .every((message) => message.entity.data?.Tame === false)).toBe(true);
    expect(farmAnimals.map((message) => [message.entity.position.x, message.entity.position.z])).toEqual([
      [6.5, 6.5],
      [10.5, 7.5],
      [7.5, 11.5],
      [12.5, 12.5],
      [1.5, 1.5],
      [14.5, 14.5],
      [3.5, 10.5],
      [4.5, 13.5],
      [13.5, 4.5],
      [14.5, 9.5],
      [1.5, 4.5],
      [4.5, 1.5],
      [12.5, 1.5],
      [1.5, 14.5],
      [5.5, 3.5],
      [9.5, 3.5],
      [3.5, 5.5],
      [11.5, 14.5],
      [14.5, 1.5],
      [1.5, 7.5],
      [2.5, 8.5],
      [8.5, 2.5],
      [15.5, 6.5],
      [6.5, 15.5],
    ]);
    expect(farmAnimals.every((message) => message.entity.chunkX === 0 && message.entity.chunkZ === 0)).toBe(true);
  });
});
