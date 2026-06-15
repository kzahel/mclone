# Movement4 - Sprint and auto-jump polish

Standing after [`Movement3-basic-player-movement-integration.md`](Movement3-basic-player-movement-integration.md). The live path already sends sequenced movement commands through shared host/client movement simulation; this slice adds two player-facing input affordances without changing the authoritative command protocol.

## Goal

Add hold-to-sprint player input and an options-backed auto-jump toggle.

At the end of `Movement4`, `mclone` should:

- map Shift in player mode to the sprint movement button
- keep free-camera Shift behavior unchanged
- expose Auto-Jump in the WebGPU options screen
- persist Auto-Jump alongside the existing browser options
- emit auto-jump as a normal jump edge in the existing movement command stream
- keep host authority and client prediction consuming identical command records

## Source Review

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Options.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/Input.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/KeyboardInput.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java`

Vanilla keeps `Options.autoJump = true`, reads jump and shift in `KeyboardInput`, and uses `LocalPlayer.updateAutoJump(...)` plus `autoJumpTime` to turn a nearby obstacle into a one-tick jump. Vanilla's default sprint key is Control, but this slice intentionally maps player-mode Shift to sprint for the current debug controls.

## Compatibility Choices

Auto-jump is a client option, but the server should not infer it from local-only settings. The browser detects the jump opportunity against the same client prediction collision view and sends a normal jump edge. That preserves the existing Movement1-Movement3 invariant: both prediction and host authority simulate the same ordered commands.

The auto-jump probe is deliberately narrow:

- grounded standing body only
- no manual jump or crouch
- full-block collision boxes from the current collision world
- obstacles above step height and up to the vanilla 1.2-block auto-jump ceiling
- clear headroom required at the landing probe

## Validation

Minimum:

- `pnpm test -- test/runtime/movement/movement-step.test.ts test/renderer/debug-player-controls.test.ts test/renderer/browser-render-config.test.ts test/client/gui/gui-foundation.test.ts`
- `pnpm typecheck`
- `git diff --check`

Browser validation should use the existing title/options probe when available because the options screen gains a visible control.
