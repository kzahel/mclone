# 04 — Integration oracle harness

Continuing from [`03-noise-sampler-settings.md`](03-noise-sampler-settings.md). This slice is **infrastructure only**: stand up a repeatable server-side worldgen oracle before we need full-chunk diffs against `NoiseBasedChunkGenerator` in tactical `06`. It deliberately does **not** begin translating the generator itself — see [`AGENTS.md`](../../AGENTS.md).

## Goal

A second oracle tier that captures *real* Minecraft 1.17.1 server-generated chunks as committable JSON fixtures, shaped for the chunk-parity checks we will need once the terrain pipeline starts producing voxels on the TS side.

Three pieces, all additive to the existing unit oracle:

- a pinned-seed **server-jar runner** that generates a small headless overworld region
- an **Anvil/MCA reader** in TypeScript capable of parsing the `region/*.mca` files 1.17.1 produces
- a deterministic **chunk-level fixture format** plus dumper that turns decoded chunks into committable JSON

The existing unit oracle (`pnpm --silent oracle:gen prng`, `pnpm --silent oracle:gen noise ...`) stays untouched.

## Current status

- Tactical `00`, `01`, `02`, and `03` are complete
- Unit oracle (`oracle/java/OracleDumper.java`, `oracle/build.sh`, `oracle/run.sh`) lives under `oracle/` and is wired through `pnpm --silent oracle:gen ...`
- No server jar, generated world, or Anvil reader currently lives in the repo
- This doc is the first slice that touches the server side of the oracle pipeline — the unit oracle stays client-deobf-based

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | server-jar runner | Java 17, pinned seed, committed `server.properties` template | seed `S` + chunk-range → `world/region/*.mca` on disk |
| 2 | Anvil region reader | Node `zlib`, pure TS | `.mca` bytes + `(chunkX,chunkZ)` → decoded NBT compound |
| 3 | NBT decoder | pure TS | NBT byte buffer → typed tree (`byte`/`short`/`int`/`long`/`float`/`double`/`byteArray`/`string`/`list`/`compound`/`intArray`/`longArray`) |
| 4 | chunk fixture extractor | modules 2 + 3 | decoded chunk NBT → canonical fixture (`sections[] { palette, blocks }`, `heightmaps`, `biomes`, `status`, metadata) |
| 5 | end-to-end CLI | modules 1 + 4 | `--seed S --chunks x,z,...` → committed `test/fixtures/integration/overworld-seed-S.json` |

## Why this subset

For default 1.17.1 overworld parity, chunk diffs against a real server only need three things per chunk:

- **blocks**: the voxel-level terrain output of `NoiseBasedChunkGenerator` + surface rules + carvers. Captured as a per-section `(palette, blocks)` tuple where `blocks` is the decoded 4096-entry palette-index array in standard YZX order.
- **heightmaps**: decoded `WORLD_SURFACE` and `OCEAN_FLOOR` 16×16 arrays — cheap to include, cheap to diff, and exactly the kind of cross-check that catches off-by-one terrain bugs before voxel-level diffs even start running.
- **biomes**: the raw 1024-entry biome grid 1.17.1 stores per chunk (4×64×4 cells). Needed once tactical `05` lands a real biome source.

The **dormant C&C Part 1 systems** (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, active `Aquifer`, `NoiseUtils`) are still out of scope — fixtures are generated from the default `NoiseGeneratorSettings.overworld(...)` preset the server ships with, which pins all five C&C Part 1 booleans to `false` (see [`AGENTS.md`](../../AGENTS.md)).

## What lives where

```
scripts/
  fetch-server-jar.sh             # idempotent server.jar download (SHA1-verified)
oracle/
  integration/
    README.md                     # how to regenerate a fixture
    gen-fixture.sh                # single entry point: server + dump
    run-server.sh                 # headless server, pinned seed, waits for spawn gen
    dump-chunks.mjs               # CLI: region reader → fixture JSON
    server/
      server.properties.template  # pinned overworld config (seed substituted)
      eula.txt                    # EULA acceptance — oracle-only, never ships
src/
  oracle/
    anvil/
      nbt.ts                      # tag-type-aware NBT decoder
      region.ts                   # region header + chunk payload decompression
      chunk.ts                    # extract palette, heightmaps, biomes
    integration/
      chunk-fixture.ts            # typed fixture shape + builder
test/
  oracle/
    nbt.test.ts                   # tag roundtrips, common encodings
    region.test.ts                # synthetic region header, gzip/zlib payload
    chunk-fixture.test.ts         # packed block-states + heightmap decode vectors
  fixtures/
    integration/
      overworld-seed-<seed>-chunks-<range>.json
```

