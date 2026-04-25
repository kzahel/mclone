import { describe, expect, test } from "vitest";
import { EntityRuntime } from "../../src/runtime/host/entity-runtime";
import { EntityRemovalReason, SyntheticRuntimeEntity } from "../../src/world/level/entity/entity-access";
import { FullChunkStatus } from "../../src/world/level/entity/full-chunk-status";

describe("EntityRuntime", () => {
  test("owns manager callbacks and ticks only entity-ticking entities", () => {
    const ticked: number[] = [];
    const runtime = new EntityRuntime<SyntheticRuntimeEntity>({
      tickEntity: (entity) => ticked.push(entity.id),
    });
    const sheep = new SyntheticRuntimeEntity({ id: 1, uuid: "uuid-1", x: 1, y: 64, z: 1 });

    expect(runtime.addEntity(sheep)).toBe(true);
    runtime.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    runtime.tick();
    expect(ticked).toEqual([]);

    runtime.updateChunkStatus(0, 0, FullChunkStatus.ENTITY_TICKING);
    runtime.tick();
    expect(ticked).toEqual([1]);

    sheep.setRemoved(EntityRemovalReason.DISCARDED);
    runtime.tick();
    expect(ticked).toEqual([1]);
  });
});
