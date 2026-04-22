# mclone

Web-based Minecraft-inspired voxel sandbox. Private project — primary target is home/LAN use for my daughter to play with.

See [`docs/strategy.md`](docs/strategy.md) for the two-phase plan (direct translation now, optional clean-room only if we ever want to distribute) and [`docs/assets-plan.md`](docs/assets-plan.md) for asset extraction. Implementation work is tracked in numbered tactical docs under [`docs/tactical/`](docs/tactical/). Worldgen aims for **seed parity** with Minecraft Java 1.17.1 so we can oracle-test against real MC output.

## Stack

- **Language:** TypeScript end-to-end — host, workers, worldgen, meshing. No Rust.
- **Renderer:** WebGPU. Greedy-meshed chunks packed into instanced buffers.
- **Worldgen:** TS translation of MC 1.17.1's pipeline; bit-exact seed parity is the correctness bar. See `docs/tactical/`.
- **Chunk storage:** IndexedDB (consider OPFS as alternative for large binary blobs).
- **Host:** Vite + workers. Main thread handles input/UI; workers own worldgen + meshing.
- **Perf escape hatch:** any measured-hot module can move to WASM-from-C (not Rust). `cubiomes` already ships as C→WASM for biomes/structures — that's the only WASM in the default stack.

## Worldgen strategy

**Port cubiomes** (MIT) to WASM for biome IDs + structure positions — no translation-hygiene concerns, it's fair-use from day one.

**Directly translate** 1.17.1's terrain gen pipeline from the decomp: PRNG → noise → `NoiseSampler` → `NoiseBasedChunkGenerator` → carvers (`CaveWorldCarver`, `CanyonWorldCarver`) → surface rules → features. Oracle-test each layer against real MC output (see strategy doc). 1.17 specifically because it's pre-Caves-and-Cliffs — no density functions or splines to port.

Pipeline per chunk:

1. cubiomes → biome IDs, climate params, structure attempt positions
2. Our 3D density field (translated `NoiseSampler`) → solid/air
3. Carvers (translated `CaveWorldCarver` / `CanyonWorldCarver`) → caves + ravines
4. Surface rules (translated `SurfaceBuilder`) → grass/dirt/sand/stone per biome
5. Ore veins + features (translated `OreFeature` / `TreeFeature`)
6. Structures populated from cubiomes positions (block templates from extracted NBT)

Worker boundary is chunk-sized: pass chunk coords into a worldgen worker, get a packed block array + heightmap back. No per-voxel calls across thread boundaries. Same rule applies to the cubiomes WASM call, and to any future TS-module-migrated-to-WASM.

## References (repo-local, under `reference/` — gitignored)

### `cubiomes/`

Cubitect's C library that mimics Minecraft biome/structure gen. Clean C, no deps, compiles to WASM cleanly.

**Provides:**
- `getBiomeAt(g, scale, x, y, z)` — biome ID at any point, MC 1.0–1.21
- `genBiomes(g, cache, range)` — bulk biome volume
- 1.18+ climate: `sampleBiomeNoise()` returns `temperature, humidity, continentalness, erosion, depth, weirdness`
- `sampleSurfaceNoise(sn, x, y, z)` — pre-1.18 3D density field (octmin/octmax/octmain), genuinely 3D, supports overhangs in principle — `biomenoise.c:118`
- `mapApproxHeight()` — surface heightmap approximation — `generator.c:610`
- Structure positions + viability (`getStructurePos`, `isViableStructurePos`) — villages, strongholds, monuments, ancient cities, portals, mineshafts, etc.
- Nether 3D biomes (`mapNether3D`), End islands (`mapEndSurfaceHeight`)
- Stronghold iterator, spawn finder

**Does NOT provide** (grep confirmed — zero hits):
- Caves, carvers, spaghetti/noodle/cheese, aquifers, ravines
- Ores, veins, dripstone, geodes (beyond position)
- 1.18+ density function pipeline (continentalness→spline→final density)
- Actual block placement (stone vs dirt vs grass, water fill, beaches)
- Features/decoration (trees, foliage, snow)
- Structure piece block templates (positions only, no NBT)

