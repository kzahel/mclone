import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { gunzipSync, inflateSync } from "node:zlib";

import {
  decodeEntityStorageChunk,
  decodeLegacyChunkEntities,
  type DecodedEntityChunk,
} from "../../src/oracle/anvil/entity-chunk.ts";
import { COMPRESSION_GZIP, COMPRESSION_NONE, COMPRESSION_ZLIB, listPresentChunks, RegionFile } from "../../src/oracle/anvil/region.ts";
import {
  buildCreatureGenerationFixture,
  normalizeCreatureEntities,
  type CreatureGenerationFixtureMetadata,
} from "../../src/oracle/integration/creature-fixture.ts";

interface ParsedArgs {
  readonly values: ReadonlyMap<string, string>;
  readonly flags: ReadonlySet<string>;
}

function usage(): never {
  process.stderr.write(
    "usage: dump-creature-fixture.ts --world-dir <dir> --seed <signed-decimal> (--chunks <x,z,x,z,...> | --scan) [--generator <name>] [--generate-structures true|false] [--minecraft-version <id>] [--out <path>]\n",
  );
  process.exit(2);
}

function parseArgs(argv: readonly string[]): ParsedArgs {
  const values = new Map<string, string>();
  const flags = new Set<string>();
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index]!;
    if (!arg.startsWith("--")) {
      process.stderr.write(`expected --flag, got '${arg}'\n`);
      usage();
    }
    const name = arg.slice(2);
    if (name === "scan") {
      flags.add(name);
      continue;
    }
    if (index + 1 >= argv.length) {
      process.stderr.write(`missing value for --${name}\n`);
      usage();
    }
    values.set(name, argv[++index]!);
  }
  return { values, flags };
}

function requireArg(args: ReadonlyMap<string, string>, name: string): string {
  const value = args.get(name);
  if (value === undefined || value === "") {
    process.stderr.write(`missing required --${name}\n`);
    usage();
  }
  return value;
}

function parseChunks(spec: string): Array<{ chunkX: number; chunkZ: number }> {
  const parts = spec.split(",").map((part) => part.trim()).filter((part) => part !== "");
  if (parts.length === 0 || parts.length % 2 !== 0) {
    throw new Error(`--chunks expects an even-length 'x,z,x,z,...' list, got '${spec}'`);
  }
  const out: Array<{ chunkX: number; chunkZ: number }> = [];
  for (let index = 0; index < parts.length; index += 2) {
    const chunkX = Number.parseInt(parts[index]!, 10);
    const chunkZ = Number.parseInt(parts[index + 1]!, 10);
    if (!Number.isFinite(chunkX) || !Number.isFinite(chunkZ)) {
      throw new Error(`non-numeric coord in --chunks at index ${index}`);
    }
    out.push({ chunkX, chunkZ });
  }
  return out;
}

function parseBool(raw: string | undefined, name: string, fallback: boolean): boolean {
  if (raw === undefined) {
    return fallback;
  }
  if (raw === "true") {
    return true;
  }
  if (raw === "false") {
    return false;
  }
  throw new Error(`--${name} must be 'true' or 'false', got '${raw}'`);
}

function decompress(payload: Uint8Array, kind: number): Uint8Array {
  switch (kind) {
    case COMPRESSION_GZIP:
      return new Uint8Array(gunzipSync(payload));
    case COMPRESSION_ZLIB:
      return new Uint8Array(inflateSync(payload));
    case COMPRESSION_NONE:
      return payload;
    default:
      throw new Error(`unsupported chunk compression byte ${kind}`);
  }
}

function regionPath(dir: string, chunkX: number, chunkZ: number): string {
  const regionX = Math.floor(chunkX / 32);
  const regionZ = Math.floor(chunkZ / 32);
  return resolve(dir, `r.${regionX}.${regionZ}.mca`);
}

function readRegion(regionCache: Map<string, RegionFile>, path: string, allowMissingPayload = false): RegionFile | undefined {
  if (!existsSync(path)) {
    return undefined;
  }
  let region = regionCache.get(path);
  if (region === undefined) {
    const bytes = readFileSync(path);
    if (allowMissingPayload && bytes.byteLength < 8192) {
      return undefined;
    }
    region = new RegionFile(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength), decompress);
    regionCache.set(path, region);
  }
  return region;
}

function readEntityChunk(worldDir: string, regionCache: Map<string, RegionFile>, chunkX: number, chunkZ: number): DecodedEntityChunk {
  const entityRegionDir = resolve(worldDir, "entities");
  const blockRegionDir = resolve(worldDir, "region");
  const entityRegion = readRegion(regionCache, regionPath(entityRegionDir, chunkX, chunkZ), true);
  const entityRoot = entityRegion?.readChunkNbt(chunkX, chunkZ);
  if (entityRoot !== undefined) {
    const decoded = decodeEntityStorageChunk(entityRoot.value);
    assertChunkPosition(decoded, chunkX, chunkZ, "entity storage");
    return decoded;
  }

  const blockRegion = readRegion(regionCache, regionPath(blockRegionDir, chunkX, chunkZ));
  const blockRoot = blockRegion?.readChunkNbt(chunkX, chunkZ);
  if (blockRoot === undefined) {
    throw new Error(`chunk (${chunkX}, ${chunkZ}) has no entity storage record and no block chunk fallback in ${worldDir}`);
  }

  const decoded = decodeLegacyChunkEntities(blockRoot.value);
  assertChunkPosition(decoded, chunkX, chunkZ, "legacy chunk");
  return decoded;
}

