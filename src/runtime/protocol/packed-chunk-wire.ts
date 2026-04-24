import { cloneScheduledTickSnapshot, type ScheduledTickSnapshot } from "../../world/level/scheduled-tick";
import { DataLayer } from "../../world/level/chunk/data-layer";
import type { PackedChunkLight, PackedChunkLightDelta, PackedChunkSnapshot, PackedLightSectionUpdate } from "../../world/level/packed-chunk-snapshot";

export interface SerializedPackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: readonly number[];
  readonly bitsPerBlock: number;
  readonly packedBlockIndicesBase64: string;
}

export interface SerializedPackedLightSection {
  readonly y: number;
  readonly dataBase64: string;
}

export interface SerializedPackedChunkLight {
  readonly sky: readonly SerializedPackedLightSection[];
  readonly block: readonly SerializedPackedLightSection[];
  readonly lightCorrect: boolean;
}

export interface SerializedPackedLightSectionUpdate {
  readonly y: number;
  readonly dataBase64?: string;
}

export interface SerializedPackedChunkLightDelta {
  readonly sky?: readonly SerializedPackedLightSectionUpdate[];
  readonly block?: readonly SerializedPackedLightSectionUpdate[];
}

export interface SerializedPackedChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly SerializedPackedChunkSection[];
  readonly light?: SerializedPackedChunkLight;
  readonly blockTicks: readonly ScheduledTickSnapshot[];
  readonly liquidTicks: readonly ScheduledTickSnapshot[];
}

const BASE64_CHUNK_SIZE = 0x8000;

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += BASE64_CHUNK_SIZE) {
    const chunk = bytes.subarray(offset, offset + BASE64_CHUNK_SIZE);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function serializeBigInt64Words(words: BigInt64Array): string {
  const bytes = new Uint8Array(words.length * 8);
  const view = new DataView(bytes.buffer);
  for (let index = 0; index < words.length; index++) {
    view.setBigInt64(index * 8, words[index]!, true);
  }
  return bytesToBase64(bytes);
}

function deserializeBigInt64Words(value: string): BigInt64Array {
  const bytes = base64ToBytes(value);
  if (bytes.byteLength % 8 !== 0) {
    throw new Error(`Packed chunk word payload length ${bytes.byteLength.toString()} is not a multiple of 8`);
  }

  const words = new BigInt64Array(bytes.byteLength / 8);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let index = 0; index < words.length; index++) {
    words[index] = view.getBigInt64(index * 8, true);
  }
  return words;
}

export function serializePackedChunkSnapshot(snapshot: PackedChunkSnapshot): SerializedPackedChunkSnapshot {
  const serialized: SerializedPackedChunkSnapshot = {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => ({
      y: section.y,
      paletteStateIds: [...section.paletteStateIds],
      bitsPerBlock: section.bitsPerBlock,
      packedBlockIndicesBase64: serializeBigInt64Words(section.packedBlockIndices),
    })),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
  return snapshot.light === undefined ? serialized : { ...serialized, light: serializePackedChunkLight(snapshot.light) };
}

export function deserializePackedChunkSnapshot(snapshot: SerializedPackedChunkSnapshot): PackedChunkSnapshot {
  const deserialized: PackedChunkSnapshot = {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: [...snapshot.biomes],
    sections: snapshot.sections.map((section) => ({
      y: section.y,
      paletteStateIds: Uint32Array.from(section.paletteStateIds),
      bitsPerBlock: section.bitsPerBlock,
      packedBlockIndices: deserializeBigInt64Words(section.packedBlockIndicesBase64),
    })),
    blockTicks: snapshot.blockTicks.map(cloneScheduledTickSnapshot),
    liquidTicks: snapshot.liquidTicks.map(cloneScheduledTickSnapshot),
  };
  return snapshot.light === undefined ? deserialized : { ...deserialized, light: deserializePackedChunkLight(snapshot.light) };
}

export function serializePackedChunkLightDelta(delta: PackedChunkLightDelta): SerializedPackedChunkLightDelta {
  return {
    sky: delta.sky?.map((section) => serializePackedLightSectionUpdate(section, "sky")),
    block: delta.block?.map((section) => serializePackedLightSectionUpdate(section, "block")),
  };
}

export function deserializePackedChunkLightDelta(delta: SerializedPackedChunkLightDelta): PackedChunkLightDelta {
  return {
    sky: delta.sky?.map((section) => deserializePackedLightSectionUpdate(section, "sky")),
    block: delta.block?.map((section) => deserializePackedLightSectionUpdate(section, "block")),
  };
}

function serializePackedChunkLight(light: PackedChunkLight): SerializedPackedChunkLight {
  return {
    sky: light.sky.map((section) => serializePackedLightSection(section, "sky")),
    block: light.block.map((section) => serializePackedLightSection(section, "block")),
    lightCorrect: light.lightCorrect,
  };
}

function serializePackedLightSection(
  section: PackedChunkLight["sky"][number],
  layer: "sky" | "block",
): SerializedPackedLightSection {
  if (section.data.length !== DataLayer.SIZE) {
    throw new Error(`${layer} light section ${section.y} had ${section.data.length} bytes instead of ${DataLayer.SIZE}`);
  }
  return {
    y: section.y,
    dataBase64: bytesToBase64(section.data),
  };
}

function serializePackedLightSectionUpdate(
  section: PackedLightSectionUpdate,
  layer: "sky" | "block",
): SerializedPackedLightSectionUpdate {
  if (section.data === undefined) {
    return { y: section.y };
  }
  if (section.data.length !== DataLayer.SIZE) {
    throw new Error(`${layer} light section update ${section.y} had ${section.data.length} bytes instead of ${DataLayer.SIZE}`);
  }
  return {
    y: section.y,
    dataBase64: bytesToBase64(section.data),
  };
}

function deserializePackedChunkLight(light: SerializedPackedChunkLight): PackedChunkLight {
  return {
    sky: light.sky.map((section) => deserializePackedLightSection(section, "sky")),
    block: light.block.map((section) => deserializePackedLightSection(section, "block")),
    lightCorrect: light.lightCorrect,
  };
}

function deserializePackedLightSectionUpdate(
  section: SerializedPackedLightSectionUpdate,
  layer: "sky" | "block",
): PackedLightSectionUpdate {
  if (section.dataBase64 === undefined) {
    return { y: section.y };
  }

  const data = base64ToBytes(section.dataBase64);
  if (data.length !== DataLayer.SIZE) {
    throw new Error(`${layer} light section update ${section.y} had ${data.length} bytes instead of ${DataLayer.SIZE}`);
  }
  return {
    y: section.y,
    data,
  };
}

function deserializePackedLightSection(
  section: SerializedPackedLightSection,
  layer: "sky" | "block",
): PackedChunkLight["sky"][number] {
  const data = base64ToBytes(section.dataBase64);
  if (data.length !== DataLayer.SIZE) {
    throw new Error(`${layer} light section ${section.y} had ${data.length} bytes instead of ${DataLayer.SIZE}`);
  }
  return {
    y: section.y,
    data,
  };
}
