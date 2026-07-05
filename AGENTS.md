See [`README.md`](README.md) for project context.

## Windows: shell and Rust toolchain

This repo is developed on both macOS and Windows. macOS is the usual day-to-day host; switch to Windows only when it is specifically needed — Windows has the better native desktop XR runtime (the macOS desktop-XR lane is a self-ported WiVRn convenience path), and some validation (MSVC/Vulkan/DX12, Windows-only APIs) can only run there. Host switches are not free: after switching, bring the asset pipelines up to date and run a smoke/toolchain preparation pass before trusting results, and prefer batching Windows-only work so hosts switch rarely. When on Windows, run `cargo`, `pnpm`, and any native build/test/run from **PowerShell** (or the Git Bash / MINGW64 shell) — both use the native Windows Rust toolchain at `C:\Users\<user>\.cargo\bin\cargo.exe` and work correctly (clean `cargo build`/`cargo test`, including MSVC linking and test-binary execution, all pass).

**Do not run cargo from WSL.** WSL is installed and running on this machine, but it has **no Rust toolchain** (`cargo: command not found`), and even with one it builds over the slow `/mnt/c` mount, which hangs and targets Linux instead of Windows. If a `cargo`/`pnpm` command "gets stuck" or reports `command not found`, you are almost certainly in WSL, not Git Bash — switch shells rather than trying to fix the build.

The `.sh` setup scripts (`scripts/decompile-mc.sh`, `scripts/extract-assets.sh`, `oracle/build.sh`, `oracle/run.sh`, etc.) require bash; run those from Git Bash, not PowerShell. The `pnpm` cargo wrappers in `package.json` are shell-agnostic and run fine from either PowerShell or Git Bash.

## Notes for agents

**Do not use the auto-memory system** for this project (the `~/.claude/projects/-home-kgraehl-code-mclone/memory/` directory). Persist project-relevant guidance in this file (`AGENTS.md`) instead.

Background docs own the long-form project context:

- [`docs/platforms.md`](docs/platforms.md) owns the current platform posture, Playbox reference entry points, platform boundaries, and validation matrix.
- [`docs/native-engine-architecture.md`](docs/native-engine-architecture.md) owns the current native engine architecture and shared crate/app ownership shape.
- [`docs/reference-minecraft.md`](docs/reference-minecraft.md) owns the Minecraft 1.17.1 reference tree, bootstrap/mapping notes, vanilla target, and disabled Caves & Cliffs Part 1 systems.
- [`docs/native-web.md`](docs/native-web.md) owns Rust/WASM web build, smoke, deploy, and local deploy-hook notes.

Agent guardrails:

- Use `~/code/playbox` as a native `winit`/`wgpu`/headless/Android/OpenXR pattern library only. Do not depend on it directly or copy its PhysX/VaM-specific runtime shape. When the user mentions "Playbox", inspect that sibling repo directly.
- Use `reference/minecraft-1.17.1/src/` before Playbox for vanilla gameplay, assets, rendering semantics, and visual correctness.
- For feature requests without an explicit platform constraint, use **shared implementation, desktop validation first**. Keep gameplay, runtime, asset, mesh, UI, renderer, and XR contracts host-neutral.
- Treat code that heavily grows `mclone-native-client` as a cleanup smell unless it is genuinely `winit`, desktop surface, native input, CLI, headless capture, or desktop diagnostics glue.
- Keep `winit`, Android activity glue, browser glue, and OpenXR session/swapchain ownership in app/platform adapters.

For web/WASM, keep `wasm32-unknown-unknown` as the browser target unless a tactical explicitly changes it, but do not treat that as permission for a single-threaded or reduced engine architecture. Browser CPU work should converge on the same job/worker lifecycle as desktop: desktop uses native OS threads, while browser/WASM uses Web Workers with shared Wasm memory (`SharedArrayBuffer`/atomics) once that slice is implemented. Inline synchronous WASM paths are acceptable only as temporary smoke/fallback implementations behind the same compiler/session interfaces, not as the target threading model.

The retired browser engine has been removed from the live tree. Git history is the archive for old implementation context. Retained oracle helpers under `oracle/lib/**` and shared fixture data under `test/fixtures/**` are active reference assets, not legacy engine code.

Native Rust tactical docs live under `docs/tactical/` and use zero-padded numeric filenames such as `000-topic.md`, `001-next-topic.md`. Historical legacy tacticals, if still present during cleanup, are not implementation guidance for new work.

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

Reference bootstrap details live in [`docs/reference-minecraft.md`](docs/reference-minecraft.md). Run `./scripts/decompile-mc.sh` if `reference/minecraft-1.17.1/src/` is missing. Do not write a port without the source in hand.

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

Use the current default and platform-specific validation commands in [`docs/platforms.md`](docs/platforms.md#validation-policy) rather than duplicating the command matrix here.

For rendered-output validation, use native headless captures where available and save debug, smoke, and probe screenshots to `/tmp` (for example `/tmp/mclone-native-debug.png`). Never write screenshots into the repo, into `test-results/`, or anywhere that risks getting committed.

## Target: Minecraft Java 1.17.1 vanilla overworld

Seed parity against 1.17.1 vanilla overworld is the correctness bar. Detailed target notes, active pipeline, and disabled Caves & Cliffs Part 1 systems live in [`docs/reference-minecraft.md`](docs/reference-minecraft.md).

Do not port `Aquifer`, `Cavifier`, `NoodleCavifier`, `OreVeinifier`, the disabled deepslate path, or other disabled Caves & Cliffs Part 1 worldgen paths for MVP. If asked to port any of these, push back and confirm the target has changed before writing code.
