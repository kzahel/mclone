import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { BLOCKS_PER_SECTION, CHUNK_WIDTH, SECTION_HEIGHT } from "../../worldgen/chunk/chunk-block-buffer";
import { LevelChunk } from "./chunk/level-chunk";
import type { Block } from "./block/block";
import type { BlockState } from "./block/state/block-state";
import type { Property } from "./block/state/properties/property";
import {
  cloneScheduledTickSnapshot,
  resolveBlockTickTarget,
  resolveFluidTickTarget,
  type ScheduledTickSnapshot,
} from "./scheduled-tick";

const AIR_BLOCK_NAME = "minecraft:air";
export const CHUNK_SNAPSHOT_BLOCK_ORDER = "y-major,z-major,x-minor";

export interface BlockStateSnapshot {
  readonly name: string;
  readonly properties?: Readonly<Record<string, string>>;
}

export interface ChunkSectionSnapshot {
  readonly y: number;
  readonly palette: readonly BlockStateSnapshot[];
  readonly blockOrder: typeof CHUNK_SNAPSHOT_BLOCK_ORDER;
  readonly blocks: readonly number[];
}

export interface ChunkLightSectionSnapshot {
  readonly y: number;
  readonly data: Uint8Array;
}

export interface ChunkLightSnapshot {
  readonly sky: readonly ChunkLightSectionSnapshot[];
  readonly block: readonly ChunkLightSectionSnapshot[];
  readonly lightCorrect: boolean;
}

export interface ChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly ChunkSectionSnapshot[];
  readonly light?: ChunkLightSnapshot;
  readonly blockTicks: readonly ScheduledTickSnapshot[];
  readonly liquidTicks: readonly ScheduledTickSnapshot[];
}

export type BlockStateResolver = (snapshot: BlockStateSnapshot) => BlockState;

export function blockStateSnapshotKey(snapshot: BlockStateSnapshot): string {
  const properties = Object.entries(snapshot.properties ?? {}).sort(([left], [right]) => left.localeCompare(right));
  if (properties.length === 0) {
    return snapshot.name;
  }

  return `${snapshot.name}[${properties.map(([key, value]) => `${key}=${value}`).join(",")}]`;
}

export function serializeBlockStateSnapshot(state: BlockState): BlockStateSnapshot {
  if (state.isAir()) {
    return { name: AIR_BLOCK_NAME };
  }

  const blockName = Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
  if (blockName === undefined) {
    throw new Error(`Cannot snapshot unregistered block state ${state}`);
  }

  const properties = [...state.getValues().entries()]
    .map(([property, value]) => [property.getName(), property.getNameForValue(value as never)] as const)
    .sort(([left], [right]) => left.localeCompare(right));

  if (properties.length === 0) {
    return { name: blockName };
  }

  return {
    name: blockName,
    properties: Object.fromEntries(properties),
  };
}

export function createBlockStateResolver(airState: BlockState): BlockStateResolver {
  const cache = new Map<string, BlockState>();

  return (snapshot) => {
    const key = blockStateSnapshotKey(snapshot);
    const cached = cache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    let state: BlockState;
    if (snapshot.name === AIR_BLOCK_NAME) {
      state = airState;
    } else {
      const block = Registry.BLOCK.get(new ResourceLocation(snapshot.name)) as Block | undefined;
      if (block === undefined) {
        throw new Error(`Cannot hydrate missing block ${snapshot.name}`);
      }

      state = block.defaultBlockState();
      const properties = Object.entries(snapshot.properties ?? {}).sort(([left], [right]) => left.localeCompare(right));
      for (const [propertyName, propertyValueName] of properties) {
        const property = block.getStateDefinition().getProperty(propertyName);
        if (property === undefined) {
          throw new Error(`Block ${snapshot.name} has no property ${propertyName}`);
        }

        const propertyValue = property.getValue(propertyValueName);
        if (propertyValue === undefined) {
          throw new Error(`Block ${snapshot.name} has no value ${propertyValueName} for property ${propertyName}`);
        }

        state = state.setValue(property as Property<unknown>, propertyValue);
      }
    }

    cache.set(key, state);
    return state;
  };
}

