import { describe, expect, test } from "vitest";

import {
  decodeEntityStorageChunk,
  decodeLegacyChunkEntities,
  type DecodedEntityChunk,
} from "../../oracle/lib/anvil/entity-chunk.ts";
import {
  NBT_TAG_COMPOUND,
  NBT_TAG_DOUBLE,
  NBT_TAG_END,
  NBT_TAG_FLOAT,
  type NbtList,
  type NbtCompound,
  type NbtTagId,
} from "../../oracle/lib/anvil/nbt.ts";
import {
  buildCreatureGenerationFixture,
  categoryForEntityType,
  compareCreatureGenerationFixtures,
  normalizeCreatureEntity,
} from "../../oracle/lib/integration/creature-fixture.ts";

function list(type: NbtTagId, values: readonly number[]): NbtList {
  return { type, values };
}

function sheep(overrides: Partial<NbtCompound> = {}): NbtCompound {
  return {
    id: "minecraft:sheep",
    Pos: list(NBT_TAG_DOUBLE, [6.5, 64, 11.5]),
    Motion: list(NBT_TAG_DOUBLE, [0, 0, 0]),
    Rotation: list(NBT_TAG_FLOAT, [132.25, 0]),
    OnGround: 1,
    Age: 0,
    ForcedAge: 0,
    UUID: new Int32Array([1, 2, 3, 4]),
    Health: 8,
    Color: 12,
    Sheared: 0,
    ...overrides,
  };
}

function decodedChunk(chunkX: number, chunkZ: number, entities: readonly NbtCompound[]): DecodedEntityChunk {
  return {
    dataVersion: 2730,
    chunkX,
    chunkZ,
    source: "entities",
    entities,
  };
}

describe("entity chunk decoding", () => {
  test("decodes vanilla entity-storage chunks with top-level Position and Entities", () => {
    const decoded = decodeEntityStorageChunk({
      DataVersion: 2730,
      Position: new Int32Array([0, -2]),
      Entities: { type: NBT_TAG_COMPOUND, values: [sheep()] },
    });

    expect(decoded.dataVersion).toBe(2730);
    expect(decoded.chunkX).toBe(0);
    expect(decoded.chunkZ).toBe(-2);
    expect(decoded.source).toBe("entities");
    expect(decoded.entities).toHaveLength(1);
  });

  test("accepts absent or empty legacy chunk entity lists", () => {
    expect(
      decodeLegacyChunkEntities({
        DataVersion: 2730,
        Level: { xPos: -1, zPos: 2, Entities: { type: NBT_TAG_END, values: [] } },
      }),
    ).toEqual({
      dataVersion: 2730,
      chunkX: -1,
      chunkZ: 2,
      source: "legacy-chunk",
      entities: [],
    });
  });
});

describe("creature fixture normalization", () => {
  test("keeps stable generated-entity facts and omits unstable full NBT", () => {
    const normalized = normalizeCreatureEntity(sheep(), 0, 0);

    expect(normalized).toEqual({
      chunkX: 0,
      chunkZ: 0,
      type: "minecraft:sheep",
      category: "creature",
      pos: [6.5, 64, 11.5],
      rotation: [132.25, 0],
      onGround: true,
      age: 0,
      data: { Color: 12 },
    });
  });

  test("builds stable creature-generation fixtures sorted by chunk and entity facts", () => {
    const fixture = buildCreatureGenerationFixture(
      {
        minecraftVersion: "1.17.1",
        seed: "12345",
        generator: "default",
        generateStructures: false,
      },
      [
        decodedChunk(1, 0, [sheep({ Pos: list(NBT_TAG_DOUBLE, [16.5, 65, 1.5]) })]),
        decodedChunk(0, 0, [sheep()]),
      ],
    );

    expect(fixture.module).toBe("creature-generation");
    expect(fixture.source.entityStorage).toBe("entities");
    expect(fixture.chunks).toEqual([
      { chunkX: 0, chunkZ: 0 },
      { chunkX: 1, chunkZ: 0 },
    ]);
    expect(fixture.entities.map((entity) => entity.pos)).toEqual([
      [6.5, 64, 11.5],
      [16.5, 65, 1.5],
    ]);
  });

  test("maps common overworld adjacent entity categories", () => {
    expect(categoryForEntityType("minecraft:cow")).toBe("creature");
    expect(categoryForEntityType("minecraft:spider")).toBe("monster");
    expect(categoryForEntityType("minecraft:unknown")).toBe("misc");
  });
});

describe("creature fixture comparison", () => {
  test("reports readable first entity diffs", () => {
    const expected = buildCreatureGenerationFixture(
      { minecraftVersion: "1.17.1", seed: "12345", generator: "default", generateStructures: false },
      [decodedChunk(0, 0, [sheep()])],
    );
    const actual = {
      ...expected,
      entities: [
        {
          ...expected.entities[0]!,
          pos: [7.5, 64, 11.5] as const,
        },
      ],
    };

    expect(compareCreatureGenerationFixtures(expected, actual)).toEqual([
      {
        kind: "entity_mismatch",
        path: "entities[0]",
        expected: expected.entities[0],
        actual: actual.entities[0],
      },
    ]);
  });
});
