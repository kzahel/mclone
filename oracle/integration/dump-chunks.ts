import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { gunzipSync, inflateSync } from "node:zlib";

import { decodeChunk } from "../lib/anvil/chunk.ts";
import { COMPRESSION_GZIP, COMPRESSION_NONE, COMPRESSION_ZLIB, RegionFile } from "../lib/anvil/region.ts";
import { buildChunkFixture } from "../lib/integration/chunk-fixture.ts";

function usage(): never {
  process.stderr.write(
    "usage: dump-chunks.ts --region-dir <dir> --seed <signed-decimal> --chunks <x,z,x,z,...> [--generator <name>] [--generate-structures true|false] [--minecraft-version <id>] [--out <path>]\n",
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

function formatFixtureJson(value: unknown): string {
  const pretty = JSON.stringify(value, undefined, 2);
  // Collapse arrays of primitive numbers or strings onto a single line. The
  // structural JSON stays pretty-printed at 2 spaces, but the giant palette
  // index + biome arrays and heightmap rows compact down massively.
  const compacted = pretty.replace(/\[\s+([^[\]{}]*?)\s+\]/g, (_match, inner: string) => {
    const parts = inner.split(/\s*,\s*/).map((part) => part.trim()).filter((part) => part.length > 0);
    return `[${parts.join(", ")}]`;
  });
  return `${compacted}\n`;
}

function main(): void {
  const args = parseArgs(process.argv.slice(2));
  const regionDir = resolve(requireArg(args, "region-dir"));
  const seed = requireArg(args, "seed");
  const chunks = parseChunks(requireArg(args, "chunks"));
  const generator = args.get("generator") ?? "default";
  const generateStructures = parseBool(args.get("generate-structures"), "generate-structures", false);
  const minecraftVersion = args.get("minecraft-version") ?? "1.17.1";
  const outPath = args.get("out");

  const regionCache = new Map<string, RegionFile>();

  const decoded = chunks.map(({ chunkX, chunkZ }) => {
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
    return decodeChunk(root.value);
  });

  for (const [index, chunk] of decoded.entries()) {
    const requested = chunks[index]!;
    if (chunk.chunkX !== requested.chunkX || chunk.chunkZ !== requested.chunkZ) {
      throw new Error(
        `chunk coord mismatch for request (${requested.chunkX}, ${requested.chunkZ}): region stored (${chunk.chunkX}, ${chunk.chunkZ})`,
      );
    }
  }

  const fixture = buildChunkFixture(
    { minecraftVersion, seed, generator, generateStructures },
    decoded,
  );

  const json = formatFixtureJson(fixture);
  if (outPath === undefined) {
    process.stdout.write(json);
  } else {
    writeFileSync(resolve(outPath), json);
    process.stderr.write(`[dump-chunks] wrote ${outPath}\n`);
  }
}

try {
  main();
} catch (error) {
  process.stderr.write(`[dump-chunks] ${(error as Error).message}\n`);
  process.exit(1);
}
