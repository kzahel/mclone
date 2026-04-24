import { BLOCKS_PER_SECTION } from "../../worldgen/chunk/chunk-block-buffer";
import { BitStorage, paletteBitsFor } from "../../util/bit-storage";
import type { BlockStateId, BlockStateIdMap } from "./block/state/block-state-id";
import {
  CHUNK_SNAPSHOT_BLOCK_ORDER,
  serializeBlockStateSnapshot,
  type BlockStateResolver,
  type ChunkSectionSnapshot,
  type ChunkSnapshot,
} from "./chunk-snapshot";
import { cloneScheduledTickSnapshot, type ScheduledTickSnapshot } from "./scheduled-tick";

export interface PackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: Uint32Array;
  readonly bitsPerBlock: number;
  readonly packedBlockIndices: BigInt64Array;
}

export interface PackedChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly PackedChunkSection[];
  readonly blockTicks: readonly ScheduledTickSnapshot[];
  readonly liquidTicks: readonly ScheduledTickSnapshot[];
}

export function clonePackedChunkSection(section: PackedChunkSection): PackedChunkSection {
  return {
    y: section.y,
    paletteStateIds: new Uint32Array(section.paletteStateIds),
    bitsPerBlock: section.bitsPerBlock,
    packedBlockIndices: new BigInt64Array(section.packedBlockIndices),
  };
}

export function clonePackedChunkSnapshot(snapshot: PackedChunkSnapshot): PackedChunkSnapshot {
  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map(clonePackedChunkSection),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
}

export function collectPackedChunkSnapshotTransferables(snapshot: PackedChunkSnapshot): Transferable[] {
  const transferables: Transferable[] = [];
  for (const section of snapshot.sections) {
    transferables.push(section.paletteStateIds.buffer, section.packedBlockIndices.buffer);
  }
  return transferables;
}

export function packChunkSnapshot(
  snapshot: ChunkSnapshot,
  stateIds: BlockStateIdMap,
  resolveState: BlockStateResolver,
): PackedChunkSnapshot {
  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => packChunkSection(section, stateIds, resolveState)),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
}

export function unpackChunkSnapshot(snapshot: PackedChunkSnapshot, stateIds: BlockStateIdMap): ChunkSnapshot {
  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => unpackChunkSection(section, stateIds)),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
}

export function packChunkSection(
  section: ChunkSectionSnapshot,
  stateIds: BlockStateIdMap,
  resolveState: BlockStateResolver,
): PackedChunkSection {
  if (section.blockOrder !== CHUNK_SNAPSHOT_BLOCK_ORDER) {
    throw new Error(`Unsupported chunk section block order ${section.blockOrder}`);
  }
  if (section.palette.length <= 0) {
    throw new Error(`Chunk section ${section.y} cannot be packed with an empty palette`);
  }
  if (section.blocks.length !== BLOCKS_PER_SECTION) {
    throw new Error(`Chunk section ${section.y} had ${section.blocks.length} blocks instead of ${BLOCKS_PER_SECTION}`);
  }

  const paletteStateIds = new Uint32Array(section.palette.length);
  for (let index = 0; index < section.palette.length; index++) {
    paletteStateIds[index] = stateIds.idFor(resolveState(section.palette[index]!));
  }

  const bitsPerBlock = bitsForLocalPalette(section.palette.length);
  const storage = new BitStorage(bitsPerBlock, BLOCKS_PER_SECTION);
  for (let index = 0; index < section.blocks.length; index++) {
    const paletteIndex = section.blocks[index]!;
    if (!Number.isInteger(paletteIndex) || paletteIndex < 0 || paletteIndex >= section.palette.length) {
      throw new Error(`Chunk section ${section.y} referenced palette entry ${paletteIndex}`);
    }
    storage.set(index, paletteIndex);
  }

  return {
    y: section.y,
    paletteStateIds,
    bitsPerBlock,
    packedBlockIndices: storage.getRaw(),
  };
}

export function unpackChunkSection(section: PackedChunkSection, stateIds: BlockStateIdMap): ChunkSectionSnapshot {
  if (section.paletteStateIds.length <= 0) {
    throw new Error(`Packed chunk section ${section.y} has an empty palette`);
  }

  const expectedBits = bitsForLocalPalette(section.paletteStateIds.length);
  if (section.bitsPerBlock !== expectedBits) {
    throw new Error(`Packed chunk section ${section.y} used ${section.bitsPerBlock} bits, expected ${expectedBits}`);
  }

  const palette = Array.from(section.paletteStateIds, (stateId) =>
    serializeBlockStateSnapshot(stateIds.stateFor(stateId as BlockStateId))
  );
  const storage = new BitStorage(section.bitsPerBlock, BLOCKS_PER_SECTION, section.packedBlockIndices);
  const blocks = storage.getAll();
  for (const paletteIndex of blocks) {
    if (paletteIndex < 0 || paletteIndex >= palette.length) {
      throw new Error(`Packed chunk section ${section.y} referenced palette entry ${paletteIndex}`);
    }
  }

  return {
    y: section.y,
    palette,
    blockOrder: CHUNK_SNAPSHOT_BLOCK_ORDER,
    blocks,
  };
}

export function bitsForLocalPalette(paletteSize: number): number {
  return Math.max(4, paletteBitsFor(paletteSize));
}
