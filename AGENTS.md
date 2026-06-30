See [`README.md`](README.md) for project context.

## Windows: shell and Rust toolchain

This repo is developed on Windows. Run `cargo`, `pnpm`, and any native build/test/run from **PowerShell** (or the Git Bash / MINGW64 shell) — both use the native Windows Rust toolchain at `C:\Users\<user>\.cargo\bin\cargo.exe` and work correctly (clean `cargo build`/`cargo test`, including MSVC linking and test-binary execution, all pass).

**Do not run cargo from WSL.** WSL is installed and running on this machine, but it has **no Rust toolchain** (`cargo: command not found`), and even with one it builds over the slow `/mnt/c` mount, which hangs and targets Linux instead of Windows. If a `cargo`/`pnpm` command "gets stuck" or reports `command not found`, you are almost certainly in WSL, not Git Bash — switch shells rather than trying to fix the build.

The `.sh` setup scripts (`scripts/decompile-mc.sh`, `scripts/extract-assets.sh`, `oracle/build.sh`, `oracle/run.sh`, etc.) require bash; run those from Git Bash, not PowerShell. The `pnpm` cargo wrappers in `package.json` are shell-agnostic and run fine from either PowerShell or Git Bash.

## Notes for agents

**Do not use the auto-memory system** for this project (the `~/.claude/projects/-home-kgraehl-code-mclone/memory/` directory). Persist project-relevant guidance in this file (`AGENTS.md`) instead.

For the native Rust rewrite, use the sibling engine at `~/code/playbox` as a reference for mature `winit`/`wgpu`, headless capture, diagnostics, Android, and OpenXR patterns. Treat it as a pattern library only; do not depend on it directly, and do not copy its PhysX/VaM-specific runtime shape. When the user mentions "Playbox", inspect that sibling repo directly; useful entry points are `~/code/playbox/Cargo.toml` for debug-profile optimization policy, `~/code/playbox/docs/architecture/rendering.md` for render target/view boundaries, `~/code/playbox/docs/architecture/platforms.md` for desktop/Android/OpenXR platform shape, `~/code/playbox/android/README.md` for flat Android, and `~/code/playbox/android-xr/README.md` for Quest/OpenXR packaging and validation notes.

For vanilla gameplay, assets, rendering semantics, and visual correctness, treat `reference/minecraft-1.17.1/src/` as an equally important and often more authoritative reference than Playbox. Use the Java client source first for behavior that exists in Minecraft itself: block/entity model baking, texture atlas stitching, mipmap generation and sampler filtering, UV shrink/bleed rules, light texture math, fog, sky, particles, render layers, transparency/cutout choices, chunk render-section traversal, and any renderer-facing state that affects vanilla appearance. Use Playbox for native `wgpu`/platform mechanics after the Java behavior is understood.

Current target posture: five client/platform lanes are validated at the basic gameplay/rendering level: desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM. Native desktop flat remains the fastest day-to-day bring-up and validation path, but it is not the default feature target. For any feature request without an explicit platform constraint, phrase and design the work as **shared implementation, desktop validation first**. Do not let shared engine, client, server, mesh, asset, renderer, UI, or runtime contracts become desktop-only. Treat code that heavily jumps into `mclone-native-client` as a cleanup smell unless it is genuinely `winit`, desktop surface, native input, CLI, headless capture, or desktop diagnostics glue. Treat client platform and server host mode as separate axes: every client lane must retain a path to dedicated-server play, and future P2P/session topologies must fit behind the same shared command/update contracts rather than becoming platform forks. Keep `winit`, Android activity glue, browser glue, and OpenXR session/swapchain ownership in app/platform adapters. Keep renderer-facing view/projection and render-target data explicit, and keep headless/offscreen validation available. Desktop XR and Android XR share OpenXR host/graphics/scene boundaries where practical; do not fork gameplay, runtime, meshing, asset, or renderer internals for a single platform.

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

## Shared-first feature policy

Before implementing any new user-facing engine or game feature, identify the
shared owner first. App crates may own platform glue: OS/browser/Android/XR
events, window/surface/session/swapchain ownership, platform storage adapters,
native socket/browser transport setup, and lifecycle wiring. They should not own
gameplay, rendering semantics, persistence rules, input semantics, UI behavior,
or other engine policy just because one target needs the feature first.

Do not start with a desktop-only, web-only, Android-only, or XR-only gameplay
implementation unless the behavior is genuinely platform-specific. If no shared
owner exists yet, create or extend the shared crate/contract first, or record an
explicit tactical explaining the temporary platform-local exception and the path
back to a shared boundary.

Prefer reducing desktop app gravity before adding more desktop-local behavior.
When a change touches existing `mclone-native-client` code, first ask whether
that code belongs in `mclone-app-runtime`, `mclone-render-session`,
`mclone-render`, `mclone-input`, `mclone-ui`, `mclone-client`, or
`mclone-server`. If the answer is yes, move or extend the shared owner as part
of the slice instead of making the desktop app a larger staging area.

Default shared routing:

- input/capabilities/bindings: `mclone-input`
- UI/HUD/widgets/text model: `mclone-ui`
- audio/sound events/mixing: `mclone-audio`
- protocol/transport/session commands: `mclone-protocol`, `mclone-net`, `mclone-app-runtime`
- gameplay/client replica/prediction: `mclone-client`
- authoritative simulation/world state: `mclone-server`
- assets/content loading: `mclone-assets`
- rendering contracts: `mclone-render-session`, `mclone-render`
- XR pose/session/swapchain specifics: `mclone-xr-*`
- persistence, diagnostics/profiling, job scheduling, preferences/config, text
  input/IME/clipboard, content registries, inventory/items/crafting, particles
  and transient effects, entity AI/spawning, localization: define or extend a
  shared contract before adding app-local behavior.

## XR render-path guardrail

World-space or per-view visual features must be multiview-aware. Prefer routing
new debug visuals, overlays, outlines, actors, screen effects, and world UI
through existing renderers that already support single-view/per-eye and XR
multiview paths. If a new renderer, shader, uniform, render pass, or command path
is added, it must either:

- implement both the normal per-eye path and the full-frame multiview path, or
- document why the feature is intentionally unavailable in one path.

Do not land a new XR-visible render feature that only appears in per-eye
rendering when it should also appear in full-frame multiview. For per-view data,
preserve the invariant that each eye/layer uses its own view/projection data;
never share mutable per-eye uniforms across one submission.

## Reference-porting policy

For vanilla parity ports, every class or system has a 1:1 counterpart in `reference/minecraft-1.17.1/src/`. **Always read the source file before writing the port.** The default is direct translation: same field names where practical, same method names where practical, same logic flow. This applies to client graphics behavior as well as simulation and content systems. Diverge only when the target platform, runtime ownership, or Rust type system forces it.

The main rule is:

- simulation/content/vanilla visual behavior parity is the default
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

If a required native headless render/screenshot fails because `wgpu` cannot see a GPU adapter, rerun the same command with elevation before treating GPU validation as blocked.

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
