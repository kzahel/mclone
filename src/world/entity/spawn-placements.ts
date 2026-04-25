import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { WorldGenLevel } from "../level/world-gen-level";
import type { BlockState } from "../level/block/state/block-state";
import type { FluidState } from "../level/material/fluid-state";
import { Fluids } from "../level/material/fluids";
import { EntityType, EntityTypes } from "./entity-type";
import {
  checkAnimalSpawnRules,
  checkMushroomSpawnRules,
  checkPolarBearSpawnRules,
  checkRabbitSpawnRules,
} from "./animal/animal";

export const SpawnPlacementType = {
  ON_GROUND: "on_ground",
  IN_WATER: "in_water",
  NO_RESTRICTIONS: "no_restrictions",
  IN_LAVA: "in_lava",
} as const;

export type SpawnPlacementType = typeof SpawnPlacementType[keyof typeof SpawnPlacementType];

type SpawnPredicate = (level: WorldGenLevel, pos: BlockPos, random: { nextInt(bound: number): number }) => boolean;

interface SpawnPlacementData {
  readonly heightmap: Heightmap.Types;
  readonly placement: SpawnPlacementType;
  readonly predicate: SpawnPredicate;
}

const DATA_BY_TYPE = new Map<EntityType, SpawnPlacementData>();

function register(
  type: EntityType,
  placement: SpawnPlacementType,
  heightmap: Heightmap.Types,
  predicate: SpawnPredicate,
): void {
  if (DATA_BY_TYPE.has(type)) {
    throw new Error(`Duplicate spawn placement registration for type ${type.id}`);
  }

  DATA_BY_TYPE.set(type, { heightmap, placement, predicate });
}

function animalPredicate(level: WorldGenLevel, pos: BlockPos, _random: { nextInt(bound: number): number }): boolean {
  return checkAnimalSpawnRules(level, pos);
}

function rabbitPredicate(level: WorldGenLevel, pos: BlockPos, _random: { nextInt(bound: number): number }): boolean {
  return checkRabbitSpawnRules(level, pos);
}

function mushroomPredicate(level: WorldGenLevel, pos: BlockPos, _random: { nextInt(bound: number): number }): boolean {
  return checkMushroomSpawnRules(level, pos);
}

function polarBearPredicate(level: WorldGenLevel, pos: BlockPos, _random: { nextInt(bound: number): number }): boolean {
  return checkPolarBearSpawnRules(level, pos);
}

function registerGroundAnimal(type: EntityType, predicate: SpawnPredicate = animalPredicate): void {
  register(type, SpawnPlacementType.ON_GROUND, Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, predicate);
}

registerGroundAnimal(EntityTypes.CHICKEN);
registerGroundAnimal(EntityTypes.COW);
registerGroundAnimal(EntityTypes.DONKEY);
registerGroundAnimal(EntityTypes.GOAT);
registerGroundAnimal(EntityTypes.HORSE);
registerGroundAnimal(EntityTypes.LLAMA);
registerGroundAnimal(EntityTypes.MOOSHROOM, mushroomPredicate);
registerGroundAnimal(EntityTypes.PIG);
registerGroundAnimal(EntityTypes.POLAR_BEAR, polarBearPredicate);
registerGroundAnimal(EntityTypes.RABBIT, rabbitPredicate);
registerGroundAnimal(EntityTypes.SHEEP);
registerGroundAnimal(EntityTypes.WOLF);
register(
  EntityTypes.FOX,
  SpawnPlacementType.NO_RESTRICTIONS,
  Heightmap.Types.MOTION_BLOCKING_NO_LEAVES,
  animalPredicate,
);

export function getPlacementType(type: EntityType): SpawnPlacementType {
  return DATA_BY_TYPE.get(type)?.placement ?? SpawnPlacementType.NO_RESTRICTIONS;
}

export function getHeightmapType(type: EntityType): Heightmap.Types {
  return DATA_BY_TYPE.get(type)?.heightmap ?? Heightmap.Types.MOTION_BLOCKING_NO_LEAVES;
}

export function checkSpawnRules(
  type: EntityType,
  level: WorldGenLevel,
  pos: BlockPos,
  random: { nextInt(bound: number): number },
): boolean {
  return DATA_BY_TYPE.get(type)?.predicate(level, pos, random) ?? true;
}

export function isSpawnPositionOk(
  placement: SpawnPlacementType,
  level: WorldGenLevel,
  pos: BlockPos,
  type: EntityType,
): boolean {
  if (placement === SpawnPlacementType.NO_RESTRICTIONS) {
    return true;
  }

  const state = level.getBlockState(pos);
  const fluid = level.getFluidState(pos);
  const above = pos.above();
  const below = pos.below();
  switch (placement) {
    case SpawnPlacementType.IN_WATER:
      return fluid.getType().isSame(Fluids.WATER) && level.getFluidState(below).getType().isSame(Fluids.WATER);
    case SpawnPlacementType.IN_LAVA:
      return fluid.getType().isSame(Fluids.LAVA);
    case SpawnPlacementType.ON_GROUND:
    default:
      return isValidSpawnBlock(level, below, level.getBlockState(below), type)
        && isValidEmptySpawnBlock(level, pos, state, fluid, type)
        && isValidEmptySpawnBlock(level, above, level.getBlockState(above), level.getFluidState(above), type);
  }
}

function isValidSpawnBlock(level: WorldGenLevel, pos: BlockPos, state: BlockState, _type: EntityType): boolean {
  return state.isFaceSturdy(level, pos, Direction.UP) && state.getLightEmission() < 14;
}

function isValidEmptySpawnBlock(
  level: WorldGenLevel,
  pos: BlockPos,
  state: BlockState,
  fluid: FluidState,
  _type: EntityType,
): boolean {
  // Runtime: signal-source, prevent-spawn tag, and danger-block filters wait for those block APIs.
  return !state.isCollisionShapeFullBlock(level, pos) && fluid.isEmpty();
}