The reader + fixture extractor are written in TypeScript under `src/oracle/` so they get Vitest coverage and stay consumable from future harness code. The CLI under `oracle/integration/dump-chunks.mjs` is plain ESM JavaScript so it runs on `node >= 20` without adding a TS runtime dependency; it vendors the small amount of parsing logic it needs and the TS copy is the canonical reference.

## Server-runner design

Pinned settings:

- `level-seed=<seed>` (CLI flag, signed decimal, stringified at source)
- `level-type=default`
- `generate-structures=false` (structures are post-MVP, and removing them keeps fixtures smaller + less timing-sensitive)
- `spawn-protection=0`, `view-distance=10`, `online-mode=false`, `allow-nether=false`, `gamemode=creative`, `difficulty=peaceful`, `motd=mclone-oracle`
- fixed `level-name=world` so the region directory is predictable

The runner:

1. Writes `server.properties` from the template into an isolated working directory under `reference/minecraft-1.17.1/server-work-<seed>/`
2. Writes `eula.txt` with `eula=true`
3. Launches `java -Xmx2G -jar server.jar --nogui`
4. Streams stdout and watches for the `Done (` startup line
5. Sends `stop` on stdin once spawn chunks are generated
6. Exits non-zero if the server never reaches `Done` within a bounded timeout

The resulting world lives at `server-work-<seed>/world/`. The spawn chunk radius the server generates on startup covers `[-11..11]` in chunk X/Z — more than enough for the initial fixtures, which target a small band around `(0, 0)`.

## Fixture shape

```jsonc
{
  "module": "integration",
  "minecraftVersion": "1.17.1",
  "dataVersion": 2730,
  "seed": "12345",
  "generator": "default",
  "generateStructures": false,
  "chunks": [
    {
      "chunkX": 0,
      "chunkZ": 0,
      "status": "full",
      "sections": [
        {
          "y": 0,
          "palette": ["minecraft:air", "minecraft:stone", "minecraft:bedrock", "..."],
          "blockOrder": "y-major,z-major,x-minor",
          "blocks": [/* 4096 palette indices */]
        }
      ],
      "heightmaps": {
        "WORLD_SURFACE": [/* 256 ints, row-major (z-major,x-minor) */],
        "OCEAN_FLOOR":   [/* 256 ints */]
      },
      "biomes": [/* 1024 ints, MC's native 4×64×4 order */]
    }
  ]
}
```

Notes:

- `sections[].y` is the signed section index exactly as stored in NBT (`byte`)
- `sections[].palette` is ordered by palette index, mirroring the `Palette` NBT list; each entry is the block's resource key (block-state properties are dropped for the 1.17.1 MVP since we're only comparing stone/air/water/bedrock at tactical `06`)
- `sections[].blocks` is decoded from the NBT `BlockStates` long array using 1.17.1 `BitStorage` rules: `valuesPerLong = floor(64 / bits)`, `bits = max(4, ceil(log2(paletteSize)))`, no entries spanning long boundaries
- `heightmaps.*` are decoded from their 36-long packed arrays using the same `BitStorage` rules (`bits = 9` for world-height heightmaps in 1.17.1)
- `biomes` is stored in MC's native 4×4×4 cells stride-ordered `y-major, z-major, x-minor` (the 1024-entry layout from `ChunkBiomeContainer.writeBiomes()`)
- Values are intentionally **factual measurements** — no interpretation, no re-ordering beyond decoding the packed-bits layout. This keeps fixtures robust to TS-side design choices in later tactical slices.

## Translation gotchas

