import type { NbtCompound, NbtList, NbtValue } from "./nbt.ts";
import { paletteBitsFor, unpackBitStorage as unpackRuntimeBitStorage } from "../util/bit-storage.ts";

export { paletteBitsFor };

export interface DecodedPaletteEntry {
  readonly name: string;
  readonly properties?: Readonly<Record<string, string>>;
}

export interface DecodedSection {
  readonly y: number;
  readonly palette: readonly DecodedPaletteEntry[];
  readonly blocks: readonly number[];
}

export interface DecodedLightSection {
  readonly y: number;
  readonly data: Uint8Array;
}

export interface DecodedChunkLight {
  readonly block: readonly DecodedLightSection[];
  readonly sky: readonly DecodedLightSection[];
}

export interface DecodedHeightmaps {
  readonly [name: string]: readonly number[];
}

export interface DecodedChunk {
  readonly dataVersion: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: string;
  readonly isLightOn: boolean;
  readonly sections: readonly DecodedSection[];
  readonly light: DecodedChunkLight;
  readonly heightmaps: DecodedHeightmaps;
  readonly biomes: readonly number[];
}

const BLOCKS_PER_SECTION = 16 * 16 * 16;
const LIGHT_DATALAYER_BYTES = 2048;
const HEIGHTMAP_BITS = 9;
const HEIGHTMAP_ENTRIES = 256;
const BIOMES_PER_CHUNK = 4 * 64 * 4;

export function decodeChunk(root: NbtCompound): DecodedChunk {
  const dataVersion = asInt(root["DataVersion"], "DataVersion");
  const level = asCompound(root["Level"], "Level");
  const chunkX = asInt(level["xPos"], "Level.xPos");
  const chunkZ = asInt(level["zPos"], "Level.zPos");
  const status = asString(level["Status"], "Level.Status");
  const isLightOn = asBooleanByte(level["isLightOn"], "Level.isLightOn", false);

  const biomesValue = level["Biomes"];
  const biomes = biomesValue === undefined ? [] : asInt32Array(biomesValue, "Level.Biomes");
  if (biomes.length !== 0 && biomes.length !== BIOMES_PER_CHUNK) {
    throw new Error(`Level.Biomes must contain ${BIOMES_PER_CHUNK} entries, got ${biomes.length}`);
  }

  const decodedSections = decodeSections(level["Sections"]);
  const heightmaps = decodeHeightmaps(level["Heightmaps"]);

  return {
    dataVersion,
    chunkX,
    chunkZ,
    status,
    isLightOn,
    sections: decodedSections.sections,
    light: decodedSections.light,
    heightmaps,
    biomes: Array.from(biomes),
  };
}

interface DecodedSectionPayload {
  readonly sections: DecodedSection[];
  readonly light: DecodedChunkLight;
}

function decodeSections(raw: NbtValue | undefined): DecodedSectionPayload {
  if (raw === undefined) {
    return { sections: [], light: { block: [], sky: [] } };
  }
  const list = asList(raw, "Level.Sections");
  const sections: DecodedSection[] = [];
  const blockLight: DecodedLightSection[] = [];
  const skyLight: DecodedLightSection[] = [];
  for (const [index, entry] of list.values.entries()) {
    const section = asCompound(entry, `Level.Sections[${index}]`);
    const decoded = decodeSection(section);
    if (decoded !== undefined) {
      sections.push(decoded);
    }
    const light = decodeSectionLight(section);
    blockLight.push(...light.block);
    skyLight.push(...light.sky);
  }
  sections.sort((a, b) => a.y - b.y);
  blockLight.sort((a, b) => a.y - b.y);
  skyLight.sort((a, b) => a.y - b.y);
  return { sections, light: { block: blockLight, sky: skyLight } };
}

