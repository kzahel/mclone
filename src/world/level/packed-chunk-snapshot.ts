import { BLOCKS_PER_SECTION } from "../../worldgen/chunk/chunk-block-buffer";
import { BitStorage, paletteBitsFor } from "../../util/bit-storage";
import { DataLayer } from "./chunk/data-layer";
import type { BlockStateId, BlockStateIdMap } from "./block/state/block-state-id";
import {
  CHUNK_SNAPSHOT_BLOCK_ORDER,
  serializeBlockStateSnapshot,
  type BlockStateResolver,
  type ChunkSectionSnapshot,
  type ChunkSnapshot,
} from "./chunk-snapshot";
import { cloneScheduledTickSnapshot, type ScheduledTickSnapshot } from "./scheduled-tick";

export interface PackedLightSection {
  readonly y: number;
  readonly data: Uint8Array;
}

export interface PackedChunkLight {
  readonly sky: readonly PackedLightSection[];
  readonly block: readonly PackedLightSection[];
  readonly lightCorrect: boolean;
}

export interface PackedLightSectionUpdate {
  readonly y: number;
  readonly data?: Uint8Array;
}

export interface PackedChunkLightDelta {
  readonly sky?: readonly PackedLightSectionUpdate[];
  readonly block?: readonly PackedLightSectionUpdate[];
}

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
  readonly light?: PackedChunkLight;
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

export function clonePackedLightSection(section: PackedLightSection): PackedLightSection {
  validatePackedLightSection(section);
  return {
    y: section.y,
    data: new Uint8Array(section.data),
  };
}

export function clonePackedChunkLight(light: PackedChunkLight): PackedChunkLight {
  return {
    sky: light.sky.map(clonePackedLightSection),
    block: light.block.map(clonePackedLightSection),
    lightCorrect: light.lightCorrect,
  };
}

export function clonePackedLightSectionUpdate(section: PackedLightSectionUpdate): PackedLightSectionUpdate {
  validatePackedLightSectionUpdate(section);
  return section.data === undefined
    ? { y: section.y }
    : { y: section.y, data: new Uint8Array(section.data) };
}

export function clonePackedChunkLightDelta(light: PackedChunkLightDelta): PackedChunkLightDelta {
  return {
    sky: light.sky?.map(clonePackedLightSectionUpdate),
    block: light.block?.map(clonePackedLightSectionUpdate),
  };
}

export function clonePackedChunkSnapshot(snapshot: PackedChunkSnapshot): PackedChunkSnapshot {
  const clone: PackedChunkSnapshot = {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map(clonePackedChunkSection),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
  return snapshot.light === undefined ? clone : { ...clone, light: clonePackedChunkLight(snapshot.light) };
}

export function applyPackedChunkLightDeltaToSnapshot(
  snapshot: PackedChunkSnapshot,
  delta: PackedChunkLightDelta,
): PackedChunkSnapshot {
  return {
    ...snapshot,
    light: {
      sky: applyPackedLightSectionUpdates(snapshot.light?.sky ?? [], delta.sky ?? []),
      block: applyPackedLightSectionUpdates(snapshot.light?.block ?? [], delta.block ?? []),
      lightCorrect: true,
    },
  };
}

export function collectPackedChunkLightDeltaTransferables(light: PackedChunkLightDelta): Transferable[] {
  const transferables: Transferable[] = [];
  for (const section of [...(light.sky ?? []), ...(light.block ?? [])]) {
    validatePackedLightSectionUpdate(section);
    if (section.data !== undefined) {
      transferables.push(section.data.buffer);
    }
  }
  return transferables;
}

export function collectPackedChunkSnapshotTransferables(snapshot: PackedChunkSnapshot): Transferable[] {
  const transferables: Transferable[] = [];
  for (const section of snapshot.sections) {
    transferables.push(section.paletteStateIds.buffer, section.packedBlockIndices.buffer);
  }
  if (snapshot.light !== undefined) {
    for (const section of [...snapshot.light.sky, ...snapshot.light.block]) {
      validatePackedLightSection(section);
      transferables.push(section.data.buffer);
    }
  }
  return transferables;
}

function applyPackedLightSectionUpdates(
  existing: readonly PackedLightSection[],
  updates: readonly PackedLightSectionUpdate[],
): readonly PackedLightSection[] {
  const next = new Map(existing.map((section) => [section.y, clonePackedLightSection(section)] as const));
  for (const update of updates) {
    next.set(update.y, update.data === undefined
      ? { y: update.y, data: new Uint8Array(DataLayer.SIZE) }
      : { y: update.y, data: new Uint8Array(update.data) });
  }

  return [...next.values()].sort((left, right) => left.y - right.y);
}

export function packChunkSnapshot(
  snapshot: ChunkSnapshot,
  stateIds: BlockStateIdMap,
  resolveState: BlockStateResolver,
): PackedChunkSnapshot {
  const packed: PackedChunkSnapshot = {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => packChunkSection(section, stateIds, resolveState)),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
  return snapshot.light === undefined ? packed : { ...packed, light: clonePackedChunkLight(snapshot.light) };
}

export function unpackChunkSnapshot(snapshot: PackedChunkSnapshot, stateIds: BlockStateIdMap): ChunkSnapshot {
  const unpacked: ChunkSnapshot = {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => unpackChunkSection(section, stateIds)),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
  return snapshot.light === undefined ? unpacked : { ...unpacked, light: clonePackedChunkLight(snapshot.light) };
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

function validatePackedLightSection(section: PackedLightSection): void {
  if (section.data.length !== DataLayer.SIZE) {
    throw new Error(`Packed light section ${section.y} had ${section.data.length} bytes instead of ${DataLayer.SIZE}`);
  }
}

function validatePackedLightSectionUpdate(section: PackedLightSectionUpdate): void {
  if (section.data !== undefined && section.data.length !== DataLayer.SIZE) {
    throw new Error(`Packed light section update ${section.y} had ${section.data.length} bytes instead of ${DataLayer.SIZE}`);
  }
}