export function buildChunkSnapshot(
  chunk: LevelChunk,
  biomes: readonly number[],
  minBuildHeight: number,
  height: number,
): ChunkSnapshot {
  const minSectionY = Math.floor(minBuildHeight / SECTION_HEIGHT);
  const sectionCount = height / SECTION_HEIGHT;
  const worldX = SectionPos.sectionToBlockCoord(chunk.chunkX);
  const worldZ = SectionPos.sectionToBlockCoord(chunk.chunkZ);
  const pos = new BlockPos.MutableBlockPos();
  const sections: ChunkSectionSnapshot[] = [];

  for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
    const sectionY = minSectionY + sectionOffset;
    const sectionMinY = minBuildHeight + (sectionOffset * SECTION_HEIGHT);
    const palette: BlockStateSnapshot[] = [];
    const paletteIndexByKey = new Map<string, number>();
    const blocks = new Array<number>(BLOCKS_PER_SECTION);
    let hasNonAir = false;
    let index = 0;

    for (let localY = 0; localY < SECTION_HEIGHT; localY++) {
      for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
        for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
          pos.set(worldX + localX, sectionMinY + localY, worldZ + localZ);
          const state = chunk.getBlockState(pos);
          hasNonAir = hasNonAir || !state.isAir();

          const snapshot = serializeBlockStateSnapshot(state);
          const key = blockStateSnapshotKey(snapshot);
          let paletteIndex = paletteIndexByKey.get(key);
          if (paletteIndex === undefined) {
            paletteIndex = palette.length;
            palette.push(snapshot);
            paletteIndexByKey.set(key, paletteIndex);
          }

          blocks[index++] = paletteIndex;
        }
      }
    }

    if (!hasNonAir) {
      continue;
    }

    sections.push({
      y: sectionY,
      palette,
      blockOrder: CHUNK_SNAPSHOT_BLOCK_ORDER,
      blocks,
    });
  }

  return {
    chunkX: chunk.chunkX,
    chunkZ: chunk.chunkZ,
    biomes: [...biomes],
    sections,
    blockTicks: chunk.getScheduledBlockTicks().map(cloneScheduledTickSnapshot),
    liquidTicks: chunk.getScheduledLiquidTicks().map(cloneScheduledTickSnapshot),
  };
}

export function hydrateChunkFromSnapshot(
  snapshot: ChunkSnapshot,
  airState: BlockState,
  resolveState: BlockStateResolver,
): LevelChunk {
  const chunk = new LevelChunk(snapshot.chunkX, snapshot.chunkZ, airState);
  const worldX = SectionPos.sectionToBlockCoord(snapshot.chunkX);
  const worldZ = SectionPos.sectionToBlockCoord(snapshot.chunkZ);
  const pos = new BlockPos.MutableBlockPos();

  for (const section of snapshot.sections) {
    if (section.blockOrder !== CHUNK_SNAPSHOT_BLOCK_ORDER) {
      throw new Error(`Unsupported chunk snapshot block order ${section.blockOrder}`);
    }

    if (section.blocks.length !== BLOCKS_PER_SECTION) {
      throw new Error(
        `Chunk snapshot section (${snapshot.chunkX}, ${snapshot.chunkZ}, ${section.y}) had ${section.blocks.length} blocks instead of ${BLOCKS_PER_SECTION}`,
      );
    }

    const sectionMinY = section.y * SECTION_HEIGHT;
    const palette = section.palette.map(resolveState);
    let index = 0;
    for (let localY = 0; localY < SECTION_HEIGHT; localY++) {
      for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
        for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
          const paletteIndex = section.blocks[index++]!;
          const state = palette[paletteIndex];
          if (state === undefined) {
            throw new Error(
              `Chunk snapshot section (${snapshot.chunkX}, ${snapshot.chunkZ}, ${section.y}) referenced palette entry ${paletteIndex}`,
            );
          }

          if (state.isAir()) {
            continue;
          }

          pos.set(worldX + localX, sectionMinY + localY, worldZ + localZ);
          chunk.setBlockState(pos, state);
        }
      }
    }
  }

  for (const tick of snapshot.blockTicks ?? []) {
    resolveBlockTickTarget(tick.target);
    chunk.recordBlockTick(new BlockPos(tick.x, tick.y, tick.z), tick.target, tick.delay);
  }

  for (const tick of snapshot.liquidTicks ?? []) {
    resolveFluidTickTarget(tick.target);
    chunk.recordLiquidTick(new BlockPos(tick.x, tick.y, tick.z), tick.target, tick.delay);
  }

  return chunk;
}
