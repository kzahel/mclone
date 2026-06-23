import {
  NBT_TAG_DOUBLE,
  NBT_TAG_FLOAT,
  type NbtCompound,
  type NbtList,
  type NbtValue,
} from "../anvil/nbt.ts";
import type { DecodedEntityChunk, DecodedEntityChunkSource } from "../anvil/entity-chunk.ts";

export type CreatureEntityCategory =
  | "monster"
  | "creature"
  | "ambient"
  | "underground_water_creature"
  | "water_creature"
  | "water_ambient"
  | "misc";

export interface CreatureFixtureChunk {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface NormalizedCreatureEntity {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly type: string;
  readonly category: CreatureEntityCategory;
  readonly pos: readonly [number, number, number];
  readonly rotation: readonly [number, number];
  readonly onGround: boolean;
  readonly age?: number;
  readonly data?: Readonly<Record<string, number | boolean | string>>;
}

export interface CreatureGenerationFixture {
  readonly module: "creature-generation";
  readonly minecraftVersion: string;
  readonly dataVersion: number;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
  readonly source: {
    readonly entityStorage: DecodedEntityChunkSource | "mixed";
    readonly normalization: readonly string[];
  };
  readonly chunks: readonly CreatureFixtureChunk[];
  readonly entities: readonly NormalizedCreatureEntity[];
}

export interface CreatureGenerationFixtureMetadata {
  readonly minecraftVersion: string;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
}

export type CreatureGenerationDiffKind =
  | "metadata_mismatch"
  | "chunk_mismatch"
  | "missing_entity"
  | "extra_entity"
  | "entity_mismatch";

export interface CreatureGenerationDiff {
  readonly kind: CreatureGenerationDiffKind;
  readonly path?: string;
  readonly expected?: unknown;
  readonly actual?: unknown;
}

const NORMALIZATION_NOTES = [
  "uuid omitted",
  "motion omitted until simulated",
  "health, attributes, brain, equipment, leash, passengers, and full NBT omitted",
  "only allowlisted type data compared",
] as const;

const ENTITY_CATEGORY_BY_TYPE: Readonly<Record<string, CreatureEntityCategory>> = {
  "minecraft:bat": "ambient",
  "minecraft:bee": "creature",
  "minecraft:blaze": "monster",
  "minecraft:cat": "creature",
  "minecraft:cave_spider": "monster",
  "minecraft:chicken": "creature",
  "minecraft:cod": "water_ambient",
  "minecraft:cow": "creature",
  "minecraft:creeper": "monster",
  "minecraft:dolphin": "water_creature",
  "minecraft:donkey": "creature",
  "minecraft:drowned": "monster",
  "minecraft:enderman": "monster",
  "minecraft:fox": "creature",
  "minecraft:glow_squid": "underground_water_creature",
  "minecraft:goat": "creature",
  "minecraft:horse": "creature",
  "minecraft:husk": "monster",
  "minecraft:llama": "creature",
  "minecraft:mooshroom": "creature",
  "minecraft:ocelot": "creature",
  "minecraft:panda": "creature",
  "minecraft:parrot": "creature",
  "minecraft:pig": "creature",
  "minecraft:polar_bear": "creature",
  "minecraft:pufferfish": "water_ambient",
  "minecraft:rabbit": "creature",
  "minecraft:salmon": "water_ambient",
  "minecraft:sheep": "creature",
  "minecraft:skeleton": "monster",
  "minecraft:slime": "monster",
  "minecraft:spider": "monster",
  "minecraft:squid": "water_creature",
  "minecraft:stray": "monster",
  "minecraft:tropical_fish": "water_ambient",
  "minecraft:turtle": "creature",
  "minecraft:witch": "monster",
  "minecraft:wolf": "creature",
  "minecraft:zombie": "monster",
  "minecraft:zombie_villager": "monster",
};

export function buildCreatureGenerationFixture(
  metadata: CreatureGenerationFixtureMetadata,
  chunks: readonly DecodedEntityChunk[],
): CreatureGenerationFixture {
  if (chunks.length === 0) {
    throw new Error("buildCreatureGenerationFixture requires at least one entity chunk");
  }

  const ordered = [...chunks].sort((a, b) => (a.chunkX - b.chunkX) || (a.chunkZ - b.chunkZ));
  const dataVersion = ordered[0]!.dataVersion;
  for (const chunk of ordered) {
    if (chunk.dataVersion !== dataVersion) {
      throw new Error(
        `mixed data versions in creature fixture: chunk (${chunk.chunkX}, ${chunk.chunkZ}) has ${chunk.dataVersion}, expected ${dataVersion}`,
      );
    }
  }

  return {
    module: "creature-generation",
    minecraftVersion: metadata.minecraftVersion,
    dataVersion,
    seed: metadata.seed,
    generator: metadata.generator,
    generateStructures: metadata.generateStructures,
    source: {
      entityStorage: entityStorageSource(ordered),
      normalization: NORMALIZATION_NOTES,
    },
    chunks: ordered.map(({ chunkX, chunkZ }) => ({ chunkX, chunkZ })),
    entities: normalizeCreatureEntities(ordered),
  };
}

export function normalizeCreatureEntities(chunks: readonly DecodedEntityChunk[]): NormalizedCreatureEntity[] {
  const out: NormalizedCreatureEntity[] = [];
  for (const chunk of chunks) {
    for (const [index, raw] of chunk.entities.entries()) {
      out.push(normalizeCreatureEntity(raw, chunk.chunkX, chunk.chunkZ, `chunk(${chunk.chunkX},${chunk.chunkZ}).Entities[${index}]`));
    }
  }
  return sortCreatureEntities(out);
}

export function normalizeCreatureEntity(
  raw: NbtCompound,
  chunkX: number,
  chunkZ: number,
  path = "entity",
): NormalizedCreatureEntity {
  const type = asString(raw.id, `${path}.id`);
  const entity: NormalizedCreatureEntity = {
    chunkX,
    chunkZ,
    type,
    category: categoryForEntityType(type),
    pos: numberList(raw.Pos, `${path}.Pos`, NBT_TAG_DOUBLE, 3) as [number, number, number],
    rotation: numberList(raw.Rotation, `${path}.Rotation`, NBT_TAG_FLOAT, 2) as [number, number],
    onGround: asBooleanByte(raw.OnGround, `${path}.OnGround`),
    ...optionalAge(raw.Age, `${path}.Age`),
    ...optionalEntityData(type, raw),
  };
  return entity;
}

export function sortCreatureEntities(entities: readonly NormalizedCreatureEntity[]): NormalizedCreatureEntity[] {
  return [...entities].sort((left, right) =>
    (left.chunkX - right.chunkX)
    || (left.chunkZ - right.chunkZ)
    || left.category.localeCompare(right.category)
    || left.type.localeCompare(right.type)
    || compareNumberTuple(left.pos, right.pos)
    || compareNumberTuple(left.rotation, right.rotation)
    || compareOptionalNumber(left.age, right.age)
    || stableStringify(left.data ?? {}).localeCompare(stableStringify(right.data ?? {}))
  );
}

export function compareCreatureGenerationFixtures(
  expected: CreatureGenerationFixture,
  actual: CreatureGenerationFixture,
): CreatureGenerationDiff[] {
  const diffs: CreatureGenerationDiff[] = [];
  pushMismatch(diffs, "minecraftVersion", expected.minecraftVersion, actual.minecraftVersion);
  pushMismatch(diffs, "dataVersion", expected.dataVersion, actual.dataVersion);
  pushMismatch(diffs, "seed", expected.seed, actual.seed);
  pushMismatch(diffs, "generator", expected.generator, actual.generator);
  pushMismatch(diffs, "generateStructures", expected.generateStructures, actual.generateStructures);

  if (stableStringify(expected.chunks) !== stableStringify(actual.chunks)) {
    diffs.push({ kind: "chunk_mismatch", path: "chunks", expected: expected.chunks, actual: actual.chunks });
  }
  if (diffs.length > 0) {
    return diffs;
  }

  const expectedEntities = sortCreatureEntities(expected.entities);
  const actualEntities = sortCreatureEntities(actual.entities);
  const max = Math.max(expectedEntities.length, actualEntities.length);
  for (let index = 0; index < max; index++) {
    const expectedEntity = expectedEntities[index];
    const actualEntity = actualEntities[index];
    if (stableStringify(expectedEntity) === stableStringify(actualEntity)) {
      continue;
    }
    if (expectedEntity === undefined) {
      diffs.push({ kind: "extra_entity", path: `entities[${index}]`, actual: actualEntity });
    } else if (actualEntity === undefined) {
      diffs.push({ kind: "missing_entity", path: `entities[${index}]`, expected: expectedEntity });
    } else {
      diffs.push({ kind: "entity_mismatch", path: `entities[${index}]`, expected: expectedEntity, actual: actualEntity });
    }
    break;
  }
  return diffs;
}

export function categoryForEntityType(type: string): CreatureEntityCategory {
  return ENTITY_CATEGORY_BY_TYPE[type] ?? "misc";
}

function entityStorageSource(chunks: readonly DecodedEntityChunk[]): DecodedEntityChunkSource | "mixed" {
  const first = chunks[0]!.source;
  return chunks.every((chunk) => chunk.source === first) ? first : "mixed";
}

function optionalAge(value: NbtValue | undefined, path: string): { readonly age?: number } {
  if (value === undefined) {
    return {};
  }
  return { age: asInt(value, path) };
}

function optionalEntityData(
  type: string,
  raw: NbtCompound,
): { readonly data?: Readonly<Record<string, number | boolean | string>> } {
  const data: Record<string, number | boolean | string> = {};
  if (type === "minecraft:sheep" && raw.Color !== undefined) {
    data.Color = asInt(raw.Color, "entity.Color");
  }
  if (Object.keys(data).length === 0) {
    return {};
  }
  return { data };
}

function pushMismatch(
  diffs: CreatureGenerationDiff[],
  path: string,
  expected: unknown,
  actual: unknown,
): void {
  if (stableStringify(expected) !== stableStringify(actual)) {
    diffs.push({ kind: "metadata_mismatch", path, expected, actual });
  }
}

function compareNumberTuple(left: readonly number[], right: readonly number[]): number {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index++) {
    const diff = left[index]! - right[index]!;
    if (diff !== 0) {
      return diff;
    }
  }
  return left.length - right.length;
}

