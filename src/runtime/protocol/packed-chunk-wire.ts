import { cloneScheduledTickSnapshot, type ScheduledTickSnapshot } from "../../world/level/scheduled-tick";
import type { PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";

export interface SerializedPackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: readonly number[];
  readonly bitsPerBlock: number;
  readonly packedBlockIndicesBase64: string;
}

export interface SerializedPackedChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly SerializedPackedChunkSection[];
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
  return {
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
}

export function deserializePackedChunkSnapshot(snapshot: SerializedPackedChunkSnapshot): PackedChunkSnapshot {
  return {
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
}
