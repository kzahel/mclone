import { describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { SectionPos } from "../../../../src/core/section-pos";
import { SyntheticRuntimeEntity } from "../../../../src/world/level/entity/entity-access";
import { EntitySectionStorage } from "../../../../src/world/level/entity/entity-section-storage";
import { AABB } from "../../../../src/world/phys/aabb";
import { Visibility } from "../../../../src/world/level/entity/visibility";

function entity(id: number, x: number, y: number, z: number): SyntheticRuntimeEntity {
  return new SyntheticRuntimeEntity({ id, uuid: `uuid-${id}`, x, y, z });
}

describe("EntitySectionStorage", () => {
  test("groups sections by chunk and scans only accessible sections for AABB queries", () => {
    const storage = new EntitySectionStorage<SyntheticRuntimeEntity>((chunkX, chunkZ) => {
      return chunkX === 0 && chunkZ === 0 ? Visibility.TRACKED : Visibility.HIDDEN;
    });
    const visibleEntity = entity(1, 1, 64, 1);
    const hiddenEntity = entity(2, 33, 64, 1);

    storage.getOrCreateSection(SectionPos.asLongFromBlockPos(visibleEntity.blockPosition())).add(visibleEntity);
    storage.getOrCreateSection(SectionPos.asLongFromBlockPos(hiddenEntity.blockPosition())).add(hiddenEntity);

    const found: number[] = [];
    storage.getEntities(new AABB(new BlockPos(-4, 60, -4), new BlockPos(40, 70, 8)), (current) => {
      found.push(current.id);
    });

    expect(found).toEqual([1]);
    expect(storage.getExistingSectionPositionsInChunk(0, 0)).toHaveLength(1);
    expect(storage.getExistingSectionPositionsInChunk(2, 0)).toHaveLength(1);
    expect(storage.getAllChunksWithExistingSections()).toEqual([
      { chunkX: 0, chunkZ: 0 },
      { chunkX: 2, chunkZ: 0 },
    ]);
  });
});
