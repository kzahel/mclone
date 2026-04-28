import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../../src/core/block-pos";
import {
  BinaryHeap,
  BlockPathTypes,
  Node,
  Path,
  Target,
  type PathEntity,
} from "../../../../src/world/level/pathfinder";

function expectBlockPos(pos: BlockPos, x: number, y: number, z: number): void {
  expect(pos.getX()).toBe(x);
  expect(pos.getY()).toBe(y);
  expect(pos.getZ()).toBe(z);
}

describe("path node foundation", () => {
  test("BlockPathTypes preserves vanilla order and malus values", () => {
    const values = BlockPathTypes.values();

    expect(values).toHaveLength(25);
    expect(values.map((value) => value.toString())).toEqual([
      "BLOCKED",
      "OPEN",
      "WALKABLE",
      "WALKABLE_DOOR",
      "TRAPDOOR",
      "POWDER_SNOW",
      "FENCE",
      "LAVA",
      "WATER",
      "WATER_BORDER",
      "RAIL",
      "UNPASSABLE_RAIL",
      "DANGER_FIRE",
      "DAMAGE_FIRE",
      "DANGER_CACTUS",
      "DAMAGE_CACTUS",
      "DANGER_OTHER",
      "DAMAGE_OTHER",
      "DOOR_OPEN",
      "DOOR_WOOD_CLOSED",
      "DOOR_IRON_CLOSED",
      "BREACH",
      "LEAVES",
      "STICKY_HONEY",
      "COCOA",
    ]);
    expect(BlockPathTypes.WATER.getMalus()).toBe(8.0);
    expect(BlockPathTypes.DAMAGE_FIRE.getMalus()).toBe(16.0);
    expect(BlockPathTypes.LEAVES.getMalus()).toBe(-1.0);
    expect(BlockPathTypes.COCOA.ordinal()).toBe(24);
  });

  test("Node hashes, distances, position helpers, and clone state match vanilla", () => {
    const origin = new Node(1, 2, 3);
    const other = new Node(4, 6, 3);

    expect(origin.hashCode()).toBe(Node.createHash(1, 2, 3));
    expect(new Node(-1, 255, -2).equals(new Node(-1, 255, -2))).toBe(true);
    expect(new Node(-1, 255, -2).equals(new Node(-1, 255, -1))).toBe(false);
    expect(origin.distanceTo(other)).toBe(5);
    expect(origin.distanceToSqr(other)).toBe(25);
    expect(origin.distanceManhattan(other)).toBe(7);
    expect(origin.distanceTo(new BlockPos(4, 6, 3))).toBe(5);
    expect(origin.toString()).toBe("Node{x=1, y=2, z=3}");
    expect(origin.asVec3()).toEqual({ x: 1, y: 2, z: 3 });
    expectBlockPos(origin.asBlockPos(), 1, 2, 3);

    const cameFrom = new Node(0, 0, 0);
    origin.heapIdx = 4;
    origin.g = 1.25;
    origin.h = 2.5;
    origin.f = 3.75;
    origin.cameFrom = cameFrom;
    origin.closed = true;
    origin.walkedDistance = 12.5;
    origin.costMalus = 8.0;
    origin.type = BlockPathTypes.WATER;

    const clone = origin.cloneAndMove(7, 8, 9);

    expect(clone).not.toBe(origin);
    expect(clone.x).toBe(7);
    expect(clone.y).toBe(8);
    expect(clone.z).toBe(9);
    expect(clone.heapIdx).toBe(4);
    expect(clone.g).toBe(1.25);
    expect(clone.h).toBe(2.5);
    expect(clone.f).toBe(3.75);
    expect(clone.cameFrom).toBe(cameFrom);
    expect(clone.closed).toBe(true);
    expect(clone.walkedDistance).toBe(12.5);
    expect(clone.costMalus).toBe(8.0);
    expect(clone.type).toBe(BlockPathTypes.WATER);
    expect(clone.inOpenSet()).toBe(true);
  });

  test("BinaryHeap orders by f cost and maintains vanilla heap indices", () => {
    const heap = new BinaryHeap();
    const high = new Node(0, 0, 0);
    const low = new Node(1, 0, 0);
    const middle = new Node(2, 0, 0);
    high.f = 5.0;
    low.f = 2.0;
    middle.f = 3.0;

    expect(heap.isEmpty()).toBe(true);
    expect(heap.insert(high)).toBe(high);
    heap.insert(low);
    heap.insert(middle);

    expect(heap.size()).toBe(3);
    expect(heap.peek()).toBe(low);
    expect(low.heapIdx).toBe(0);
    expect(() => heap.insert(low)).toThrow("OW KNOWS!");

    heap.changeCost(high, 1.0);
    expect(heap.peek()).toBe(high);
    expect(heap.pop()).toBe(high);
    expect(high.heapIdx).toBe(-1);

    heap.remove(middle);
    expect(middle.heapIdx).toBe(-1);
    expect(heap.getHeap()).toEqual([low]);
    expect(heap.pop()).toBe(low);
    expect(heap.isEmpty()).toBe(true);
  });

  test("Path stores nodes, target distance, advancement, and entity-position offsets", () => {
    const entity: PathEntity = { getBbWidth: () => 0.6 };
    const nodes = [
      new Node(0, 64, 0),
      new Node(1, 64, 0),
      new Node(2, 64, 0),
    ];
    const path = new Path(nodes, new BlockPos(4, 64, 0), true);

    expect(path.notStarted()).toBe(true);
    expect(path.isDone()).toBe(false);
    expect(path.getNodeCount()).toBe(3);
    expect(path.getEndNode()).toBe(nodes[2]);
    expect(path.getTarget()).toEqual(new BlockPos(4, 64, 0));
    expect(path.getDistToTarget()).toBe(2);
    expect(path.canReach()).toBe(true);
    expect(path.getNextNode()).toBe(nodes[0]);
    expect(path.getNextEntityPos(entity)).toEqual({ x: 0.5, y: 64, z: 0.5 });

    path.advance();

    expect(path.notStarted()).toBe(false);
    expect(path.getNextNodeIndex()).toBe(1);
    expect(path.getPreviousNode()).toBe(nodes[0]);
    expectBlockPos(path.getNextNodePos(), 1, 64, 0);
    expect(path.getEntityPosAtNode({ getBbWidth: () => 1.2 }, 1)).toEqual({ x: 2, y: 64, z: 1 });

    const replacement = new Node(9, 65, 9);
    path.replaceNode(1, replacement);
    expect(path.getNode(1)).toBe(replacement);
    path.truncateNodes(2);
    expect(path.getNodeCount()).toBe(2);
    expect(path.sameAs(new Path([new Node(0, 64, 0), new Node(9, 65, 9)], new BlockPos(9, 65, 9), true))).toBe(true);
    expect(path.sameAs(new Path([new Node(0, 64, 0)], new BlockPos(9, 65, 9), true))).toBe(false);
    expect(path.sameAs(undefined)).toBe(false);
    expect(path.toString()).toBe("Path(length=2)");

    const openSet = [nodes[0]!];
    const closedSet = [replacement];
    const target = new Target(9, 65, 9);
    const targetNodes = new Set([target]);
    path.setDebug(openSet, closedSet, targetNodes);
    expect(path.getOpenSet()).toBe(openSet);
    expect(path.getClosedSet()).toBe(closedSet);
    expect(path.getTargetNodes()).toBe(targetNodes);
  });

  test("Target tracks best candidate and reached state", () => {
    const start = new Node(3, 4, 5);
    const worse = new Node(6, 4, 5);
    const better = new Node(4, 4, 5);
    const target = new Target(start);

    expect(target.x).toBe(3);
    expect(target.y).toBe(4);
    expect(target.z).toBe(5);
    expect(target.getBestNode()).toBeUndefined();
    expect(target.isReached()).toBe(false);

    target.updateBest(10.0, worse);
    target.updateBest(12.0, new Node(99, 99, 99));
    target.updateBest(1.0, better);
    target.setReached();

    expect(target.getBestNode()).toBe(better);
    expect(target.isReached()).toBe(true);
  });
});