- **1.17.1 BitStorage does not span longs.** Entries are packed into `floor(64 / bits)` slots per long with leftover high bits as padding. This changed in 1.16; anyone remembering the pre-1.16 "entries may span longs" layout will produce wrong decodes. Always test with `bits = 5` (14-entry longs) and `bits = 9` (7-entry longs) vectors.
- **`BlockStates` bits-per-entry is palette-dependent.** The 1.17.1 server writes `bits = max(4, ceil(log2(paletteSize)))` for sections with a palette; sections with a single palette entry may omit `BlockStates` entirely. Handle the absent-array case as "all indices are `0`."
- **Sections are sparse.** A chunk's `Sections` NBT list only includes sections that have at least one block state set (plus lighting sections). Empty-air sections are omitted; decoded fixtures should preserve the subset as-is rather than filling gaps with synthetic empty sections.
- **Heightmap `bits = 9`.** The `long[]` for `WORLD_SURFACE`, `OCEAN_FLOOR`, etc. is always 36 longs regardless of palette. Decoded values are in `[0, 383]` (1.17's extended height range prep, even though the actual terrain still tops out at Y = 319 for overworld).
- **Chunk status filter.** The server only persists *fully generated* chunks as `Status = "full"`. Fixtures should reject any chunk whose status is not `"full"` — those chunks represent partially generated border chunks and are not parity-comparable.
- **NBT endianness is big-endian.** Java's `DataInputStream` convention. Use `DataView.getInt32(offset, /* littleEndian = */ false)`.
- **Region file sectors are 4096 bytes.** Each chunk payload is prefixed with a 4-byte big-endian length + 1-byte compression type (`1 = gzip`, `2 = zlib`, `3 = uncompressed`, `128 | type = external .mcc file`). External chunks are rare for spawn-sized regions but the reader should reject them explicitly rather than silently returning garbage.
- **EULA file is mandatory.** The server refuses to start without `eula=true` in `eula.txt`. Ship the template checked in; make sure it's scoped to oracle-only use in the README.

## Concrete steps

1. `scripts/fetch-server-jar.sh`: new, minimal, SHA1-verified download of the 1.17.1 server jar using the existing Mojang manifest logic from `decompile-mc.sh`. Writes to `reference/minecraft-1.17.1/server.jar`. Idempotent.
2. `oracle/integration/run-server.sh` + `server/` template: spawn the server headless against a pinned seed, wait for `Done (`, send `stop` on stdin.
3. `src/oracle/anvil/nbt.ts`: NBT decoder. All 12 tag types. Big-endian. Modified UTF-8 (no surrogates; allow the usual ASCII identifier subset — throw on non-ASCII for the MVP since all keys we care about are ASCII).
4. `src/oracle/anvil/region.ts`: region-file header parse + chunk payload decompress (`zlib.inflateSync` and `zlib.gunzipSync` via Node's `node:zlib`). Reject external `.mcc` sidecar chunks.
5. `src/oracle/anvil/chunk.ts`: decode sections (palette + `BitStorage` unpack), heightmaps, biomes.
6. `src/oracle/integration/chunk-fixture.ts`: canonical shape + pure builder from decoded NBT.
7. `oracle/integration/dump-chunks.mjs`: CLI that stitches the pieces together. Reads `world/region/r.*.mca`, extracts a chunk range, writes the fixture JSON.
8. `oracle/integration/gen-fixture.sh`: orchestration (`fetch-server-jar.sh` → `run-server.sh` → `dump-chunks.mjs`).
9. Vitest coverage: packed-bits decoder vectors, NBT roundtrip for known tag shapes, region header decoding on a synthetic 8KB header, fixture-shape assertion on a committed sample fixture.
10. `oracle/README.md` gains an "Integration oracle" section; `docs/tactical/README.md` flips the `04-` row to "in progress → complete."

## Done when

- `scripts/fetch-server-jar.sh` places `reference/minecraft-1.17.1/server.jar` with the SHA1 from Mojang's manifest
- `oracle/integration/gen-fixture.sh --seed 12345 --chunks 0,0 --out test/fixtures/integration/overworld-seed-12345-chunks-0-0.json` produces a committable fixture end-to-end
- `pnpm test` covers the Anvil reader and chunk fixture extractor with synthetic inputs
- `pnpm typecheck` stays green
- `oracle/README.md` documents how to regenerate an integration fixture
- At least one integration fixture is checked in (small: 1 chunk at `(0, 0)` for seed `12345`)

## Out of scope for this doc

- any translation work on `NoiseBasedChunkGenerator`, `SurfaceBuilder`, carvers, or biome sources — those are tactical `05` / `06` / `07` / `08`
- multi-region fixture merging or large pre-generated worlds — add when tactical `06` demands wider diffs
- Minecraft-version abstractions — the reader is hardcoded to 1.17.1 NBT conventions for now (no pre-1.16 long-spanning bit storage, no 1.18 section-Y normalization)
- structure / entity NBT — `generate-structures=false` at the server side and the fixture format skips those regardless
- distribution — the server jar and the generated world stay gitignored under `reference/`, matching the existing convention
