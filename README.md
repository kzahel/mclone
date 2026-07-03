# mclone

Minecraft-inspired voxel sandbox. Private project — primary target is home/LAN use for my daughter to play with.

The current direction is a native-first Rust engine with five validated client/platform lanes: desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM. The basic loop is proven across those lanes: local integrated runtime, locomotion/input, world rendering, and chunk loading/generation. Native desktop flat remains the fastest daily bring-up path, and the headless/offscreen path is being promoted into a real no-window flat client validation host rather than a screenshot-only helper. Shared gameplay, runtime, asset, mesh, UI, renderer, and XR contracts must stay host-neutral. Client platform and server host mode are separate axes: every client lane should be able to play against a dedicated server, with future P2P/session topologies fitting behind the same shared command/update contracts. See [`docs/platforms.md`](docs/platforms.md) for the current platform matrix, [`docs/offscreen-flat-client.md`](docs/offscreen-flat-client.md) for the offscreen flat-client target, [`docs/native-rewrite-roadmap.md`](docs/native-rewrite-roadmap.md) for durable architecture, [`android/README.md`](android/README.md) for flat Android, and [`android-xr/README.md`](android-xr/README.md) for Quest/OpenXR. The retired browser engine has been removed from the live tree; retained reference tooling lives under [`oracle/`](oracle/) and shared oracle fixture data remains under [`test/fixtures/`](test/fixtures/). The decompiled Minecraft Java 1.17.1 client under [`reference/minecraft-1.17.1/src/`](reference/minecraft-1.17.1/src/) is the primary source for vanilla behavior and visual correctness; the sibling Rust engine at `~/code/playbox` is an explicit reference for native `winit`/`wgpu`, headless capture, diagnostics, Android, and OpenXR patterns.

See [`docs/strategy.md`](docs/strategy.md) for translation/oracle policy, [`docs/architecture.md`](docs/architecture.md) for the existing runtime/host split, [`docs/runtime-data-model.md`](docs/runtime-data-model.md) for shared chunk/block-state data contracts, [`docs/protocol.md`](docs/protocol.md) for the host/client message model, [`docs/loading-persistence.md`](docs/loading-persistence.md) for world loading and save policy, [`docs/persistence-architecture.md`](docs/persistence-architecture.md) for the broader shared persistence target, [`docs/worldgen-deterministic-order.md`](docs/worldgen-deterministic-order.md) for vanilla status order, decoration finality, lighting gates, and chunk publication gates, [`docs/lod-architecture.md`](docs/lod-architecture.md) for the far-terrain LOD plan, [`docs/carver-status.md`](docs/carver-status.md) for the carver-parity/oracle tracker, [`docs/structures.md`](docs/structures.md) for vanilla overworld structure generation architecture, [`docs/liquids.md`](docs/liquids.md) for liquid simulation architecture, [`docs/entity-architecture.md`](docs/entity-architecture.md) for entity/mob runtime boundaries, [`docs/creatures.md`](docs/creatures.md) for overworld creature spawning architecture, [`docs/topics/`](docs/topics/README.md) for durable subsystem progress indexes, [`docs/performance-records.md`](docs/performance-records.md) for native benchmark baselines, and [`docs/assets-plan.md`](docs/assets-plan.md) for asset extraction. Native Rust workstream tacticals live under [`docs/tactical/`](docs/tactical/README.md). Worldgen aims for **seed parity** with Minecraft Java 1.17.1 so we can oracle-test against real MC output.

Runtime/host arc status: `R0` through `R8` are landed: browser singleplayer, native dedicated hosting, remote browser clients, protocol hardening, and the first authoritative player/control loop all use the shared host/client boundary.

## Stack

