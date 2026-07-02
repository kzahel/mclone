# Vanilla Client Screenshot Oracle

Status: active first slice landed.

Workstream: oracle/reference only.

## Goal

Produce deterministic Minecraft Java 1.17.1 client screenshots for visual parity checks against native captures, without relying on Prism Launcher UI automation or a mod loader. The preferred path is a repo-owned Java harness against `reference/minecraft-1.17.1/client-deobf.jar`, using official assets/libraries and a local/offline session for singleplayer rendering.

## Reference Baseline

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/main/Main.java`: vanilla launch arguments and `GameConfig` construction. `--accessToken` and `--version` are syntactically required; local screenshot work can use a dummy offline token because no online service path is needed.
- `reference/minecraft-1.17.1/src/net/minecraft/client/Screenshot.java`: vanilla framebuffer readback path through `Screenshot.takeScreenshot(...)` / `Screenshot.grab(...)`.

## Constraints

- Do not automate Prism or click through launcher/game menus.
- Do not introduce Fabric/Forge unless the direct deobfuscated-client harness proves materially worse.
- Keep Prism as an optional source of known-good local assets only; it should not be in the normal oracle command path.
- Keep screenshots and temporary game dirs under `/tmp` or gitignored `reference/` runtime caches.
- Mac launch must respect LWJGL's first-thread requirement through `-XstartOnFirstThread`.

## First Slice

Landed scaffold:

- `oracle/java/VanillaClientLauncher.java` builds vanilla `Main` arguments from `--mclone-*` harness options, can print the exact argument vector, and can explicitly delegate to `net.minecraft.client.main.Main.main(...)`.
- `oracle/vanilla-client-launch.mjs` builds the host-filtered client classpath from Mojang's `1.17.1.json`, downloads/extracts native classifiers and asset objects on `--hydrate` / `--launch`, and prints or runs the Java command.
- The default command prints the launch command only. `--launch` is intentionally explicit so validation does not open a window by accident.

Smoke commands:

```bash
node oracle/vanilla-client-launch.mjs --print-command
node oracle/vanilla-client-launch.mjs --print-args
node oracle/vanilla-client-launch.mjs --hydrate
node oracle/vanilla-client-launch.mjs --launch
```

## Mac Risks

This Mac is `arm64`, while Minecraft 1.17.1's official macOS LWJGL native classifiers predate the modern Apple Silicon launcher path. The first visible launch may need either an x86_64/Rosetta JDK or a narrow LWJGL-native substitution. Treat that as a launch-runtime compatibility issue, not a renderer-oracle design change.

## Next Slices

1. Hydrate official assets/natives and prove a visible vanilla 1.17.1 client launch from the repo command on this Mac.
2. Add a harness lifecycle hook that creates/loads a fixed singleplayer world, sets seed/time/weather/options, waits for target chunks, captures `Screenshot.takeScreenshot(minecraft.getMainRenderTarget())`, writes a named PNG under `/tmp`, and exits.
3. Try a hidden or unfocused GLFW window after visible launch works; keep a tiny visible window as fallback if macOS rejects hidden rendering.
4. Add camera-position arguments matching native `--view-pose` and a paired native/vanilla comparison script for seed/position probes.
