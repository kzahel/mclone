See [`README.md`](README.md) for project context.

## Windows: shell and Rust toolchain

This repo is developed on Windows. Run `cargo`, `pnpm`, and any native build/test/run from **PowerShell** (or the Git Bash / MINGW64 shell) — both use the native Windows Rust toolchain at `C:\Users\<user>\.cargo\bin\cargo.exe` and work correctly (clean `cargo build`/`cargo test`, including MSVC linking and test-binary execution, all pass).

**Do not run cargo from WSL.** WSL is installed and running on this machine, but it has **no Rust toolchain** (`cargo: command not found`), and even with one it builds over the slow `/mnt/c` mount, which hangs and targets Linux instead of Windows. If a `cargo`/`pnpm` command "gets stuck" or reports `command not found`, you are almost certainly in WSL, not Git Bash — switch shells rather than trying to fix the build.

The `.sh` setup scripts (`scripts/decompile-mc.sh`, `scripts/extract-assets.sh`, `oracle/build.sh`, `oracle/run.sh`, etc.) require bash; run those from Git Bash, not PowerShell. The `pnpm` cargo wrappers in `package.json` are shell-agnostic and run fine from either PowerShell or Git Bash.

## Notes for agents

**Do not use the auto-memory system** for this project (the `~/.claude/projects/-home-kgraehl-code-mclone/memory/` directory). Persist project-relevant guidance in this file (`AGENTS.md`) instead.

For the native Rust rewrite, use the sibling engine at `~/code/playbox` as a reference for mature `winit`/`wgpu`, headless capture, diagnostics, Android, and OpenXR patterns. Treat it as a pattern library only; do not depend on it directly, and do not copy its PhysX/VaM-specific runtime shape. When the user mentions "Playbox", inspect that sibling repo directly; useful entry points are `~/code/playbox/Cargo.toml` for debug-profile optimization policy, `~/code/playbox/docs/architecture/rendering.md` for render target/view boundaries, `~/code/playbox/docs/architecture/platforms.md` for desktop/Android/OpenXR platform shape, `~/code/playbox/android/README.md` for flat Android, and `~/code/playbox/android-xr/README.md` for Quest/OpenXR packaging and validation notes.

Current target posture: five client/platform lanes are validated at the basic gameplay/rendering level: desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM. Native desktop flat remains the fastest day-to-day bring-up path, but do not let shared engine, client, server, mesh, asset, renderer, UI, or runtime contracts become desktop-only. Treat client platform and server host mode as separate axes: every client lane must retain a path to dedicated-server play, and future P2P/session topologies must fit behind the same shared command/update contracts rather than becoming platform forks. Keep `winit`, Android activity glue, browser glue, and OpenXR session/swapchain ownership in app/platform adapters. Keep renderer-facing view/projection and render-target data explicit, and keep headless/offscreen validation available. Desktop XR and Android XR share OpenXR host/graphics/scene boundaries where practical; do not fork gameplay, runtime, meshing, asset, or renderer internals for a single platform.

For web/WASM, keep `wasm32-unknown-unknown` as the browser target unless a tactical explicitly changes it, but do not treat that as permission for a single-threaded or reduced engine architecture. Browser CPU work should converge on the same job/worker lifecycle as desktop: desktop uses native OS threads, while browser/WASM uses Web Workers with shared Wasm memory (`SharedArrayBuffer`/atomics) once that slice is implemented. Inline synchronous WASM paths are acceptable only as temporary smoke/fallback implementations behind the same compiler/session interfaces, not as the target threading model.

The retired browser engine has been removed from the live tree. Git history is the archive for old implementation context. Retained oracle helpers under `oracle/lib/**` and shared fixture data under `test/fixtures/**` are active reference assets, not legacy engine code.

Native Rust rewrite tactical docs live under `docs/tactical/` and use zero-padded numeric filenames such as `000-topic.md`, `001-next-topic.md`. Historical legacy tacticals, if still present during cleanup, are not implementation guidance for new work.

## Workstream routing

Default implementation target is the native Rust workspace under `native/`.

For any request about engine behavior, client/runtime behavior, renderer behavior, input, movement, camera controls, assets, mesh, server, worldgen, UI, or platform work, inspect and edit the native Rust path first. Relevant native entry points include:

- `native/Cargo.toml`
- `native/apps/mclone-native-client/src/main.rs`
- `native/apps/mclone-web-client/src/lib.rs`
- `native/crates/mclone-*`
- `docs/tactical/README.md`

Do not recreate or maintain the retired TypeScript engine surface. If a request explicitly needs old behavior context, use Git history or retained oracle fixtures as reference before changing live native code.

Before the first edit, identify the workstream being modified: `native Rust`, `native web/WASM`, `oracle/reference only`, or `documentation cleanup`.

## Reference-porting policy

For vanilla parity ports, every class or system has a 1:1 counterpart in `reference/minecraft-1.17.1/src/`. **Always read the source file before writing the port.** The default is direct translation: same field names where practical, same method names where practical, same logic flow. Diverge only when the target platform, runtime ownership, or Rust type system forces it.

The main rule is:

- simulation/content parity is the default
- runtime orchestration may diverge when platform constraints require it
- any such divergence must preserve a clear path for future parity work instead of making it opaque or harder to recover

