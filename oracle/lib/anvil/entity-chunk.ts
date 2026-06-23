import {
  NBT_TAG_COMPOUND,
  NBT_TAG_END,
  type NbtCompound,
  type NbtList,
  type NbtValue,
} from "./nbt.ts";

export type DecodedEntityChunkSource = "entities" | "legacy-chunk";

export interface DecodedEntityChunk {
  readonly dataVersion: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly source: DecodedEntityChunkSource;
  readonly entities: readonly NbtCompound[];
}

export function decodeEntityStorageChunk(root: NbtCompound): DecodedEntityChunk {
  const dataVersion = asInt(root["DataVersion"], "DataVersion");
  const position = asInt32Array(root["Position"], "Position");
  if (position.length < 2) {
    throw new Error(`Position must contain chunk x/z, got ${position.length} entries`);
  }

  return {
    dataVersion,
    chunkX: position[0]!,
    chunkZ: position[1]!,
    source: "entities",
    entities: decodeEntityList(root["Entities"], "Entities"),
  };
}

export function decodeLegacyChunkEntities(root: NbtCompound): DecodedEntityChunk {
  const dataVersion = asInt(root["DataVersion"], "DataVersion");
  const level = asCompound(root["Level"], "Level");
  return {
    dataVersion,
    chunkX: asInt(level["xPos"], "Level.xPos"),
    chunkZ: asInt(level["zPos"], "Level.zPos"),
    source: "legacy-chunk",
    entities: decodeEntityList(level["Entities"], "Level.Entities"),
  };
}

export function emptyDecodedEntityChunk(
  dataVersion: number,
  chunkX: number,
  chunkZ: number,
  source: DecodedEntityChunkSource,
): DecodedEntityChunk {
  return { dataVersion, chunkX, chunkZ, source, entities: [] };
}

function decodeEntityList(value: NbtValue | undefined, path: string): NbtCompound[] {
  if (value === undefined) {
    return [];
  }
  const list = asList(value, path);
  if (list.values.length === 0) {
    return [];
  }
  if (list.type !== NBT_TAG_COMPOUND) {
    throw new Error(`${path} must be a compound list, got element type ${list.type}`);
  }
  return list.values.map((entry, index) => asCompound(entry, `${path}[${index}]`));
}

function asCompound(value: NbtValue | undefined, path: string): NbtCompound {
  if (value === undefined) {
    throw new Error(`missing required compound tag ${path}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`expected compound at ${path}`);
  }
  if (value instanceof Int8Array || value instanceof Int32Array || value instanceof BigInt64Array) {
    throw new Error(`expected compound at ${path}, got array`);
  }
  if ("type" in value && "values" in value) {
    throw new Error(`expected compound at ${path}, got list`);
  }
  return value as NbtCompound;
}

function asList(value: NbtValue, path: string): NbtList {
  if (
    typeof value !== "object" ||
    value === null ||
    value instanceof Int8Array ||
    value instanceof Int32Array ||
    value instanceof BigInt64Array
  ) {
    throw new Error(`expected list at ${path}`);
  }
  if (!("type" in value) || !("values" in value)) {
    throw new Error(`expected list at ${path}`);
  }
  const list = value as NbtList;
  if (list.type === NBT_TAG_END && list.values.length > 0) {
    throw new Error(`${path} has TAG_End element type with ${list.values.length} entries`);
  }
  return list;
}

function asInt(value: NbtValue | undefined, path: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected integer at ${path}, got ${typeof value}`);
  }
  return value | 0;
}

function asInt32Array(value: NbtValue | undefined, path: string): Int32Array {
  if (!(value instanceof Int32Array)) {
    throw new Error(`expected int array at ${path}`);
  }
  return value;
}