function compareOptionalNumber(left: number | undefined, right: number | undefined): number {
  if (left === right) {
    return 0;
  }
  if (left === undefined) {
    return -1;
  }
  if (right === undefined) {
    return 1;
  }
  return left - right;
}

function numberList(value: NbtValue | undefined, path: string, tag: number, length: number): number[] {
  const list = asList(value, path);
  if (list.type !== tag) {
    throw new Error(`${path} must have element type ${tag}, got ${list.type}`);
  }
  if (list.values.length !== length) {
    throw new Error(`${path} must contain ${length} entries, got ${list.values.length}`);
  }
  return list.values.map((entry, index) => asNumber(entry, `${path}[${index}]`));
}

function asList(value: NbtValue | undefined, path: string): NbtList {
  if (value === undefined) {
    throw new Error(`missing required list tag ${path}`);
  }
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

function asString(value: NbtValue | undefined, path: string): string {
  if (typeof value !== "string") {
    throw new Error(`expected string at ${path}, got ${typeof value}`);
  }
  return value;
}

function asBooleanByte(value: NbtValue | undefined, path: string): boolean {
  if (typeof value !== "number") {
    throw new Error(`expected boolean byte at ${path}, got ${typeof value}`);
  }
  return value !== 0;
}

function asInt(value: NbtValue, path: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected integer at ${path}, got ${typeof value}`);
  }
  return value | 0;
}

function asNumber(value: NbtValue, path: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected number at ${path}, got ${typeof value}`);
  }
  return value;
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(",")}]`;
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(([left], [right]) => left.localeCompare(right));
  return `{${entries.map(([key, entry]) => `${JSON.stringify(key)}:${stableStringify(entry)}`).join(",")}}`;
}
