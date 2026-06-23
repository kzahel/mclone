import { describe, expect, test } from "vitest";

import cowFixture from "../fixtures/creatures/overworld-seed-12345-chunk-2--18-entities.json";
import sheepFixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import {
  compareCreatureGenerationFixtures,
  type CreatureGenerationFixture,
} from "../../oracle/lib/integration/creature-fixture.ts";

describe("committed creature generation fixture", () => {
  test.each([
    ["sheep", sheepFixture as unknown as CreatureGenerationFixture, [{ chunkX: -7, chunkZ: -15 }], "minecraft:sheep"],
    ["cow", cowFixture as unknown as CreatureGenerationFixture, [{ chunkX: 2, chunkZ: -18 }], "minecraft:cow"],
  ] as const)("contains normalized %s passive generation entities", (_name, creatureFixture, chunks, expectedType) => {
    expect(creatureFixture.module).toBe("creature-generation");
    expect(creatureFixture.minecraftVersion).toBe("1.17.1");
    expect(creatureFixture.source.entityStorage).toBe("entities");
    expect(creatureFixture.chunks).toEqual(chunks);
    expect(creatureFixture.entities.length).toBeGreaterThan(0);
    expect(creatureFixture.entities.some((entity) => entity.type === expectedType)).toBe(true);
    expect(creatureFixture.entities.every((entity) => entity.category === "creature")).toBe(true);
    expect(compareCreatureGenerationFixtures(creatureFixture, creatureFixture)).toEqual([]);
  });
});
