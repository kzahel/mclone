export const NBT_TAG_END = 0;
export const NBT_TAG_BYTE = 1;
export const NBT_TAG_SHORT = 2;
export const NBT_TAG_INT = 3;
export const NBT_TAG_LONG = 4;
export const NBT_TAG_FLOAT = 5;
export const NBT_TAG_DOUBLE = 6;
export const NBT_TAG_BYTE_ARRAY = 7;
export const NBT_TAG_STRING = 8;
export const NBT_TAG_LIST = 9;
export const NBT_TAG_COMPOUND = 10;
export const NBT_TAG_INT_ARRAY = 11;
export const NBT_TAG_LONG_ARRAY = 12;

export type NbtTagId =
  | typeof NBT_TAG_END
  | typeof NBT_TAG_BYTE
  | typeof NBT_TAG_SHORT
  | typeof NBT_TAG_INT
  | typeof NBT_TAG_LONG
  | typeof NBT_TAG_FLOAT
  | typeof NBT_TAG_DOUBLE
  | typeof NBT_TAG_BYTE_ARRAY
  | typeof NBT_TAG_STRING
  | typeof NBT_TAG_LIST
  | typeof NBT_TAG_COMPOUND
  | typeof NBT_TAG_INT_ARRAY
  | typeof NBT_TAG_LONG_ARRAY;

export interface NbtList {
  readonly type: NbtTagId;
  readonly values: readonly NbtValue[];
}

export type NbtCompound = { readonly [key: string]: NbtValue };

export type NbtValue =
  | number
  | bigint
  | string
  | Int8Array
  | Int32Array
  | BigInt64Array
  | NbtList
  | NbtCompound;

export interface NamedNbt {
  readonly name: string;
  readonly value: NbtCompound;
}

class NbtReader {
  private offset = 0;
  private readonly view: DataView;

  public constructor(view: DataView) {
    this.view = view;
  }

  public readTagId(): NbtTagId {
    const tag = this.view.getUint8(this.offset);
    this.offset += 1;
    if (tag > NBT_TAG_LONG_ARRAY) {
      throw new Error(`unknown NBT tag id ${tag}`);
    }
    return tag as NbtTagId;
  }

  public readString(): string {
    const length = this.view.getUint16(this.offset, false);
    this.offset += 2;
    const bytes = new Uint8Array(this.view.buffer, this.view.byteOffset + this.offset, length);
    this.offset += length;
    return decodeModifiedUtf8(bytes);
  }

  public readValue(tag: NbtTagId): NbtValue {
    switch (tag) {
      case NBT_TAG_END:
        throw new Error("cannot read value for TAG_End");
      case NBT_TAG_BYTE: {
        const value = this.view.getInt8(this.offset);
        this.offset += 1;
        return value;
      }
      case NBT_TAG_SHORT: {
        const value = this.view.getInt16(this.offset, false);
        this.offset += 2;
        return value;
      }
      case NBT_TAG_INT: {
        const value = this.view.getInt32(this.offset, false);
        this.offset += 4;
        return value;
      }
      case NBT_TAG_LONG: {
        const value = this.view.getBigInt64(this.offset, false);
        this.offset += 8;
        return value;
      }
      case NBT_TAG_FLOAT: {
        const value = this.view.getFloat32(this.offset, false);
        this.offset += 4;
        return value;
      }
      case NBT_TAG_DOUBLE: {
        const value = this.view.getFloat64(this.offset, false);
        this.offset += 8;
        return value;
      }
      case NBT_TAG_BYTE_ARRAY: {
        const length = this.view.getInt32(this.offset, false);
        this.offset += 4;
        const out = new Int8Array(length);
        for (let index = 0; index < length; index++) {
          out[index] = this.view.getInt8(this.offset + index);
        }
        this.offset += length;
        return out;
      }
      case NBT_TAG_STRING:
        return this.readString();
      case NBT_TAG_LIST:
        return this.readList();
      case NBT_TAG_COMPOUND:
        return this.readCompoundBody();
      case NBT_TAG_INT_ARRAY: {
        const length = this.view.getInt32(this.offset, false);
        this.offset += 4;
        const out = new Int32Array(length);
        for (let index = 0; index < length; index++) {
          out[index] = this.view.getInt32(this.offset + (index * 4), false);
        }
        this.offset += length * 4;
        return out;
      }
      case NBT_TAG_LONG_ARRAY: {
        const length = this.view.getInt32(this.offset, false);
        this.offset += 4;
        const out = new BigInt64Array(length);
        for (let index = 0; index < length; index++) {
          out[index] = this.view.getBigInt64(this.offset + (index * 8), false);
        }
        this.offset += length * 8;
        return out;
      }
      default:
        throw new Error(`unhandled NBT tag id ${tag satisfies never}`);
    }
  }