function decodeSection(section: NbtCompound): DecodedSection | undefined {
  const y = asInt(section["Y"], "Sections[].Y");
  const paletteValue = section["Palette"];
  const blockStatesValue = section["BlockStates"];
  if (paletteValue === undefined) {
    return undefined;
  }

  const paletteList = asList(paletteValue, "Sections[].Palette");
  const palette = paletteList.values.map((entry, index) => decodePaletteEntry(entry, index));

  let blocks: number[];
  if (blockStatesValue === undefined) {
    blocks = new Array<number>(BLOCKS_PER_SECTION).fill(0);
  } else {
    const packed = asInt64Array(blockStatesValue, "Sections[].BlockStates");
    const bits = Math.max(4, paletteBitsFor(palette.length));
    blocks = unpackBitStorage(packed, bits, BLOCKS_PER_SECTION);
  }

  if (blocks.length !== BLOCKS_PER_SECTION) {
    throw new Error(`section Y=${y} decoded ${blocks.length} blocks, expected ${BLOCKS_PER_SECTION}`);
  }
  for (const index of blocks) {
    if (index < 0 || index >= palette.length) {
      throw new Error(`section Y=${y} palette index ${index} out of range [0, ${palette.length})`);
    }
  }

  return { y, palette, blocks };
}

function decodeSectionLight(section: NbtCompound): DecodedChunkLight {
  const y = asInt(section["Y"], "Sections[].Y");
  const block = decodeLightLayer(section, "BlockLight", y);
  const sky = decodeLightLayer(section, "SkyLight", y);
  return {
    block: block === undefined ? [] : [block],
    sky: sky === undefined ? [] : [sky],
  };
}

function decodeLightLayer(section: NbtCompound, tagName: "BlockLight" | "SkyLight", y: number): DecodedLightSection | undefined {
  const value = section[tagName];
  if (value === undefined) {
    return undefined;
  }
  const raw = asByteArray(value, `Sections[Y=${y}].${tagName}`);
  if (raw.length !== LIGHT_DATALAYER_BYTES) {
    throw new Error(`Sections[Y=${y}].${tagName} must contain ${LIGHT_DATALAYER_BYTES} bytes, got ${raw.length}`);
  }
  return {
    y,
    data: new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength),
  };
}

function decodePaletteEntry(entry: NbtValue, index: number): DecodedPaletteEntry {
  const compound = asCompound(entry, `Palette[${index}]`);
  const name = asString(compound["Name"], `Palette[${index}].Name`);
  const propertiesValue = compound["Properties"];
  if (propertiesValue === undefined) {
    return { name };
  }
  const raw = asCompound(propertiesValue, `Palette[${index}].Properties`);
  const properties: Record<string, string> = {};
  for (const key of Object.keys(raw).sort()) {
    properties[key] = asString(raw[key], `Palette[${index}].Properties.${key}`);
  }
  return { name, properties };
}

function decodeHeightmaps(raw: NbtValue | undefined): DecodedHeightmaps {
  if (raw === undefined) {
    return {};
  }
  const compound = asCompound(raw, "Level.Heightmaps");
  const out: Record<string, number[]> = {};
  for (const key of Object.keys(compound).sort()) {
    const raw = compound[key];
    if (raw === undefined) {
      continue;
    }
    const packed = asInt64Array(raw, `Level.Heightmaps.${key}`);
    out[key] = unpackBitStorage(packed, HEIGHTMAP_BITS, HEIGHTMAP_ENTRIES);
  }
  return out;
}

export function unpackBitStorage(packed: readonly bigint[] | BigInt64Array, bits: number, entries: number): number[] {
  return unpackRuntimeBitStorage(packed, bits, entries);
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
  return value as NbtList;
}

function asInt(value: NbtValue | undefined, path: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected integer at ${path}, got ${typeof value}`);
  }
  return value | 0;
}

function asString(value: NbtValue | undefined, path: string): string {
  if (typeof value !== "string") {
    throw new Error(`expected string at ${path}, got ${typeof value}`);
  }
  return value;
}

function asBooleanByte(value: NbtValue | undefined, path: string, fallback: boolean): boolean {
  if (value === undefined) {
    return fallback;
  }
  if (typeof value !== "number") {
    throw new Error(`expected boolean byte at ${path}, got ${typeof value}`);
  }
  return value !== 0;
}

function asInt32Array(value: NbtValue, path: string): Int32Array {
  if (!(value instanceof Int32Array)) {
    throw new Error(`expected int array at ${path}`);
  }
  return value;
}

function asInt64Array(value: NbtValue, path: string): BigInt64Array {
  if (!(value instanceof BigInt64Array)) {
    throw new Error(`expected long array at ${path}`);
  }
  return value;
}

function asByteArray(value: NbtValue, path: string): Int8Array {
  if (!(value instanceof Int8Array)) {
    throw new Error(`expected byte array at ${path}`);
  }
  return value;
}