Key headers: `generator.h`, `biomenoise.h`, `finders.h`, `noise.h`.

### `minecraft-1.17.1/`

Decompiled Minecraft 1.17.1 client, for reference when writing our own carvers / surface / features.

Why 1.17 specifically: pre-Caves-and-Cliffs full release, so the terrain pipeline is way simpler than 1.18+ (no density functions, no continentalness splines). Matches cubiomes' `sampleSurfaceNoise` cleanly. Bonus: 1.17.1 already ships Caves & Cliffs Part 1 internals (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`) that weren't enabled by default — so we get both systems to reference.

**Layout:**
```
client.jar          original obfuscated (SHA1-verified from Mojang)
client.txt          official Mojang Proguard mappings
client-deobf.jar    remapped
src/                ~4100 .java files, fully decompiled
tools/              SpecialSource + Vineflower jars
parchment/          Parchment zip cache (if --parchment was used)
extracted/          filtered client.jar assets — textures, models,
                    blockstates, structure NBTs (see docs/assets-plan.md)
*.json              Mojang manifests (for re-running pipeline)
```

**Key files for worldgen:**
- `net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` — top-level terrain pipeline, 3D density → blocks
- `.../NoiseSampler.java` — the 3D density sampler
- `.../carver/CaveWorldCarver.java` — classic connected-tunnel caves
- `.../carver/CanyonWorldCarver.java` — ravines
- `.../carver/WorldCarver.java` — carver base class
- `.../Aquifer.java` — water/lava in caves
- `.../Cavifier.java`, `NoodleCavifier.java` — 1.18-style noodle/cheese caves
- `.../OreVeinifier.java` — ore vein placement
- `.../surfacebuilders/` — grass/dirt/sand layering per biome
- `.../feature/` — trees, ore blobs, foliage

**Parameter names:** Mojang mappings don't cover method parameters, so a raw decompile has `var1, var2, …` in every signature. The `--parchment` flag to `decompile-mc.sh` runs `apply-parchment.py` afterwards, which pulls [Parchment](https://parchmentmc.org/) community mappings and rewrites signatures + bodies to use real names (e.g. `carve(CarvingContext context, CaveCarverConfiguration config, ChunkAccess chunk, …)` instead of `(var1, var2, var3, …)`). For 1.17.1 this renames ~15k methods across ~2.8k files.

**True local variables** (declared inside method bodies, e.g. `int var9 = ...`) stay as `varN` regardless of mapping set — that information was never in the obfuscated jar.

## Setup scripts

Everything in the "References" section above can be rebuilt from scratch using scripts in `scripts/`. Outputs land in `<repo>/reference/` (gitignored).

```bash
# cubiomes → reference/cubiomes (shallow git clone)
./scripts/fetch-cubiomes.sh

# Minecraft 1.17.1 client: download + remap + decompile + Parchment +
# asset extraction → reference/minecraft-1.17.1
./scripts/decompile-mc.sh 1.17.1 --parchment

# Other versions / server jar / custom output dir
./scripts/decompile-mc.sh 1.16.5 --parchment
./scripts/decompile-mc.sh 1.18.2 --server --parchment
./scripts/decompile-mc.sh 1.17.1 --out /tmp/mc-scratch
./scripts/decompile-mc.sh 1.17.1 --no-assets       # skip asset extraction
./scripts/decompile-mc.sh 1.17.1 --force           # redo all steps

# Standalone asset extraction (if you already ran decompile-mc.sh)
./scripts/extract-assets.sh 1.17.1

# Apply Parchment standalone to an existing tree
./scripts/apply-parchment.py reference/minecraft-1.17.1/src --mc 1.17.1
```

