import { decodeNbt, type NamedNbt } from "./nbt.ts";

const SECTOR_BYTES = 4096;
const SECTOR_INTS = 1024;
const CHUNK_HEADER_SIZE = 5;
const EXTERNAL_STREAM_FLAG = 0x80;

export const COMPRESSION_GZIP = 1;
export const COMPRESSION_ZLIB = 2;
export const COMPRESSION_NONE = 3;

export type DecompressFn = (payload: Uint8Array, kind: number) => Uint8Array;

export interface RegionChunkLocation {
  readonly localX: number;
  readonly localZ: number;
}

export class RegionFile {
  private readonly bytes: Uint8Array;
  private readonly decompress: DecompressFn;

  public constructor(bytes: Uint8Array, decompress: DecompressFn) {
    if (bytes.byteLength < SECTOR_BYTES * 2) {
      throw new Error(`region file is shorter than its 8KB header (${bytes.byteLength} bytes)`);
    }
    this.bytes = bytes;
    this.decompress = decompress;
  }

  private locationIndex(chunkX: number, chunkZ: number): number {
    const local = ((chunkX & 31) + 32 * (chunkZ & 31)) | 0;
    if (local < 0 || local >= SECTOR_INTS) {
      throw new Error(`local chunk index ${local} out of range`);
    }
    return local;
  }

  public hasChunk(chunkX: number, chunkZ: number): boolean {
    const view = new DataView(this.bytes.buffer, this.bytes.byteOffset, this.bytes.byteLength);
    const location = view.getInt32(this.locationIndex(chunkX, chunkZ) * 4, false);
    return location !== 0;
  }

  public readChunkNbt(chunkX: number, chunkZ: number): NamedNbt | undefined {
    const view = new DataView(this.bytes.buffer, this.bytes.byteOffset, this.bytes.byteLength);
    const location = view.getInt32(this.locationIndex(chunkX, chunkZ) * 4, false);
    if (location === 0) {
      return undefined;
    }

    const sector = (location >>> 8) & 0xFFFFFF;
    const sectorCount = location & 0xFF;
    if (sector < 2) {
      throw new Error(`chunk (${chunkX}, ${chunkZ}) has invalid sector ${sector}`);
    }
    if (sectorCount <= 0) {
      throw new Error(`chunk (${chunkX}, ${chunkZ}) has non-positive sector count ${sectorCount}`);
    }

    const offset = sector * SECTOR_BYTES;
    if (offset + CHUNK_HEADER_SIZE > this.bytes.byteLength) {
      throw new Error(`chunk (${chunkX}, ${chunkZ}) offset ${offset} is past end of region`);
    }

    const payloadLength = view.getInt32(offset, false);
    const compressionByte = view.getUint8(offset + 4);
    if ((compressionByte & EXTERNAL_STREAM_FLAG) !== 0) {
      throw new Error(
        `chunk (${chunkX}, ${chunkZ}) is stored externally (.mcc sidecar); reader does not support that`,
      );
    }

    const bodyStart = offset + CHUNK_HEADER_SIZE;
    const bodyEnd = offset + 4 + payloadLength;
    if (bodyEnd > this.bytes.byteLength) {
      throw new Error(`chunk (${chunkX}, ${chunkZ}) body ${bodyStart}..${bodyEnd} runs past region end`);
    }

    const compressed = this.bytes.subarray(bodyStart, bodyEnd);
    const raw = this.decompress(compressed, compressionByte);
    return decodeNbt(raw);
  }
}

export function listPresentChunks(region: RegionFile): RegionChunkLocation[] {
  const out: RegionChunkLocation[] = [];
  for (let localZ = 0; localZ < 32; localZ++) {
    for (let localX = 0; localX < 32; localX++) {
      if (region.hasChunk(localX, localZ)) {
        out.push({ localX, localZ });
      }
    }
  }
  return out;
}
