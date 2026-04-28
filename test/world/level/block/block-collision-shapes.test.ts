import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../../src/core/block-pos";
import { blockGetterCollisionWorld } from "../../../../src/runtime/movement";
import { AirBlock } from "../../../../src/world/level/block/air-block";
import { BushBlock } from "../../../../src/world/level/block/bush-block";
import { LeavesBlock } from "../../../../src/world/level/block/leaves-block";
import { SlabBlock } from "../../../../src/world/level/block/slab-block";
import { BlockBehaviour } from "../../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { SlabType } from "../../../../src/world/level/block/state/properties/slab-type";
import type { BlockGetter } from "../../../../src/world/level/block-getter";
import { Material } from "../../../../src/world/level/material/material";
import type { FluidState } from "../../../../src/world/level/material/fluid-state";
import { AABB } from "../../../../src/world/phys/aabb";

const ORIGIN = new BlockPos(0, 0, 0);

class MapBlockGetter implements BlockGetter {
  public constructor(
    private readonly blocks: ReadonlyMap<string, BlockState>,
    private readonly defaultState: BlockState,
  ) {}

  public getBlockState(pos: BlockPos): BlockState {
    return this.blocks.get(key(pos)) ?? this.defaultState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }
}

function key(pos: BlockPos): string {
  return `${pos.getX()},${pos.getY()},${pos.getZ()}`;
}

function airState(): BlockState {
  return new AirBlock(BlockBehaviour.Properties.of(Material.AIR).noCollission().noOcclusion().air()).defaultBlockState();
}

function levelWith(state: BlockState): BlockGetter {
  return new MapBlockGetter(new Map([[key(ORIGIN), state]]), airState());
}

function expectBox(actual: AABB, expected: AABB): void {
  expect(actual.minX).toBeCloseTo(expected.minX, 8);
  expect(actual.minY).toBeCloseTo(expected.minY, 8);
  expect(actual.minZ).toBeCloseTo(expected.minZ, 8);
  expect(actual.maxX).toBeCloseTo(expected.maxX, 8);
  expect(actual.maxY).toBeCloseTo(expected.maxY, 8);
  expect(actual.maxZ).toBeCloseTo(expected.maxZ, 8);
}

describe("block collision shapes", () => {
  test("leaves are non-occluding for rendering but retain full block collision", () => {
    const leaves = new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES).noOcclusion()).defaultBlockState();
    const level = levelWith(leaves);

    expect(leaves.canOcclude()).toBe(false);
    expect(leaves.getBlock().hasCollision).toBe(true);
    expect(leaves.getCollisionShape(level, ORIGIN).isEmpty()).toBe(false);
    expect(leaves.isCollisionShapeFullBlock(level, ORIGIN)).toBe(true);

    const query = blockGetterCollisionWorld(level).queryBlockCollisions(new AABB(0.25, 0.25, 0.25, 0.75, 0.75, 0.75));

    expect(query.type).toBe("loaded");
    if (query.type === "loaded") {
      expect(query.boxes).toHaveLength(1);
      expectBox(query.boxes[0]!, new AABB(0, 0, 0, 1, 1, 1));
    }
  });

  test("no-collision plants remain non-colliding even though they are also non-occluding", () => {
    const plant = new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().noOcclusion()).defaultBlockState();
    const level = levelWith(plant);

    expect(plant.canOcclude()).toBe(false);
    expect(plant.getBlock().hasCollision).toBe(false);
    expect(plant.getCollisionShape(level, ORIGIN).isEmpty()).toBe(true);
    expect(plant.isCollisionShapeFullBlock(level, ORIGIN)).toBe(false);

    const query = blockGetterCollisionWorld(level).queryBlockCollisions(new AABB(0.25, 0.25, 0.25, 0.75, 0.75, 0.75));

    expect(query.type).toBe("loaded");
    if (query.type === "loaded") {
      expect(query.boxes).toHaveLength(0);
    }
  });

  test("slabs are non-occluding partial collision shapes and double slabs are full collision blocks", () => {
    const slabBlock = new SlabBlock(BlockBehaviour.Properties.of(Material.STONE).noOcclusion());
    const bottom = slabBlock.defaultBlockState();
    const top = bottom.setValue(SlabBlock.TYPE, SlabType.TOP);
    const double = bottom.setValue(SlabBlock.TYPE, SlabType.DOUBLE);
    const level = levelWith(bottom);

    expect(bottom.canOcclude()).toBe(false);
    expect(bottom.getCollisionShape(level, ORIGIN).isEmpty()).toBe(false);
    expect(bottom.isCollisionShapeFullBlock(level, ORIGIN)).toBe(false);
    expect(top.getCollisionShape(level, ORIGIN).toAabbs()[0]!.minY).toBe(0.5);
    expect(double.isCollisionShapeFullBlock(level, ORIGIN)).toBe(true);

    const query = blockGetterCollisionWorld(level).queryBlockCollisions(new AABB(0.25, 0.25, 0.25, 0.75, 0.75, 0.75));

    expect(query.type).toBe("loaded");
    if (query.type === "loaded") {
      expect(query.boxes).toHaveLength(1);
      expectBox(query.boxes[0]!, new AABB(0, 0, 0, 1, 0.5, 1));
    }
  });
});
