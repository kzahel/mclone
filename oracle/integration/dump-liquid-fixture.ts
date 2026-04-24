import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { gunzipSync, inflateSync } from "node:zlib";

import { decodeChunk, type DecodedChunk } from "../../src/oracle/anvil/chunk.ts";
import { COMPRESSION_GZIP, COMPRESSION_NONE, COMPRESSION_ZLIB, RegionFile } from "../../src/oracle/anvil/region.ts";
import {
  buildLiquidRegionFixture,
  decodeChunkLiquidTicks,
  type ChunkLiquidTickInput,
} from "../../src/oracle/integration/liquid-fixture.ts";
import { boundsMaxX, boundsMaxZ, readLiquidScenario } from "../../src/oracle/integration/liquid-scenario.ts";

function usage(): never {
  process.stderr.write(
    "usage: dump-liquid-fixture.ts --region-dir <dir> --scenario <path> [--out <path>]\n",
  );
  process.exit(2);
}

function parseArgs(argv: readonly string[]): Map<string, string> {
  const out = new Map<string, string>();
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index]!;
    if (!arg.startsWith("--")) {
      process.stderr.write(`expected --flag, got '${arg}'\n`);
      usage();
    }
    const name = arg.slice(2);
    if (index + 1 >= argv.length) {
      process.stderr.write(`missing value for --${name}\n`);
      usage();
    }
    out.set(name, argv[++index]!);
  }
  return out;
}

function requireArg(args: Map<string, string>, name: string): string {
  const value = args.get(name);
  if (value === undefined || value === "") {
    process.stderr.write(`missing required --${name}\n`);
    usage();
  }
  return value;
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

function fixtureChunkCoords(minX: number, maxX: number, minZ: number, maxZ: number): Array<{ chunkX: number; chunkZ: number }> {
  const minChunkX = Math.floor(minX / 16);
  const maxChunkX = Math.floor(maxX / 16);
  const minChunkZ = Math.floor(minZ / 16);
  const maxChunkZ = Math.floor(maxZ / 16);
  const out: Array<{ chunkX: number; chunkZ: number }> = [];
  for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
    for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
      out.push({ chunkX, chunkZ });
    }
  }
  return out;
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
  const args = parseArgs(process.argv.slice(2));
  const regionDir = resolve(requireArg(args, "region-dir"));
  const scenario = readLiquidScenario(resolve(requireArg(args, "scenario")));
  const outPath = args.get("out");
  const chunks = fixtureChunkCoords(
    scenario.bounds.minX - scenario.tickMargin,
    boundsMaxX(scenario.bounds) + scenario.tickMargin,
    scenario.bounds.minZ - scenario.tickMargin,
    boundsMaxZ(scenario.bounds) + scenario.tickMargin,
  );
  const regionCache = new Map<string, RegionFile>();
  const decoded: DecodedChunk[] = [];
  const liquidTicks: ChunkLiquidTickInput[] = [];

  for (const { chunkX, chunkZ } of chunks) {
    const path = regionPath(regionDir, chunkX, chunkZ);
    let region = regionCache.get(path);
    if (region === undefined) {
      const bytes = readFileSync(path);
      region = new RegionFile(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength), decompress);
      regionCache.set(path, region);
    }
    const root = region.readChunkNbt(chunkX, chunkZ);
    if (root === undefined) {
      throw new Error(`chunk (${chunkX}, ${chunkZ}) not present in ${path}`);
    }
    const chunk = decodeChunk(root.value);
    if (chunk.chunkX !== chunkX || chunk.chunkZ !== chunkZ) {
      throw new Error(`chunk coord mismatch for request (${chunkX}, ${chunkZ}): region stored (${chunk.chunkX}, ${chunk.chunkZ})`);
    }
    decoded.push(chunk);
    liquidTicks.push({ chunkX, chunkZ, ticks: decodeChunkLiquidTicks(root.value) });
  }

  const fixture = buildLiquidRegionFixture(scenario, decoded, liquidTicks);
  const json = formatFixtureJson(fixture);
  if (outPath === undefined) {
    process.stdout.write(json);
  } else {
    writeFileSync(resolve(outPath), json);
    process.stderr.write(`[dump-liquid-fixture] wrote ${outPath}\n`);
  }
}

try {
  main();
} catch (error) {
  process.stderr.write(`[dump-liquid-fixture] ${(error as Error).message}\n`);
  process.exit(1);
}
