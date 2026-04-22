# mclone

Web-based Minecraft-inspired voxel sandbox. Not seed-compatible with Java Edition — aims for Minecraft *feel*, not bit-exact parity.

## Stack

- **Renderer:** WebGPU. Greedy-meshed chunks packed into instanced buffers.
- **Worldgen / hot paths:** WASM (C or Rust). Terrain gen, meshing, lighting, chunk (de)serialization.
- **Chunk storage:** IndexedDB (consider OPFS as alternative for large binary blobs).
- **Host:** TypeScript + Vite, main thread handles input/UI, workers own worldgen + meshing.

## Worldgen strategy

Port **cubiomes** to WASM for biome IDs + structure positions. Write our own 3D density + carvers on top, using **1.17 decompiled source** as reference. Skip reimplementing 1.18+ density functions / splines — way too much surface area, and we're not seed-compatible anyway.

Pipeline sketch per chunk:

1. cubiomes → biome IDs, climate params (temperature/humidity/continentalness/erosion/weirdness), structure attempt positions
2. Our 3D density field (simplex or adapted 1.17 `NoiseSampler`) → solid/air
3. Carvers (worley-based or ported `CaveWorldCarver` / `CanyonWorldCarver`) → caves + ravines
4. Surface rules (port of `SurfaceBuilder`) → grass/dirt/sand/stone per biome
5. Ore veins + features (trees, flowers) — ported `OreFeature` / `TreeFeature` logic
6. Structures populated from cubiomes positions

JS ↔ WASM boundary is chunk-sized: pass chunk coords in, get packed block array + heightmap out. No per-voxel calls across the boundary.

## References (outside this repo, in `~/code/reference/`)

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

Everything in the "References" section above can be rebuilt from scratch using scripts in `mclone/scripts/`.

```bash
# cubiomes → ~/code/reference/cubiomes (shallow git clone)
./mclone/scripts/fetch-cubiomes.sh

# Minecraft 1.17.1 client, decompiled + Parchment param names applied
#   → ~/code/reference/minecraft-1.17.1
./mclone/scripts/decompile-mc.sh 1.17.1 --parchment

# Other versions / server jar / custom output dir
./mclone/scripts/decompile-mc.sh 1.16.5 --parchment
./mclone/scripts/decompile-mc.sh 1.18.2 --server --parchment
./mclone/scripts/decompile-mc.sh 1.17.1 --out /tmp/mc-scratch
./mclone/scripts/decompile-mc.sh 1.17.1 --force          # redo all steps

# Apply Parchment standalone to an existing tree
./mclone/scripts/apply-parchment.py ~/code/reference/minecraft-1.17.1/src --mc 1.17.1
```

Prereqs: JDK 17+, `curl`, `jq`, `sha1sum`. `python3` for `--parchment`.

The decompile script is idempotent — each step (manifest fetch / jar download / remap / decompile) skips if its output already exists. Re-running just prints "Done". Use `--force` to redo. Official Mojang mappings are only available for **1.14.4+**; older versions need MCP.

Pipeline stages, for reference:

1. Fetch `piston-meta.mojang.com/mc/game/version_manifest_v2.json` → find version entry
2. Fetch that version's per-version manifest → get SHA1-addressed URLs for jar + mappings
3. Download `client.jar` (or `server.jar`) and `client.txt` (Proguard-format deobf→obf map)
4. Remap with **SpecialSource** (default direction, no `--reverse` — SpecialSource's Proguard parser expects `deobf -> obf` already)
5. Decompile with **Vineflower**
6. (Optional, `--parchment`) Download Parchment zip from `maven.parchmentmc.org` → parse `parchment.json` → rewrite `.java` files to replace `varN` method params with Parchment names

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

## Open questions

- Which noise for 3D density: port 1.17's `NoiseSampler` (matches cubiomes' surface noise) vs. roll our own simplex-based one? Former is closer to Minecraft feel but more code.
- Carver approach: port 1.17's random-walk carver vs. use a worley-based analytic field (no stored state, works great in a WASM shader-like kernel).
- Meshing: greedy in WASM, emit indexed buffer directly. Investigate binary greedy meshing (bitwise tricks over 64-wide columns).
- Lighting: flood-fill on chunk changes. Two passes (block + sky). Possibly deferred to GPU compute shader.
