import { describe, expect, test } from "vitest";

import fixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import {
  compareCreatureGenerationFixtures,
  type CreatureGenerationFixture,
} from "../../src/oracle/integration/creature-fixture.ts";

describe("committed creature generation fixture", () => {
  test("contains normalized passive generation entities", () => {
    const creatureFixture = fixture as unknown as CreatureGenerationFixture;

    expect(creatureFixture.module).toBe("creature-generation");
    expect(creatureFixture.minecraftVersion).toBe("1.17.1");
    expect(creatureFixture.source.entityStorage).toBe("entities");
    expect(creatureFixture.chunks).toEqual([{ chunkX: -7, chunkZ: -15 }]);
    expect(creatureFixture.entities.length).toBeGreaterThan(0);
    expect(creatureFixture.entities.every((entity) => entity.category === "creature")).toBe(true);
    expect(compareCreatureGenerationFixtures(creatureFixture, creatureFixture)).toEqual([]);
  });
});