Before making an architectural divergence, explicitly determine what the reference Minecraft source does today, why that shape is a poor fit for the native/web/runtime target, the exact scope of the divergence, and whether the divergence makes future parity work easier, neutral, or harder.

### Reference tree must be present

Run `./scripts/decompile-mc.sh` if `reference/minecraft-1.17.1/src/` is missing. Do not write a port without the source in hand.

### Oracle and jar prerequisites

Before doing oracle work, make sure the local Minecraft artifacts are actually hydrated instead of assuming they are already present.

- If `reference/minecraft-1.17.1/src/` or `reference/minecraft-1.17.1/client-deobf.jar` is missing, run `./scripts/decompile-mc.sh` first.
- For the Java oracle harness, run `./oracle/build.sh` to hydrate Mojang-declared runtime libraries into `reference/minecraft-1.17.1/libraries` and compile `oracle/java/*.java` into `oracle/classes`.
- `pnpm --silent oracle:gen ...` is the normal entry point and already calls `oracle/run.sh`, which builds first if needed. Use this for real fixture generation; use `./oracle/build.sh` when you only need to prepare dependencies.
- For server-backed integration fixtures, make sure `reference/minecraft-1.17.1/server.jar` exists by running `./scripts/fetch-server-jar.sh 1.17.1` before `./oracle/integration/gen-fixture.sh ...`.
- If any of those download steps fail because the sandbox cannot reach Mojang hosts, request escalation immediately and rerun the same command with elevation. For this repo, elevation is explicitly allowed for downloading the decomp inputs, Mojang runtime libraries, and the pinned 1.17.1 server jar.
- Do not stop at "network failed" when oracle work is part of the task. The expected recovery path is: rerun the same bootstrap/download command with elevation, then continue with fixture generation or parity testing.

## Native validation

For any native slice that produces pixels, **capture a screenshot and look at it before moving on.** Do not finish a whole slice and then check. Check at the first drawable milestone, then keep checking as complexity increases.

Use native validation lanes first:

- `cargo test --manifest-path native/Cargo.toml`
- `pnpm native:worldgen:smoke` for native worldgen smoke coverage
- `pnpm native:movement:smoke` for native movement/runtime camera paths
- `pnpm native:timedemo:smoke` for native renderer/camera paths
- `pnpm native:web:build` and `pnpm native:web:smoke` for Rust WASM/web compatibility gates

For rendered-output validation, use native headless captures where available and save debug, smoke, and probe screenshots to `/tmp` (for example `/tmp/mclone-native-debug.png`). Never write screenshots into the repo, into `test-results/`, or anywhere that risks getting committed.

## Target: Minecraft Java 1.17.1 vanilla overworld

Seed parity against 1.17.1 vanilla overworld is the correctness bar. The decomp under `reference/minecraft-1.17.1/` contains Caves & Cliffs Part 1 systems that are **present in the source but disabled by default in 1.17.1**. Do not port them for MVP. Revisit only if we later commit to 1.18+ or enable C&C Part 1 experimentally — at that point the target has changed and this section should be reviewed.

### Disabled flags in `NoiseGeneratorSettings.overworld(...)`

File: `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java:241`. The `overworld(...)` factory passes `false` for all five C&C Part 1 booleans:

| Flag | Effect when `false` |
|---|---|
| `aquifers_enabled` | `Aquifer.createDisabled` is used; `barrier`/`waterLevel`/`lava` `NormalNoise` fields in `NoiseBasedChunkGenerator` are allocated but never sampled |
| `noise_caves_enabled` | `Cavifier` is replaced by `NoiseModifier.PASSTHROUGH` |
| `deepslate_enabled` | `DepthBasedReplacingBaseStoneSource` skips deepslate substitution |
| `ore_veins_enabled` | `OreVeinifier.fillStream` short-circuits |
| `noodle_caves_enabled` | all four `NoodleCavifier.fill*NoiseColumn` methods short-circuit |

### Dead code in our target — do not port

Transitively, the following classes exist in the 1.17.1 decomp but are never exercised in vanilla overworld:

- `net.minecraft.world.level.levelgen.Aquifer`
- `net.minecraft.world.level.levelgen.Cavifier`
- `net.minecraft.world.level.levelgen.NoodleCavifier`
- `net.minecraft.world.level.levelgen.OreVeinifier`
- `net.minecraft.world.level.levelgen.synth.NormalNoise` — only live consumers are the classes above, plus `GeodeFeature` (post-MVP feature) and `MultiNoiseBiomeSource` (nether/end — overworld uses the layered `OverworldBiomeSource` path)
- `net.minecraft.world.level.levelgen.synth.NoiseUtils` — only consumers are `Cavifier` / `NoodleCavifier`
- the deepslate path in `DepthBasedReplacingBaseStoneSource`

If asked to port any of these, push back and confirm the target has changed before writing code.

### Active MVP pipeline

Per [`docs/tactical/README.md`](docs/tactical/README.md): PRNG → octaved noise (`PerlinNoise`, `SimplexNoise`, `BlendedNoise`) → surface-path noise (`PerlinSimplexNoise` + `SurfaceNoise` interface) → `NoiseSampler` (with `NoiseModifier.PASSTHROUGH` since `Cavifier` is disabled) → `NoiseBasedChunkGenerator` (terrain only) → classic carvers (`CaveWorldCarver`, `CanyonWorldCarver`) → surface rules → features. Everything after classic carvers is post-MVP polish.
