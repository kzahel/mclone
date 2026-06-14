See [`README.md`](README.md) for project context.

## Notes for agents

**Do not use the auto-memory system** for this project (the `~/.claude/projects/-home-kgraehl-code-mclone/memory/` directory). Persist project-relevant guidance in this file (`AGENTS.md`) instead.

For the native Rust rewrite, use the sibling engine at `~/code/playbox` as a reference for mature `winit`/`wgpu`, headless capture, diagnostics, and future desktop OpenXR patterns. Treat it as a pattern library only; do not depend on it directly, and do not copy its PhysX/VaM-specific runtime shape.

Native Rust rewrite tactical docs live under `docs/tactical/native/` and use zero-padded numeric filenames such as `000-topic.md`, `001-next-topic.md`. Do not add new native workstream tacticals beside the legacy TypeScript/browser tacticals in `docs/tactical/`.

## Porting methodology — applies to every slice, worldgen and renderer

Every class we port has a 1:1 counterpart in `reference/minecraft-1.17.1/src/`. **Always read the source file before writing the port.** The default is direct translation: same field names, same method names, same logic flow. Diverge only when the platform forces it.

### Sanctioned divergences (exhaustive list)

| Layer | Java | TypeScript / WebGPU replacement |
|---|---|---|
| Renderer only | `GlStateManager` + `Uniform` | WebGPU pipeline descriptors + bind groups |
| Renderer only | GLSL core shaders (`assets/minecraft/shaders/core/`) | WGSL equivalents |
| Language | `ByteBuffer`, Java primitives, `int` bit-twiddling | `DataView` / typed arrays; preserve byte layout exactly |
| Language | Java generics / checked exceptions | TypeScript generics / unchecked throws |

Everything else — vertex formats, buffer builders, atlas stitching, model baking, chunk meshing, noise math, PRNG, biome layering — is a straight port. If you find yourself writing logic that isn't in the source, stop and re-read the source.

## Architectural choices and divergence review

Runtime architecture does not have to transliterate class-for-class from Minecraft, but architectural choices still need to be weighed against the reference source.

Before making an architectural divergence, explicitly determine:

1. What the reference Minecraft source does today
2. Why that shape is a poor fit for browser / WebGPU / worker / Node constraints
3. The exact scope of the divergence
4. Whether the divergence makes future parity work easier, neutral, or harder

Do not make architectural changes on the basis of convenience alone. Prefer the reference shape when it still fits. When diverging, keep the divergence intentional, narrow, and documented.

The main rule is:

- simulation/content parity is the default
- runtime orchestration may diverge when platform constraints require it
- any such divergence must preserve a clear path for future parity work instead of making it opaque or harder to recover

### Callout convention

When a method diverges from its Java counterpart, add a one-line comment:
```ts
// WebGPU: GPUBuffer instead of GL VAO/VBO/IBO
```
Straight ports get no comment.

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

## Visual validation — fail fast, don't batch

For any slice that produces pixels, **capture a screenshot and look at it before moving on.** Do not finish a whole slice and then check. Check at the first drawable milestone — even a solid-color quad or a clear-color frame — then keep checking as complexity increases.

### Host capability check

Before choosing browser/WebGPU validation on an unfamiliar host, run `pnpm host:check`. The script reports whether the host has a display session, visible GPU devices, Chrome/Chromium, Playwright, and which validation lanes are expected to work.

- On a headless Linux host with no `DISPLAY` / `WAYLAND_DISPLAY`, Chrome/WebGPU Playwright lanes are expected to be unavailable. Do not treat failures from `pnpm test:browser`, `pnpm test:browser:integration`, or `pnpm probe:browser ...` as renderer regressions until they reproduce on a host with a working Chrome GPU/browser path.
- On this Linux Wayland host, the shell may have `WAYLAND_DISPLAY` unset even though `$XDG_RUNTIME_DIR/wayland-0` exists. For browser GPU validation here, run Playwright headed with an explicit Wayland display and a clean Vite port:
  - Browser smoke: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.config.ts --headed`
  - Browser integration: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.integration.config.ts --headed`
  - Browser probe: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.probes.config.ts --headed test/browser/probes/<name>.probe.ts`
  - `CI=1` prevents Playwright from reusing an unrelated dev server, and `VITE_PORT=5073` avoids the user shell's `VITE_PORT=3402` override. Plain `pnpm test:browser` can reuse the wrong app on port 3402.
  - Headless Chrome on this host can fail WebGPU with `Instance dropped in popErrorScope` and black screenshots even when headed Wayland Chrome renders correctly. Prefer the headed Wayland command for browser-GPU validation; use Deno smokes as the headless WebGPU control.
- On macOS, keep browser screenshot probes on Playwright's Chrome channel and launch with `--enable-unsafe-webgpu --use-angle=metal`. Playwright's bundled Chromium can expose `navigator.gpu` and pass offscreen WebGPU readback while still screenshotting presented WebGPU canvases as black in headless mode unless `--use-angle=metal` is present. The shared Playwright config and smoke scripts include this default.
- On that kind of host, prefer display-independent validation: `pnpm test`, `pnpm typecheck`, Node headless host tests, and focused Deno WebGPU smokes such as `pnpm smoke:deno:webgpu`, `pnpm smoke:deno:pipeline`, `pnpm smoke:deno:world-assets`, or `pnpm smoke:deno:generated-world`.
- To run an actual Deno WebGPU capability probe through the checker, use `pnpm host:check -- --probe-deno-webgpu`. To confirm a Chrome/WebGPU browser path, use `pnpm host:check -- --probe-browser-webgpu`; the browser probe uses a localhost secure context and verifies both offscreen GPU readback and presented WebGPU canvas pixels in a screenshot, so it catches black-canvas headless failures.

Concretely:
- `pnpm test:browser` is the fast automatic WebGPU smoke lane. Run it for browser boot, WebGPU setup, remote-host smoke, and changes that could break the default browser entrypoint.
- `pnpm test:browser:integration` is the slower automatic browser integration lane. Run it when touching remote player state, debug camera controls, resize/backing-buffer logic, or chunk-interest movement.
- `pnpm probe:browser -- test/browser/probes/<name>.probe.ts` is the screenshot lane for human visual inspection. Run the smallest relevant probe when a change affects rendered pixels, terrain appearance, surface materials, vegetation, or camera framing. Do not run the full probe suite unless the user asks or the change broadly affects visual output.
- After wiring up a new draw path, run the smallest browser command that reaches the new path, capture a screenshot, read the image, and describe what you see. Does it look right? Are the shapes, colors, and positions what you expect?
- If the output looks wrong (blank canvas, wrong color, garbage geometry, WebGPU validation errors in the console), stop and fix it before adding more code on top.
- Unit tests (`pnpm test`) catch data-structure correctness. They do not catch GPU submission errors, wrong buffer layouts, or misconfigured pipelines. Actually looking at the rendered output is the only way to catch those.
- **Always save debug, smoke, and probe screenshots to `/tmp`** (e.g. `/tmp/mclone-debug-freecam.png`). Never write screenshots into the repo, into `test-results/`, or anywhere that risks getting committed. `/tmp` is also clickable in the chat UI per the global file-path convention.

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
