import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../../src/core/block-pos";
import { floor } from "../../../../src/util/mth";
import { MobAttribute } from "../../../../src/world/entity/attribute";
import type {
  MobAiLevel,
  MobLookControl,
  MobMoveControl,
  MobNavigation,
  MobRandom,
  PathfinderMob,
} from "../../../../src/world/entity/ai/pathfinder-mob";
import { AirBlock } from "../../../../src/world/level/block/air-block";
import { Block } from "../../../../src/world/level/block/block";
import { BushBlock } from "../../../../src/world/level/block/bush-block";
import { CactusBlock } from "../../../../src/world/level/block/cactus-block";
import { LeavesBlock } from "../../../../src/world/level/block/leaves-block";
import { LiquidBlock } from "../../../../src/world/level/block/liquid-block";
import { SlabBlock } from "../../../../src/world/level/block/slab-block";
import { SweetBerryBushBlock } from "../../../../src/world/level/block/sweet-berry-bush-block";
import { BlockBehaviour } from "../../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import type { BlockGetter } from "../../../../src/world/level/block-getter";
import { Fluids } from "../../../../src/world/level/material/fluids";
import type { Fluid } from "../../../../src/world/level/material/fluid";
import type { FluidState } from "../../../../src/world/level/material/fluid-state";
import { Material } from "../../../../src/world/level/material/material";
import {
  BlockPathTypes,
  PathFinder,
  WalkNodeEvaluator,
  blockGetterPathNavigationRegion,
  type Path,
} from "../../../../src/world/level/pathfinder";
import { AABB } from "../../../../src/world/phys/aabb";

const AIR = new AirBlock(BlockBehaviour.Properties.of(Material.AIR).noCollission().noOcclusion().air()).defaultBlockState();
const STONE = new Block(BlockBehaviour.Properties.of(Material.STONE)).defaultBlockState();
const LEAVES = new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES).noOcclusion()).defaultBlockState();
const PLANT = new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().noOcclusion()).defaultBlockState();
const CACTUS = new CactusBlock(BlockBehaviour.Properties.of(Material.CACTUS).noOcclusion()).defaultBlockState();
const SWEET_BERRY = new SweetBerryBushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().noOcclusion()).defaultBlockState();
const WATER = new LiquidBlock(Fluids.WATER, BlockBehaviour.Properties.of(Material.WATER).noCollission().noOcclusion()).defaultBlockState();
const SLAB = new SlabBlock(BlockBehaviour.Properties.of(Material.STONE).noOcclusion()).defaultBlockState();

const NULL_NAVIGATION: MobNavigation = {
  isDone: () => true,
  isStableDestination: () => true,
  moveTo: () => false,
  stop: () => {},
  tick: () => {},
};

const NULL_MOVE_CONTROL: MobMoveControl = {
  setWantedPosition: () => {},
};

const NULL_LOOK_CONTROL: MobLookControl = {
  setLookAt: () => {},
  tick: () => {},
};

const NULL_RANDOM: MobRandom = {
  nextInt: () => 0,
  nextFloat: () => 0,
  nextDouble: () => 0,
};

class MapBlockGetter implements BlockGetter {
  public constructor(
    private readonly blocks: ReadonlyMap<string, BlockState>,
    private readonly defaultState = AIR,
  ) {}

  public getBlockState(pos: BlockPos): BlockState {
    return this.blocks.get(key(pos.getX(), pos.getY(), pos.getZ())) ?? this.defaultState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }
}

class TestMob implements PathfinderMob {
  public readonly position: { readonly x: number; readonly y: number; readonly z: number };
  private readonly malus = new Map<BlockPathTypes, number>();

  public constructor(
    x: number,
    y: number,
    z: number,
    private readonly width = 0.9,
    private readonly height = 1.4,
  ) {
    this.position = { x, y, z };
    this.setPathfindingMalus(BlockPathTypes.DANGER_FIRE, 16.0);
    this.setPathfindingMalus(BlockPathTypes.DAMAGE_FIRE, -1.0);
  }

  public blockPosition(): BlockPos {
    return new BlockPos(floor(this.position.x), floor(this.position.y), floor(this.position.z));
  }

  public getBoundingBox(): AABB {
    const halfWidth = this.width / 2.0;
    return new AABB(
      this.position.x - halfWidth,
      this.position.y,
      this.position.z - halfWidth,
      this.position.x + halfWidth,
      this.position.y + this.height,
      this.position.z + halfWidth,
    );
  }

