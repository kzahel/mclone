# Asset extraction plan

The decompile pipeline only produces `.java` source. The `client.jar` contains a lot more — PNG textures, block models, structure NBTs, etc. — that's useful for bootstrapping the renderer and world populator without writing placeholder content from scratch.

## What's worth extracting (ranked)

1. **Textures** (`assets/minecraft/textures/`) — 16×16 PNGs for every block, item, entity, particle, GUI element. Drop directly into our texture atlas during dev. Biggest immediate unblocker for the renderer.
2. **Block models** (`assets/minecraft/models/block/`) — JSON geometry for non-cube blocks (stairs, slabs, fences, torches, buttons). Each has an `elements` array of box pieces with per-face UVs. Reference for our own model system; also parseable directly if we port the JSON format.
3. **Blockstates** (`assets/minecraft/blockstates/`) — JSON that maps stateful blocks (e.g. `stairs[facing=east,half=top]`) to a specific model + rotation. The adapter between game state and renderable geometry.
4. **Structure NBT** (`data/minecraft/structures/`) — actual building templates for villages, pillager outposts, woodland mansions, ancient cities, etc. Translated structure-position finders give coordinates; these give the blocks. Requires a small NBT parser (~200 LOC, well-documented binary format).
5. **`version.json`** + **`pack.png`** — version/build metadata and the default pack icon. Tiny.

   (Note: `pack.mcmeta` is *not* present in 1.17.1's vanilla `client.jar` despite earlier guesses; the resource-pack `pack_format` integer lives in `version.json`'s `pack_version.resource` instead.)

## Skip

- `shaders/` — post-processing only (spectator mode, creeper view). Not a general rendering shader. We'll write our own.
- `lang/`, `texts/`, `font/`, `particles/`, `gpu_warnlist.json` — niche / not needed.
- `data/minecraft/advancements`, `loot_tables`, `recipes`, `tags` — game logic data, not relevant until we have a game loop.
- Sounds — not in the jar. Live in a separate asset index (`totalSize: 348MB` per-version). Defer.

## Notes on 1.17.1 vs 1.18+

1.17.1's `data/minecraft/` does **not** have a `worldgen/` folder. In 1.18+, Mojang moved noise settings, biome parameters, configured features, and placement modifiers into JSON datapacks at `data/minecraft/worldgen/`. If we want data-driven worldgen references (vs. reading Java code), we'd fetch the 1.18.2 jar specifically for that folder, using the same `decompile-mc.sh` pipeline with `--server` (server jar is smaller and has the same `data/` tree).

## Extraction

Runs automatically at the end of `decompile-mc.sh` (client builds), or standalone:

```bash
./scripts/extract-assets.sh 1.17.1
./scripts/extract-assets.sh 1.17.1 --force         # re-extract
./scripts/extract-assets.sh 1.17.1 --out /tmp/mc   # alternate output dir
pnpm assets:pack                                  # build extracted.zip + manifest for browser runtime
```

Output lands at `reference/minecraft-<version>/extracted/`. The script is idempotent — it skips if `extracted/pack.mcmeta` already exists.

Expected output size: ~30MB (mostly textures and structure NBTs). Full `assets/*` + `data/*` dump would be ~40MB; the filter skips the categories we don't need.

For browser deployment, the loose extracted tree is packed into `reference/minecraft-1.17.1/extracted.zip` with a sibling `extracted.zip.json` manifest containing size, file count, and SHA-256. The runtime loads and verifies the zip once, then resolves vanilla resource paths out of the in-memory pack. This avoids issuing thousands of small HTTP requests for blockstate/model/texture metadata.

## Where to put extracted assets

- **Not in the git repo.** The `.gitignore` excludes `extracted/`, `minecraft-*/`, `reference/`.
- Output sits next to the decomp: `reference/minecraft-1.17.1/extracted/`.
- When we're ready to use them, reference by path from mclone's build system.

## Not automated yet

Sound extraction. Sounds aren't in `client.jar` — they live in a per-version asset index (`totalSize: ~348MB`), each `.ogg` addressed by SHA1 from `resources.download.minecraft.net`. When/if we need audio, add a sound-fetching step (to `extract-assets.sh` or a sibling script).

## Legal reminder

Personal/home use only, per the project's stance. These assets are Mojang's IP. Don't commit them, don't publish them, don't include them in any public release. For any eventual distribution, replace with originals or a CC0 pack (e.g. Kenney's voxel assets).