function assertChunkPosition(decoded: DecodedEntityChunk, chunkX: number, chunkZ: number, source: string): void {
  if (decoded.chunkX !== chunkX || decoded.chunkZ !== chunkZ) {
    throw new Error(
      `${source} coord mismatch for request (${chunkX}, ${chunkZ}): stored (${decoded.chunkX}, ${decoded.chunkZ})`,
    );
  }
}

function scanCreatureChunks(worldDir: string, metadata: CreatureGenerationFixtureMetadata): unknown {
  const entityRegionDir = resolve(worldDir, "entities");
  if (!existsSync(entityRegionDir)) {
    throw new Error(`world has no entity storage directory: ${entityRegionDir}`);
  }

  const chunks: Array<{
    readonly chunkX: number;
    readonly chunkZ: number;
    readonly entityCount: number;
    readonly creatureCount: number;
    readonly types: readonly string[];
  }> = [];
  const regionFiles = readdirSync(entityRegionDir).filter((entry) => /^r\.-?\d+\.-?\d+\.mca$/.test(entry)).sort();
  for (const file of regionFiles) {
    const match = /^r\.(-?\d+)\.(-?\d+)\.mca$/.exec(basename(file));
    if (match === null) {
      continue;
    }
    const regionX = Number.parseInt(match[1]!, 10);
    const regionZ = Number.parseInt(match[2]!, 10);
    const bytes = readFileSync(resolve(entityRegionDir, file));
    if (bytes.byteLength < 8192) {
      continue;
    }
    const region = new RegionFile(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength), decompress);
    for (const { localX, localZ } of listPresentChunks(region)) {
      const chunkX = regionX * 32 + localX;
      const chunkZ = regionZ * 32 + localZ;
      const root = region.readChunkNbt(chunkX, chunkZ);
      if (root === undefined) {
        continue;
      }
      const decoded = decodeEntityStorageChunk(root.value);
      const entities = normalizeCreatureEntities([decoded]);
      const creatureEntities = entities.filter((entity) => entity.category === "creature");
      if (creatureEntities.length === 0) {
        continue;
      }
      chunks.push({
        chunkX,
        chunkZ,
        entityCount: entities.length,
        creatureCount: creatureEntities.length,
        types: [...new Set(creatureEntities.map((entity) => entity.type))].sort(),
      });
    }
  }

  return {
    module: "creature-generation-scan",
    minecraftVersion: metadata.minecraftVersion,
    seed: metadata.seed,
    generator: metadata.generator,
    generateStructures: metadata.generateStructures,
    chunks,
  };
}

function formatFixtureJson(value: unknown): string {
  const pretty = JSON.stringify(value, undefined, 2);
  const compacted = pretty.replace(/\[\s+([^[\]{}]*?)\s+\]/g, (_match, inner: string) => {
    const parts = inner.split(/\s*,\s*/).map((part) => part.trim()).filter((part) => part.length > 0);
    return `[${parts.join(", ")}]`;
  });
  return `${compacted}\n`;
}

function main(): void {
  const parsed = parseArgs(process.argv.slice(2));
  const worldDir = resolve(requireArg(parsed.values, "world-dir"));
  const seed = requireArg(parsed.values, "seed");
  const generator = parsed.values.get("generator") ?? "default";
  const generateStructures = parseBool(parsed.values.get("generate-structures"), "generate-structures", false);
  const minecraftVersion = parsed.values.get("minecraft-version") ?? "1.17.1";
  const outPath = parsed.values.get("out");
  const metadata = { minecraftVersion, seed, generator, generateStructures };

  const hasChunks = parsed.values.has("chunks");
  const scan = parsed.flags.has("scan");
  if (hasChunks === scan) {
    throw new Error("provide exactly one of --chunks or --scan");
  }

  let payload: unknown;
  if (scan) {
    payload = scanCreatureChunks(worldDir, metadata);
  } else {
    const requestedChunks = parseChunks(requireArg(parsed.values, "chunks"));
    const regionCache = new Map<string, RegionFile>();
    const decoded = requestedChunks.map(({ chunkX, chunkZ }) => readEntityChunk(worldDir, regionCache, chunkX, chunkZ));
    payload = buildCreatureGenerationFixture(metadata, decoded);
  }

  const json = formatFixtureJson(payload);
  if (outPath === undefined) {
    process.stdout.write(json);
  } else {
    writeFileSync(resolve(outPath), json);
    process.stderr.write(`[dump-creature-fixture] wrote ${outPath}\n`);
  }
}

try {
  main();
} catch (error) {
  process.stderr.write(`[dump-creature-fixture] ${(error as Error).message}\n`);
  process.exit(1);
}
