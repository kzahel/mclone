import { deflateSync, inflateSync } from "node:zlib";
import { describe, expect, test } from "vitest";

import { COMPRESSION_ZLIB, listPresentChunks, RegionFile } from "../../src/oracle/anvil/region.ts";
import {
  encodeModifiedUtf8,
  NBT_TAG_BYTE,
  NBT_TAG_COMPOUND,
  NBT_TAG_INT,
} from "../../src/oracle/anvil/nbt.ts";

const SECTOR = 4096;

function writeUInt16BE(view: DataView, offset: number, value: number): void {
  view.setUint16(offset, value, false);
}

function buildTinyNbt(): Uint8Array {
  const name = encodeModifiedUtf8("");
  const keyName = encodeModifiedUtf8("answer");
  const total = 1 + 2 + name.length + 1 + 2 + keyName.length + 4 + 1;
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  let offset = 0;
  out[offset++] = NBT_TAG_COMPOUND;
  writeUInt16BE(view, offset, name.length);
  offset += 2;
  out.set(name, offset);
  offset += name.length;
  out[offset++] = NBT_TAG_INT;
  writeUInt16BE(view, offset, keyName.length);
  offset += 2;
  out.set(keyName, offset);
  offset += keyName.length;
  view.setInt32(offset, 42, false);
  offset += 4;
  out[offset++] = 0;
  return out;
}

function buildRegionWithOneChunk(chunkLocalX: number, chunkLocalZ: number): Uint8Array {
  const nbt = buildTinyNbt();
  const compressed = deflateSync(nbt);
  const payloadLength = compressed.byteLength + 1; // +1 for the compression-type byte
  const sectorsNeeded = Math.ceil((payloadLength + 4) / SECTOR);
  const region = new Uint8Array(SECTOR * (2 + sectorsNeeded));
  const view = new DataView(region.buffer);
  const localIndex = ((chunkLocalX & 31) + 32 * (chunkLocalZ & 31)) * 4;
  view.setInt32(localIndex, (2 << 8) | sectorsNeeded, false);
  view.setInt32(SECTOR + localIndex, 1234567, false);
  const chunkOffset = 2 * SECTOR;
  view.setInt32(chunkOffset, payloadLength, false);
  region[chunkOffset + 4] = COMPRESSION_ZLIB;
  region.set(compressed, chunkOffset + 5);
  return region;
}

function inflate(payload: Uint8Array, kind: number): Uint8Array {
  if (kind !== COMPRESSION_ZLIB) {
    throw new Error(`test region only uses zlib, got kind ${kind}`);
  }
  return new Uint8Array(inflateSync(payload));
}

describe("RegionFile", () => {
  test("decodes a single zlib-compressed chunk payload", () => {
    const bytes = buildRegionWithOneChunk(5, 7);
    const region = new RegionFile(bytes, inflate);
    expect(region.hasChunk(5, 7)).toBe(true);
    expect(region.hasChunk(0, 0)).toBe(false);
    const decoded = region.readChunkNbt(5, 7);
    expect(decoded?.value["answer"]).toBe(42);
  });

  test("listPresentChunks enumerates every populated slot", () => {
    const bytes = buildRegionWithOneChunk(12, 3);
    const region = new RegionFile(bytes, inflate);
    const present = listPresentChunks(region);
    expect(present).toEqual([{ localX: 12, localZ: 3 }]);
  });

  test("rejects external .mcc sidecar chunks", () => {
    const bytes = buildRegionWithOneChunk(0, 0);
    // Flip the external-stream flag on the chunk's compression byte.
    const view = new DataView(bytes.buffer);
    const chunkOffset = 2 * SECTOR;
    view.setUint8(chunkOffset + 4, COMPRESSION_ZLIB | 0x80);
    const region = new RegionFile(bytes, inflate);
    expect(() => region.readChunkNbt(0, 0)).toThrow(/stored externally/);
  });

  test("rejects truncated regions", () => {
    const bytes = new Uint8Array(1024);
    expect(() => new RegionFile(bytes, inflate)).toThrow(/shorter than its 8KB header/);
  });

  test("bits just the chunk coord's local position when reading", () => {
    const bytes = buildRegionWithOneChunk(5, 7);
    const region = new RegionFile(bytes, inflate);
    // chunkX 5 + 32 still lands on local 5; the region reader only cares
    // about the low 5 bits of the chunk coord.
    expect(region.hasChunk(5 + 32, 7 - 32)).toBe(true);
  });
});

describe("NBT + region roundtrip", () => {
  test("decodes the tiny NBT payload back to its original fields", () => {
    const nbt = buildTinyNbt();
    const compressed = deflateSync(nbt);
    expect(compressed.byteLength).toBeGreaterThan(0);
    const region = new RegionFile(buildRegionWithOneChunk(0, 0), inflate);
    const decoded = region.readChunkNbt(0, 0);
    expect(decoded?.name).toBe("");
    expect(decoded?.value["answer"]).toBe(42);
    // And verify the tag ID is preserved (TAG_Int -> number).
    expect(typeof decoded?.value["answer"]).toBe("number");
    expect(NBT_TAG_BYTE).toBe(1);
  });
});
