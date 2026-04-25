import { describe, expect, test } from "vitest";
import { SyntheticRuntimeEntity } from "../../../../src/world/level/entity/entity-access";
import { EntityTickList } from "../../../../src/world/level/entity/entity-tick-list";

function entity(id: number): SyntheticRuntimeEntity {
  return new SyntheticRuntimeEntity({ id, uuid: `uuid-${id}` });
}

describe("EntityTickList", () => {
  test("uses vanilla copy-on-write semantics during iteration", () => {
    const first = entity(1);
    const second = entity(2);
    const third = entity(3);
    const tickList = new EntityTickList<SyntheticRuntimeEntity>();
    tickList.add(first);
    tickList.add(second);

    const seen: number[] = [];
    tickList.forEach((current) => {
      seen.push(current.id);
      if (current === first) {
        tickList.add(third);
        tickList.remove(second);
      }
    });

    expect(seen).toEqual([1, 2]);
    expect(tickList.contains(first)).toBe(true);
    expect(tickList.contains(second)).toBe(false);
    expect(tickList.contains(third)).toBe(true);

    const nextPass: number[] = [];
    tickList.forEach((current) => nextPass.push(current.id));
    expect(nextPass).toEqual([1, 3]);
  });

  test("rejects nested iteration", () => {
    const tickList = new EntityTickList<SyntheticRuntimeEntity>();
    tickList.add(entity(1));

    expect(() => {
      tickList.forEach(() => {
        tickList.forEach(() => {});
      });
    }).toThrow("Only one concurrent iteration supported");
  });
});
