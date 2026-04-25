import { BlockPos } from "../../core/block-pos";
import { clamp, floor } from "../../util/mth";
import type { Biome } from "../biome/biome";
import { CHUNK_WIDTH } from "../chunk/chunk-block-buffer";
import type { WorldgenRandom } from "../prng/worldgen-random";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { AABB } from "../../world/phys/aabb";
import { EntityType, type GeneratedMobEntity } from "../../world/entity/entity-type";
import { MobCategory } from "../../world/entity/mob-category";
import {
  checkSpawnRules,
  getHeightmapType,
  getPlacementType,
  isSpawnPositionOk,
  SpawnPlacementType,
} from "../../world/entity/spawn-placements";

export interface GenerationEntitySink {
  addFreshEntityWithPassengers(entity: GeneratedMobEntity): void;
}

export interface NaturalSpawnerOptions {
  readonly levelRandom?: { nextInt(bound: number): number };
  readonly nextEntityId?: () => number;
  readonly nextEntityUuid?: (id: number) => string;
}

let nextGeneratedEntityId = 1;

function allocateEntityId(): number {
  return nextGeneratedEntityId++;
}

function defaultEntityUuid(id: number): string {
  return `mclone:worldgen/${id}`;
}

export class NaturalSpawner {
  public static spawnMobsForChunkGeneration(
    level: WorldGenLevel,
    biome: Biome,
    chunkX: number,
    chunkZ: number,
    random: WorldgenRandom,
    sink: GenerationEntitySink,
    options: NaturalSpawnerOptions = {},
  ): void {
    const mobSettings = biome.getMobSettings();
    const mobs = mobSettings.getMobs(MobCategory.CREATURE);
    if (mobs.isEmpty()) {
      return;
    }

    const minBlockX = chunkX * CHUNK_WIDTH;
    const minBlockZ = chunkZ * CHUNK_WIDTH;
    const levelRandom = options.levelRandom ?? random;
    const nextEntityId = options.nextEntityId ?? allocateEntityId;
    const nextEntityUuid = options.nextEntityUuid ?? defaultEntityUuid;

    while (random.nextFloat() < mobSettings.getCreatureProbability()) {
      const spawnerData = mobs.getRandom(random);
      if (spawnerData === undefined) {
        continue;
      }

      const count = spawnerData.minCount + random.nextInt(1 + spawnerData.maxCount - spawnerData.minCount);
      let x = minBlockX + random.nextInt(CHUNK_WIDTH);
      let z = minBlockZ + random.nextInt(CHUNK_WIDTH);
      const originX = x;
      const originZ = z;

      for (let spawnedCount = 0; spawnedCount < count; spawnedCount++) {
        let spawned = false;

        for (let attempt = 0; !spawned && attempt < 4; attempt++) {
          const pos = getTopNonCollidingPos(level, spawnerData.type, x, z);
          if (spawnerData.type.canSummon() && isSpawnPositionOk(getPlacementType(spawnerData.type), level, pos, spawnerData.type)) {
            const width = spawnerData.type.width;
            const spawnX = clamp(x, minBlockX + width, minBlockX + CHUNK_WIDTH - width);
            const spawnZ = clamp(z, minBlockZ + width, minBlockZ + CHUNK_WIDTH - width);
            const spawnPos = blockPosFromDouble(spawnX, pos.getY(), spawnZ);
            if (!noCollision(level, spawnerData.type.getAABB(spawnX, pos.getY(), spawnZ))) {
              continue;
            }
            if (!checkSpawnRules(spawnerData.type, level, spawnPos, levelRandom)) {
              continue;
            }

            const entityId = nextEntityId();
            const entity = spawnerData.type.createGeneratedMob(
              entityId,
              nextEntityUuid(entityId),
              spawnX,
              pos.getY(),
              spawnZ,
              Math.fround(random.nextFloat() * 360.0),
              0.0,
              levelRandom,
            );
            // Runtime: generated mobs go to the host-owned entity sink instead of mutating ServerLevel directly.
            sink.addFreshEntityWithPassengers(entity);
            spawned = true;
          }

          x += random.nextInt(5) - random.nextInt(5);
          z += random.nextInt(5) - random.nextInt(5);

          while (x < minBlockX || x >= minBlockX + CHUNK_WIDTH || z < minBlockZ || z >= minBlockZ + CHUNK_WIDTH) {
            x = originX + random.nextInt(5) - random.nextInt(5);
            z = originZ + random.nextInt(5) - random.nextInt(5);
          }
        }
      }
    }
  }
}

function getTopNonCollidingPos(level: WorldGenLevel, type: EntityType, x: number, z: number): BlockPos {
  const height = level.getHeight(getHeightmapType(type), x, z);
  const pos = new BlockPos(x, height, z);
  if (getPlacementType(type) === SpawnPlacementType.ON_GROUND) {
    const below = pos.below();
    if (isPathfindableLand(level, below)) {
      return below;
    }
  }

  return pos;
}

function blockPosFromDouble(x: number, y: number, z: number): BlockPos {
  return new BlockPos(floor(x), floor(y), floor(z));
}

function isPathfindableLand(level: WorldGenLevel, pos: BlockPos): boolean {
  const state = level.getBlockState(pos);
  // Runtime: pathfinding shape parity is deferred; non-colliding dry blocks match the worldgen spawn use here.
  return !state.isCollisionShapeFullBlock(level, pos) && state.getFluidState().isEmpty();
}

function noCollision(level: WorldGenLevel, box: AABB): boolean {
  const epsilon = 1.0e-7;
  const minX = floor(box.minX);
  const minY = floor(box.minY);
  const minZ = floor(box.minZ);
  const maxX = floor(box.maxX - epsilon);
  const maxY = floor(box.maxY - epsilon);
  const maxZ = floor(box.maxZ - epsilon);

  for (let y = minY; y <= maxY; y++) {
    for (let z = minZ; z <= maxZ; z++) {
      for (let x = minX; x <= maxX; x++) {
        const pos = new BlockPos(x, y, z);
        if (level.getBlockState(pos).isCollisionShapeFullBlock(level, pos)) {
          return false;
        }
      }
    }
  }

  return true;
}