Prereqs: JDK 17+, `curl`, `jq`, `sha1sum`, `unzip`. `python3` for `--parchment`.

The decompile script is idempotent — each step (manifest fetch / jar download / remap / decompile) skips if its output already exists. Re-running just prints "Done". Use `--force` to redo. Official Mojang mappings are only available for **1.14.4+**; older versions need MCP.

Pipeline stages, for reference:

1. Fetch `piston-meta.mojang.com/mc/game/version_manifest_v2.json` → find version entry
2. Fetch that version's per-version manifest → get SHA1-addressed URLs for jar + mappings
3. Download `client.jar` (or `server.jar`) and `client.txt` (Proguard-format deobf→obf map)
4. Remap with **SpecialSource** (default direction, no `--reverse` — SpecialSource's Proguard parser expects `deobf -> obf` already)
5. Decompile with **Vineflower**
6. (Optional, `--parchment`) Download Parchment zip from `maven.parchmentmc.org` → parse `parchment.json` → rewrite `.java` files to replace `varN` method params with Parchment names
7. (Client only, unless `--no-assets`) Extract filtered textures / models / blockstates / structures from `client.jar` into `extracted/` via `extract-assets.sh` (see `docs/assets-plan.md`)

## Community mappings (for parameter names and javadoc)

Mojang's mappings only cover **class / method / field** names. Local variables come out as `var1, var2, …` because the compiled bytecode never contained local names (stripped from `LocalVariableTable`). Parameter names and javadoc aren't in Mojang's mappings either.

The main community mapping sets that fill these gaps:

| | **Mojang** | **Yarn** (Fabric) | **Parchment** |
|---|---|---|---|
| Class / method / field | official | replacement names | uses Mojang's |
| Parameter names | no | yes | yes |
| Javadoc | no | yes | yes |
| Format | Proguard `.txt` | Tiny v2 | layered JSON on top of Mojang |
| Standalone CLI | easy | moderate (tiny-remapper) | painful (built for Gradle) |

- **Yarn** *replaces* Mojang's class/method names with community picks. More descriptive, but you lose the "grep for 'NoiseBasedChunkGenerator' and find it in Minecraft wiki or mod-dev articles" workflow.
- **Parchment** is the right fit for our reference-reading use case — it's *layered*, so you keep Mojang's names and just get parameter names added. Downside: Parchment's tooling assumes Gradle + NeoGradle / Librarian. No clean standalone CLI.

**Our approach:** Mojang for names, Parchment for parameters, via a small Python post-processor (`scripts/apply-parchment.py`). It downloads the Parchment release from Maven (auto-selects the latest stable for the given MC version), parses `parchment.json`, walks the decompiled tree, and uses a regex-based Java scanner to find method signatures, match them to Parchment by `(class, method_name, param_count)` positionally, and rewrite `varN` identifiers across each method's signature + body with word-boundary-safe substitution. Ambiguous overloads are skipped. Script is idempotent — a second run produces zero renames. This sidesteps all the Gradle/Librarian tooling that Parchment normally requires.

Neither Mojang, Yarn, nor Parchment can recover true **local variable names inside method bodies** — that information was never in the obfuscated jar. Those are decompiler-generated and will stay as `var1, var2…` regardless of mapping set.

## Open questions (original-work side only)

Worldgen questions are settled by the direct-translation strategy — we mirror what MC does, then oracle-test against it. Open questions for parts we're writing from scratch:

- **Meshing:** greedy in TS first, emit indexed buffer directly. Investigate binary greedy meshing (bitwise tricks over 64-wide `Uint32Array` columns). Move to C→WASM only if measured.
- **Lighting:** flood-fill on chunk changes. Two passes (block + sky). Possibly deferred to GPU compute shader.
- **Chunk compression:** raw palette-encoded blocks → LZ4 or zstd before IndexedDB write? Trade-off between storage size and seek latency.
- **Worker topology:** one worldgen worker per core? One shared, queued? Ownership of chunk memory across transfers.