  public getX(): number {
    return this.position.x;
  }

  public getY(): number {
    return this.position.y;
  }

  public getZ(): number {
    return this.position.z;
  }

  public getEyeY(): number {
    return this.position.y + (this.height * 0.85);
  }

  public getBlockY(): number {
    return floor(this.position.y);
  }

  public getYRot(): number {
    return 0;
  }

  public setYRot(_yaw: number): void {}

  public getXRot(): number {
    return 0;
  }

  public setXRot(_pitch: number): void {}

  public getYHeadRot(): number {
    return 0;
  }

  public setYHeadRot(_yaw: number): void {}

  public getYBodyRot(): number {
    return 0;
  }

  public setYBodyRot(_yaw: number): void {}

  public getMaxHeadXRot(): number {
    return 40;
  }

  public getMaxHeadYRot(): number {
    return 75;
  }

  public getHeadRotSpeed(): number {
    return 10;
  }

  public getBbWidth(): number {
    return this.width;
  }

  public getBbHeight(): number {
    return this.height;
  }

  public getMaxUpStep(): number {
    return 1.0;
  }

  public getMaxFallDistance(): number {
    return 3;
  }

  public getRandom(): MobRandom {
    return NULL_RANDOM;
  }

  public getNoActionTime(): number {
    return 0;
  }

  public getNavigation(): MobNavigation {
    return NULL_NAVIGATION;
  }

  public getMoveControl(): MobMoveControl {
    return NULL_MOVE_CONTROL;
  }

  public getLookControl(): MobLookControl {
    return NULL_LOOK_CONTROL;
  }

  public getAiLevel(): MobAiLevel | undefined {
    return undefined;
  }

  public getPathfindingMalus(type: BlockPathTypes): number {
    return this.malus.get(type) ?? type.getMalus();
  }

  public setPathfindingMalus(type: BlockPathTypes, priority: number): void {
    this.malus.set(type, priority);
  }

  public canCutCorner(type: BlockPathTypes): boolean {
    return type !== BlockPathTypes.DANGER_FIRE
      && type !== BlockPathTypes.DANGER_CACTUS
      && type !== BlockPathTypes.DANGER_OTHER
      && type !== BlockPathTypes.WALKABLE_DOOR;
  }

  public getAttributeValue(attribute: MobAttribute): number {
    return attribute === MobAttribute.MOVEMENT_SPEED ? 0.2 : 0.0;
  }

  public setSpeed(_speed: number): void {}

  public setZza(_forward: number): void {}

  public setXxa(_strafe: number): void {}

  public canStandOnFluid(_fluid: Fluid): boolean {
    return false;
  }

  public isVehicle(): boolean {
    return false;
  }

  public isAlive(): boolean {
    return true;
  }

  public isBaby(): boolean {
    return false;
  }

  public ate(): void {}

  public isOnGround(): boolean {
    return true;
  }

  public isInWaterOrBubble(): boolean {
    return false;
  }

  public hasRestriction(): boolean {
    return false;
  }

  public getRestrictCenter(): BlockPos {
    return BlockPos.ZERO;
  }

  public getRestrictRadius(): number {
    return -1.0;
  }

  public isWithinRestriction(_pos: BlockPos): boolean {
    return true;
  }

  public getWalkTargetValue(_pos: BlockPos): number {
    return 0.0;
  }

  public hasPathfindingMalus(_pos: BlockPos): boolean {
    return false;
  }
}

function key(x: number, y: number, z: number): string {
  return `${x},${y},${z}`;
}

function levelWith(entries: readonly (readonly [number, number, number, BlockState])[]): MapBlockGetter {
  return new MapBlockGetter(new Map(entries.map(([x, y, z, state]) => [key(x, y, z), state])));
}

function flatLevel(xMin: number, xMax: number, zMin: number, zMax: number, extras: readonly (readonly [number, number, number, BlockState])[] = []): MapBlockGetter {
  const entries: [number, number, number, BlockState][] = [];
  for (let x = xMin; x <= xMax; x++) {
    for (let z = zMin; z <= zMax; z++) {
      entries.push([x, 0, z, STONE]);
    }
  }
  entries.push(...extras.map(([x, y, z, state]) => [x, y, z, state] as [number, number, number, BlockState]));
  return levelWith(entries);
}