- **Primary language:** Rust, under [`native/`](native/).
- **Primary development loop:** desktop flat, with `mclone-native-client` as the fastest interactive loop and the offscreen flat-client host as the target no-window validation path.
- **Desktop XR:** opt-in OpenXR mode in `mclone-native-client` behind the `xr` feature; validated with real mclone stereo terrain and controller locomotion through shared XR crates.
- **Android XR / Quest:** standalone Quest OpenXR package under [`android-xr/`](android-xr/) using `mclone-android-xr-client`; validated with staged assets, real stereo terrain, controller setup, and basic locomotion.
- **Flat Android:** non-XR `NativeActivity` APK under [`android/`](android/) using `mclone-android-client`; validated with AVD screenshot and touch-orbit smokes.
- **Web target:** Rust/WASM browser client through `mclone-web-client`, WebGPU, browser workers, and deployment at `mclone.kzahel.com`.
- **Renderer:** `wgpu`, native first, web-compatible capability checks at renderer milestones. Vanilla visual behavior should be checked against the Java 1.17.1 client source before borrowing renderer policy from other engines.
- **Worldgen:** direct Rust port of MC Java 1.17.1's pipeline; bit-exact seed parity is the correctness bar.
- **Protocol/runtime:** `mclone_protocol`, `mclone_net`, `mclone_server`, `mclone_client`, `mclone_app_runtime`, and `mclone_render_session` keep singleplayer, remote, render-section, and platform app paths on shared contracts.
- **Host mode invariant:** local integrated, remote dedicated, and future P2P/session modes are runtime host choices, not platform identities. Desktop, web, flat Android, and XR clients should converge on the same client/server protocol and runtime shell wherever the display/input platform permits it.
- **XR sharing:** `mclone_xr_host`, `mclone_xr_graphics`, and `mclone_xr_scene` keep desktop XR and Android XR from growing private copies of session, swapchain, terrain, and controller-locomotion behavior.
- **Java reference client:** `reference/minecraft-1.17.1/src/` is the authority for vanilla block/entity rendering behavior, model baking, atlas stitching, mipmaps/filtering, render layers, lighting, fog, sky, particles, and client-visible state.
- **Sibling reference engine:** `~/code/playbox` is the local Rust `winit`/`wgpu`/headless/Android/OpenXR pattern library. For platform or XR work, start with its `Cargo.toml`, `docs/architecture/rendering.md`, `docs/architecture/platforms.md`, `android/README.md`, and `android-xr/README.md`.
- **Oracle tooling:** Java and TypeScript fixture-generation helpers live under [`oracle/`](oracle/); shared fixture JSON remains under [`test/fixtures/`](test/fixtures/) and is consumed by native Rust tests.

## Native Web

The Rust/WASM web app is served from:

```text
https://mclone.kzahel.com/
```

Use the native web scripts for this path:

```bash
# Build and serve the Rust/WASM app locally with the headers required for
# SharedArrayBuffer/Web Workers. The command prints the local app URL.
pnpm native:web:serve

# Validate the interactive browser app with Playwright screenshots in /tmp.
pnpm native:web:app-smoke

# Build the exact deploy bundle into dist-native-web/ without uploading.
pnpm native:web:bundle

# Build, upload the native web bundle and asset pack to the mclone R2 bucket,
# deploy the Cloudflare Worker, and make it available at mclone.kzahel.com.
pnpm deploy
```

The deploy path packages `native/apps/mclone-web-client/www`, wasm-bindgen
output under `/pkg/`, and `reference/minecraft-1.17.1/extracted.zip`, then
serves them through `worker/index.js` with COOP/COEP/CORP headers. Wrangler
must be authenticated for the Cloudflare account before deploy. `pnpm deploy`
is an alias for `pnpm native:web:deploy`.

For this local checkout,
`scripts/local-deploy/deploy-after-main-push.sh` can be installed as a
`pre-push` hook:

```bash
./scripts/local-deploy/install-hook.sh
```

The hook returns immediately, then a background worker waits until the pushed
`main` commit is visible on the remote before running `pnpm deploy`. Quick
successive pushes replace the pending SHA before deployment starts. The worker
deploys from a reusable sibling worktree, by default `../mclone-deploy-worktree`,
which it resets to the pushed commit before running `pnpm deploy`; your active
checkout can be edited immediately after pushing. Local ignored inputs/caches
such as `reference/minecraft-1.17.1` and `node_modules` are linked into that
deploy worktree when present.

Status is available with
`./scripts/local-deploy/deploy-after-main-push.sh --status`; the latest summary,
completed deploy, and failed deploy are stored under
`.git/mclone-deploy-after-main-push/`, with the full log in `deploy.log`.
Completed records include total seconds from hook scheduling to deploy success,
deploy command seconds, and the deploy worktree path.

## Worldgen strategy

**Directly translate** 1.17.1's full worldgen pipeline from the decomp into Rust: PRNG → noise → biome source → `NoiseSampler` → `NoiseBasedChunkGenerator` → carvers (`CaveWorldCarver`, `CanyonWorldCarver`) → surface rules → features → structures. Oracle-test each layer against real MC output (see strategy doc). Preserve the vanilla status scheduling and finality gates in [`docs/worldgen-deterministic-order.md`](docs/worldgen-deterministic-order.md). 1.17 specifically because it's pre-Caves-and-Cliffs — no density functions or splines to port.