  private readList(): NbtList {
    const elementTag = this.readTagId();
    const length = this.view.getInt32(this.offset, false);
    this.offset += 4;
    if (length <= 0) {
      return { type: elementTag, values: [] };
    }
    if (elementTag === NBT_TAG_END) {
      throw new Error("TAG_List with TAG_End element type and non-zero length");
    }
    const values: NbtValue[] = [];
    for (let index = 0; index < length; index++) {
      values.push(this.readValue(elementTag));
    }
    return { type: elementTag, values };
  }

  private readCompoundBody(): NbtCompound {
    const out: Record<string, NbtValue> = {};
    while (true) {
      const tag = this.readTagId();
      if (tag === NBT_TAG_END) {
        return out;
      }
      const name = this.readString();
      out[name] = this.readValue(tag);
    }
  }

  public bytesConsumed(): number {
    return this.offset;
  }
}

export function decodeNbt(bytes: Uint8Array): NamedNbt {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const reader = new NbtReader(view);
  const rootTag = reader.readTagId();
  if (rootTag !== NBT_TAG_COMPOUND) {
    throw new Error(`root NBT tag must be TAG_Compound, got ${rootTag}`);
  }
  const name = reader.readString();
  const value = reader.readValue(NBT_TAG_COMPOUND) as NbtCompound;
  return { name, value };
}

function decodeModifiedUtf8(bytes: Uint8Array): string {
  // Java's Modified UTF-8: 1-3 byte sequences; NULL encoded as 0xC0 0x80;
  // supplementary chars encoded as two 3-byte surrogate halves. For oracle
  // fixtures we only care about the ASCII identifier subset MC uses for keys
  // and block/biome resource locations, which stays identical to UTF-8.
  const out: number[] = [];
  let index = 0;
  while (index < bytes.length) {
    const byte = bytes[index]!;
    if (byte < 0x80) {
      out.push(byte);
      index += 1;
      continue;
    }
    if ((byte & 0xE0) === 0xC0) {
      if (index + 1 >= bytes.length) {
        throw new Error("truncated modified UTF-8 2-byte sequence");
      }
      const lo = bytes[index + 1]!;
      if ((lo & 0xC0) !== 0x80) {
        throw new Error("invalid continuation byte in 2-byte modified UTF-8");
      }
      out.push(((byte & 0x1F) << 6) | (lo & 0x3F));
      index += 2;
      continue;
    }
    if ((byte & 0xF0) === 0xE0) {
      if (index + 2 >= bytes.length) {
        throw new Error("truncated modified UTF-8 3-byte sequence");
      }
      const b1 = bytes[index + 1]!;
      const b2 = bytes[index + 2]!;
      if ((b1 & 0xC0) !== 0x80 || (b2 & 0xC0) !== 0x80) {
        throw new Error("invalid continuation byte in 3-byte modified UTF-8");
      }
      out.push(((byte & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F));
      index += 3;
      continue;
    }
    throw new Error(`invalid modified UTF-8 lead byte 0x${byte.toString(16)}`);
  }
  return String.fromCharCode(...out);
}

export function encodeModifiedUtf8(value: string): Uint8Array {
  const out: number[] = [];
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code > 0 && code < 0x80) {
      out.push(code);
    } else if (code < 0x800) {
      out.push(0xC0 | (code >> 6));
      out.push(0x80 | (code & 0x3F));
    } else {
      out.push(0xE0 | (code >> 12));
      out.push(0x80 | ((code >> 6) & 0x3F));
      out.push(0x80 | (code & 0x3F));
    }
  }
  return new Uint8Array(out);
}