function findPath(
  level: BlockGetter,
  mob: TestMob,
  target: BlockPos,
  maxRange = 32.0,
): Path | undefined {
  const region = blockGetterPathNavigationRegion(level, 0, 16);
  const evaluator = new WalkNodeEvaluator();
  const finder = new PathFinder(evaluator, 256);
  return finder.findPath(region, mob, new Set([target]), maxRange, 0, 1.0);
}

function nodeTriples(path: Path): readonly string[] {
  const triples: string[] = [];
  for (let i = 0; i < path.getNodeCount(); i++) {
    const node = path.getNode(i);
    triples.push(`${node.x},${node.y},${node.z}`);
  }
  return triples;
}

describe("WalkNodeEvaluator and PathFinder", () => {
  test("finds a vanilla weighted-A-star path across flat walkable ground", () => {
    const level = flatLevel(0, 4, 0, 0);
    const path = findPath(level, new TestMob(0.5, 1.0, 0.5), new BlockPos(4, 1, 0));

    expect(path?.canReach()).toBe(true);
    expect(nodeTriples(path!)).toEqual(["0,1,0", "1,1,0", "2,1,0", "3,1,0", "4,1,0"]);
  });

  test("accepts a one-block step using mob step height and floor-level checks", () => {
    const level = flatLevel(0, 3, 0, 0, [
      [1, 1, 0, STONE],
      [2, 1, 0, STONE],
    ]);
    const path = findPath(level, new TestMob(0.5, 1.0, 0.5), new BlockPos(2, 2, 0));

    expect(path?.canReach()).toBe(true);
    expect(nodeTriples(path!)).toEqual(["0,1,0", "1,2,0", "2,2,0"]);
  });

  test("returns the nearest partial path when a target is blocked", () => {
    const level = flatLevel(0, 2, 0, 0, [
      [1, 1, 0, STONE],
      [1, 2, 0, STONE],
    ]);
    const path = findPath(level, new TestMob(0.5, 1.0, 0.5), new BlockPos(2, 1, 0), 8.0);

    expect(path).toBeDefined();
    expect(path!.canReach()).toBe(false);
    expect(path!.getNode(0).toString()).toBe("Node{x=0, y=1, z=0}");
    expect(path!.getEndNode()?.x).not.toBe(2);
  });

  test("routes around water when the water malus makes dry ground cheaper", () => {
    const level = flatLevel(0, 5, -2, 2, [
      [1, 1, 0, WATER],
      [2, 1, 0, WATER],
      [3, 1, 0, WATER],
    ]);
    const path = findPath(level, new TestMob(0.5, 1.0, 0.5), new BlockPos(5, 1, 0));

    expect(path?.canReach()).toBe(true);
    expect(nodeTriples(path!)).not.toContain("1,1,0");
    expect(nodeTriples(path!)).not.toContain("2,1,0");
    expect(nodeTriples(path!)).not.toContain("3,1,0");
  });

  test("classifies common passive-mob block path types from vanilla rules", () => {
    const level = levelWith([
      [0, 0, 0, STONE],
      [1, 1, 0, LEAVES],
      [2, 1, 0, PLANT],
      [3, 1, 0, CACTUS],
      [4, 1, 0, SWEET_BERRY],
      [5, 1, 0, WATER],
      [8, 0, 0, SLAB],
    ]);
    const region = blockGetterPathNavigationRegion(level, 0, 16);
    const evaluator = new WalkNodeEvaluator();
    const mob = new TestMob(0.5, 1.0, 0.5);

    expect(evaluator.getBlockPathType(region, 0, 1, 0)).toBe(BlockPathTypes.WALKABLE);
    expect(evaluator.getBlockPathType(region, 1, 1, 0, mob, 1, 2, 1, false, false)).toBe(BlockPathTypes.BLOCKED);
    expect(evaluator.getBlockPathType(region, 2, 1, 0)).toBe(BlockPathTypes.OPEN);
    expect(evaluator.getBlockPathType(region, 3, 1, 0)).toBe(BlockPathTypes.DAMAGE_CACTUS);
    expect(evaluator.getBlockPathType(region, 4, 1, 0)).toBe(BlockPathTypes.DAMAGE_OTHER);
    expect(evaluator.getBlockPathType(region, 5, 1, 0)).toBe(BlockPathTypes.WATER);
    expect(evaluator.getBlockPathType(region, 8, 1, 0)).toBe(BlockPathTypes.WALKABLE);
  });
});
