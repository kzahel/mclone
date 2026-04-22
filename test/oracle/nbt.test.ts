import { describe, expect, test } from "vitest";

import {
  decodeNbt,
  encodeModifiedUtf8,
  NBT_TAG_BYTE,
  NBT_TAG_COMPOUND,
  NBT_TAG_DOUBLE,
  NBT_TAG_FLOAT,
  NBT_TAG_INT,
  NBT_TAG_INT_ARRAY,
  NBT_TAG_LIST,
  NBT_TAG_LONG,
  NBT_TAG_LONG_ARRAY,
  NBT_TAG_SHORT,
  NBT_TAG_STRING,
  type NbtCompound,
  type NbtList,
} from "../../src/oracle/anvil/nbt.ts";

class NbtWriter {
  public readonly chunks: Uint8Array[] = [];

  public writeTag(tag: number): this {
    this.chunks.push(new Uint8Array([tag & 0xFF]));
    return this;
  }

  public writeName(name: string): this {
    const encoded = encodeModifiedUtf8(name);
    const header = new Uint8Array(2);
    new DataView(header.buffer).setUint16(0, encoded.length, false);
    this.chunks.push(header);
    this.chunks.push(encoded);
    return this;
  }

  public writeInt8(value: number): this {
    this.chunks.push(new Uint8Array([value & 0xFF]));
    return this;
  }

  public writeInt16(value: number): this {
    const buffer = new Uint8Array(2);
    new DataView(buffer.buffer).setInt16(0, value, false);
    this.chunks.push(buffer);
    return this;
  }

  public writeInt32(value: number): this {
    const buffer = new Uint8Array(4);
    new DataView(buffer.buffer).setInt32(0, value, false);
    this.chunks.push(buffer);
    return this;
  }

  public writeInt64(value: bigint): this {
    const buffer = new Uint8Array(8);
    new DataView(buffer.buffer).setBigInt64(0, value, false);
    this.chunks.push(buffer);
    return this;
  }

  public writeFloat32(value: number): this {
    const buffer = new Uint8Array(4);
    new DataView(buffer.buffer).setFloat32(0, value, false);
    this.chunks.push(buffer);
    return this;
  }

  public writeFloat64(value: number): this {
    const buffer = new Uint8Array(8);
    new DataView(buffer.buffer).setFloat64(0, value, false);
    this.chunks.push(buffer);
    return this;
  }

  public finish(): Uint8Array {
    const total = this.chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
    const out = new Uint8Array(total);
    let offset = 0;
    for (const chunk of this.chunks) {
      out.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return out;
  }
}

function buildSampleChunk(): Uint8Array {
  const writer = new NbtWriter();
  // Root compound: ""
  writer.writeTag(NBT_TAG_COMPOUND).writeName("");
  writer.writeTag(NBT_TAG_BYTE).writeName("byte").writeInt8(-7);
  writer.writeTag(NBT_TAG_SHORT).writeName("short").writeInt16(-12345);
  writer.writeTag(NBT_TAG_INT).writeName("int").writeInt32(2_000_000_003);
  writer.writeTag(NBT_TAG_LONG).writeName("long").writeInt64(-0x7000_0000_0000_0001n);
  writer.writeTag(NBT_TAG_FLOAT).writeName("float").writeFloat32(1.5);
  writer.writeTag(NBT_TAG_DOUBLE).writeName("double").writeFloat64(Math.PI);
  writer.writeTag(NBT_TAG_STRING).writeName("string").writeInt16(5);
  writer.chunks.push(encodeModifiedUtf8("hello"));
  writer.writeTag(NBT_TAG_INT_ARRAY).writeName("ints").writeInt32(3);
  writer.writeInt32(1).writeInt32(-2).writeInt32(3);
  writer.writeTag(NBT_TAG_LONG_ARRAY).writeName("longs").writeInt32(2);
  writer.writeInt64(1n).writeInt64(-2n);
  // Nested compound
  writer.writeTag(NBT_TAG_COMPOUND).writeName("nested");
  writer.writeTag(NBT_TAG_STRING).writeName("key").writeInt16(5);
  writer.chunks.push(encodeModifiedUtf8("value"));
  writer.writeTag(0); // END of nested
  // List of ints
  writer.writeTag(NBT_TAG_LIST).writeName("list").writeTag(NBT_TAG_INT).writeInt32(3);
  writer.writeInt32(10).writeInt32(20).writeInt32(30);
  writer.writeTag(0); // END of root
  return writer.finish();
}

describe("decodeNbt", () => {
  test("round-trips all tag types", () => {
    const bytes = buildSampleChunk();
    const decoded = decodeNbt(bytes);
    expect(decoded.name).toBe("");
    const value = decoded.value;
    expect(value["byte"]).toBe(-7);
    expect(value["short"]).toBe(-12345);
    expect(value["int"]).toBe(2_000_000_003);
    expect(value["long"]).toBe(-0x7000_0000_0000_0001n);
    expect(value["float"]).toBeCloseTo(1.5, 6);
    expect(value["double"]).toBe(Math.PI);
    expect(value["string"]).toBe("hello");
    expect(Array.from(value["ints"] as Int32Array)).toEqual([1, -2, 3]);
    expect(Array.from(value["longs"] as BigInt64Array)).toEqual([1n, -2n]);
    const nested = value["nested"] as NbtCompound;
    expect(nested["key"]).toBe("value");
    const list = value["list"] as NbtList;
    expect(list.type).toBe(NBT_TAG_INT);
    expect(list.values).toEqual([10, 20, 30]);
  });

  test("rejects non-compound roots", () => {
    const writer = new NbtWriter();
    writer.writeTag(NBT_TAG_INT).writeName("").writeInt32(0);
    expect(() => decodeNbt(writer.finish())).toThrow(/root NBT tag must be TAG_Compound/);
  });

  test("rejects unknown tag ids", () => {
    const writer = new NbtWriter();
    writer.writeTag(NBT_TAG_COMPOUND).writeName("");
    writer.writeTag(99).writeName("bad");
    expect(() => decodeNbt(writer.finish())).toThrow(/unknown NBT tag id 99/);
  });
});
