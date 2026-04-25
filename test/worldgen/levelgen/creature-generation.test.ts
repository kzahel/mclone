import { afterEach, describe, expect, test } from "vitest";

import fixture from "../../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import { Registry } from "../../../src/core/registry.ts";
import { SectionPos } from "../../../src/core/section-pos.ts";
import { EntityRuntime } from "../../../src/runtime/host/entity-runtime.ts";
import {
  compareCreatureGenerationFixtures,
  sortCreatureEntities,
  type CreatureEntityCategory,
  type CreatureGenerationFixture,
  type NormalizedCreatureEntity,
} from "../../../src/oracle/integration/creature-fixture.ts";
import { FullChunkStatus } from "../../../src/world/level/entity/full-chunk-status.ts";
import { GeneratedRenderLevel } from "../../../src/world/level/generated-render-level.ts";
import { registerGeneratedRenderBlocks } from "../../../src/world/level/generated-render-blocks.ts";
import type { GeneratedMobEntity } from "../../../src/world/entity/entity-type.ts";
import { OverworldBiomeSource } from "../../../src/worldgen/biome/overworld-biome-source.ts";
import { NoiseBasedChunkGenerator } from "../../../src/worldgen/levelgen/noise-based-chunk-generator.ts";
import { WorldgenRandom } from "../../../src/worldgen/prng/worldgen-random.ts";

const creatureFixture = fixture as unknown as CreatureGenerationFixture;
const SEED = BigInt(creatureFixture.seed);

afterEach(() => {
  Registry.BLOCK.clear();
});

function createGeneratedLevel(): GeneratedRenderLevel {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(SEED);
  const generator = new NoiseBasedChunkGenerator(biomeSource, SEED);
  return new GeneratedRenderLevel(blocks.airState, generator, biomeSource, SEED, blocks.blockStateById);
}

function normalizeGeneratedEntity(entity: GeneratedMobEntity): NormalizedCreatureEntity {
  const data = Object.keys(entity.data).length === 0 ? undefined : entity.data;
  return {
    chunkX: SectionPos.blockToSectionCoord(entity.blockPosition().getX()),
    chunkZ: SectionPos.blockToSectionCoord(entity.blockPosition().getZ()),
    type: entity.typeId,
    category: entity.entityType.category as CreatureEntityCategory,
    pos: [entity.position.x, entity.position.y, entity.position.z],
    rotation: [entity.rotation.yaw, entity.rotation.pitch],
    onGround: entity.onGround,
    age: entity.age,
    ...(data === undefined ? {} : { data }),
  };
}

describe("passive creature generation", () => {
  test("ports NaturalSpawner chunk-generation sheep into the host entity runtime", () => {
    const chunk = creatureFixture.chunks[0]!;
    const level = createGeneratedLevel();
    level.updateChunkView(chunk.chunkX, chunk.chunkZ, 1);
    expect(level.getChunk(chunk.chunkX, chunk.chunkZ, true)).not.toBeNull();

    const runtime = new EntityRuntime<GeneratedMobEntity>();
    runtime.updateChunkStatus(chunk.chunkX, chunk.chunkZ, FullChunkStatus.BORDER);
    const spawned: GeneratedMobEntity[] = [];
    const generator = new NoiseBasedChunkGenerator(new OverworldBiomeSource(SEED), SEED);

    generator.spawnOriginalMobs(
      level,
      chunk.chunkX,
      chunk.chunkZ,
      {
        addFreshEntityWithPassengers(entity) {
          spawned.push(entity);
          runtime.addWorldGenChunkEntities([entity]);
        },
      },
      {
        levelRandom: new WorldgenRandom(0),
        nextEntityId: (() => {
          let nextId = 1;
          return () => nextId++;
        })(),
        nextEntityUuid: (id) => `creature-fixture-${id}`,
      },
    );

    const actual: CreatureGenerationFixture = {
      ...creatureFixture,
      entities: sortCreatureEntities(spawned.map(normalizeGeneratedEntity)),
    };

    expect(compareCreatureGenerationFixtures(creatureFixture, actual)).toEqual([]);
    expect(runtime.manager.getEntityGetter().getAll()).toHaveLength(creatureFixture.entities.length);
  });
});