Pipeline per chunk:

1. Biome source (translated `OverworldBiomeSource` / `BiomeManager`) → biome IDs + climate params
2. Our 3D density field (translated `NoiseSampler`) → solid/air
3. Carvers (translated `CaveWorldCarver` / `CanyonWorldCarver`) → caves + ravines
4. Surface rules (translated `SurfaceBuilder`) → grass/dirt/sand/stone per biome
5. Ore veins + features (translated `OreFeature` / `TreeFeature`)
6. Structures (status-aware starts, references, per-chunk placement, and block templates from extracted NBT; see [`docs/structures.md`](docs/structures.md))

Runtime boundaries should stay chunk/section-sized: pass chunk or section facts across crates, workers, transports, and WASM/web adapters, never per-voxel calls.

## References (repo-local, under `reference/` — gitignored)

### `minecraft-1.17.1/`

Decompiled Minecraft 1.17.1 client, for reference when writing our own vanilla simulation, content, asset, and renderer behavior.

Why 1.17 specifically: pre-Caves-and-Cliffs full release, so the terrain pipeline is way simpler than 1.18+ (no density functions, no continentalness splines). Bonus: 1.17.1 already ships Caves & Cliffs Part 1 internals (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`) that weren't enabled by default — so we get both systems to reference.

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

**Key files for client rendering and assets:**
- `net/minecraft/client/renderer/texture/TextureAtlas.java` — atlas preparation, upload, mip-level selection, and filter updates
- `net/minecraft/client/renderer/texture/TextureAtlasSprite.java` — sprite UVs, per-sprite mip generation, animation frame upload, and UV shrink ratio
- `net/minecraft/client/renderer/texture/MipmapGenerator.java` — gamma-aware mipmap generation and alpha-cutout handling
- `com/mojang/blaze3d/platform/NativeImage.java` and `TextureUtil.java` — texture upload/filtering and GL mip allocation behavior
- `net/minecraft/client/renderer/block/model/` — block model baking and baked quad UV behavior
- `net/minecraft/client/renderer/RenderType.java` and `RenderStateShard.java` — vanilla render-layer state, transparency, culling, and texture-state choices

**Parameter names:** Mojang mappings don't cover method parameters, so a raw decompile has `var1, var2, …` in every signature. The `--parchment` flag to `decompile-mc.sh` runs `apply-parchment.py` afterwards, which pulls [Parchment](https://parchmentmc.org/) community mappings and rewrites signatures + bodies to use real names (e.g. `carve(CarvingContext context, CaveCarverConfiguration config, ChunkAccess chunk, …)` instead of `(var1, var2, var3, …)`). For 1.17.1 this renames ~15k methods across ~2.8k files.

**True local variables** (declared inside method bodies, e.g. `int var9 = ...`) stay as `varN` regardless of mapping set — that information was never in the obfuscated jar.

## Setup scripts

Everything in the "References" section above can be rebuilt from scratch using scripts in `scripts/`. Outputs land in `<repo>/reference/` (gitignored).

```bash
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

- **Meshing:** native Rust first, emit renderer-ready indexed buffers through `mclone_mesh`; keep payloads plain enough for WASM/web transfer.
- **Lighting:** vanilla-shaped sky/block solver in `mclone_light`; evaluate CPU threading first, GPU compute only for measured follow-up work.
- **Chunk compression:** raw palette-encoded blocks to LZ4/zstd or region-style storage behind engine-native adapters; web storage remains an adapter constraint, not the core format.
- **Threading topology:** native worker pools first, but design scheduler/memory ownership so web workers plus `SharedArrayBuffer` can support the same chunk/status model.
- **Platform boundaries:** desktop app code may use `winit`, Android apps may use activity/JNI glue, web may use DOM/workers, and XR hosts may use OpenXR sessions/swapchains, but shared client/server/mesh/asset/UI/runtime contracts should stay independent of those adapters. Renderer inputs should carry explicit view/projection and render-target facts so no platform needs to unwind another platform's assumptions.
- **Server topology:** singleplayer integrated servers, dedicated servers, and future P2P sessions should all drive clients through the shared protocol/runtime boundary. A platform may lag in UI/config exposure, but it should not need private gameplay or render-session logic to join a shared world.
