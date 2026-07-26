# Tactical 253: World Explorer Cross-Host Parity

Status: completed 2026-07-26. Tacticals 254–256 closed the UI, color-output,
and off-thread vegetation parity gaps on native and browser hosts.

Topics:

- `world-view-navigation`
- `platform-host-boundary`
- `procedural-horizon-clipmap`

Parent directions:

- [`world-view-navigation.md`](../topics/world-view-navigation.md) owns the
  lightweight Explorer product and shared navigation;
- [`platform-host-boundary.md`](../topics/platform-host-boundary.md) owns the
  thin browser/native host rim and Rust-owned visible UI;
- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns the shared terrain and vegetation horizon; and
- [`249`](249-cross-platform-procedural-horizon-proof.md) completed the first
  native/browser proof while explicitly deferring browser vegetation work.

## Objective

Close three cross-host differences exposed by side-by-side review of the
standalone World Explorer without turning the smoke product into a full game
client:

1. browser-only HTML/CSS diagnostics;
2. surface-format-dependent color output; and
3. native-only procedural tree proxies.

These are one parity campaign but three different implementation concerns.
This parent records their order and common acceptance requirements. Each child
owns one bounded change.

## Originating Direction

The Explorer should remain UI-less until menus or player-facing controls are
designed deliberately. Browser HTML/CSS must not become a temporary product UI
stack. The current darker browser terrain appearance is the desired baseline.

Tree parity requires more care. Browser vegetation must not be restored by
running its synchronous compiler on the animation thread, and native must not
retain a privileged render-thread topology. The later fix must use one shared
Rust coordinator and equivalent native-thread/browser-Worker execution
topology, reusing the project's established opaque Worker and external
`SharedArrayBuffer` patterns.

## Child Tacticals

| Tactical | Concern | Desired result |
|---|---|---|
| [`254`](254-ui-less-world-explorer-host.md) | visible diagnostics | Terrain-only content area on both hosts; browser DOM is startup/fatal and platform mechanics only |
| [`255`](255-world-explorer-color-output-parity.md) | color transfer | Dark direct-display appearance remains stable across sRGB and non-sRGB targets |
| [`256`](256-shared-horizon-vegetation-worker-topology.md) | tree compilation | One shared coordinator with native-thread and browser-Worker executors; matching tree identity and presentation |

## Sequence

1. Remove the browser-only visible overlay and isolate smoke diagnostics.
2. Make the current darker browser appearance an explicit shared output
   contract, then compare equivalent native and browser captures.
3. Audit the existing Terrain Lab canonical coordinator, full-game render
   coordinator, server-job mailbox, and generic browser Worker transport.
4. Extract procedural vegetation compilation from frame encoding behind one
   shared coordinator.
5. Prove native threaded execution before enabling the equivalent browser
   Worker path.
6. Require matching tree identity/count diagnostics and inspected pixels.

Tacticals 254 and 255 may complete before 256's architecture checkpoint.
Exact/procedural composition should not treat the current browser omissions as
an acceptable baseline.

## Progress

Tactical [`254`](254-ui-less-world-explorer-host.md) completed on 2026-07-26.
The ordinary browser host now presents only the terrain canvas after startup,
returns no semantic frame report, and installs no diagnostic global. An
explicit query-gated smoke observer reads coherent Rust-authored frame
snapshots, while desktop/mobile headed-browser and native/offscreen captures
prove unobstructed terrain. Tactical
[`255`](255-world-explorer-color-output-parity.md) completed on 2026-07-26.
The explicit `vanilla` display-space profile now preserves the selected dark
appearance across browser UNORM and native/offscreen sRGB targets. A
lightweight shared color crate owns the CPU and WGSL transfer definition,
format-pair GPU evidence passes, and pinned browser/native captures satisfy
the documented color tolerance. Tactical
[`256`](256-shared-horizon-vegetation-worker-topology.md) completed on
2026-07-26. Its reusable
`mclone-terrain-view` vegetation streaming coordinator over a shared
`mclone-worldgen` compiler session, with World Explorer as the first
native/browser proof rather than the final owner, now drives both products
without synchronous frame compilation. A pinned native/offscreen/desktop-
browser/phone-browser receipt agreed on source fingerprint, stable record
hash, family counts, `2,609` tree instances, and `281,772` proxy vertices.
Forced browser overflow, one Worker reconstruction, movement, negative and
million-block coordinates, and graceful shutdown passed. Later game-scene
adoption remains a separate tactical.

## Campaign Acceptance

- Native and browser content areas show the same terrain-only product surface
  for a pinned view.
- Ordinary browser JavaScript contains no Explorer diagnostic formatting,
  menu layout, color policy, terrain policy, or vegetation job policy.
- Equivalent native, browser, and offscreen captures use an explicit shared
  color-output contract and retain the selected dark appearance.
- Vegetation enablement is a product/session decision, not a hardcoded target
  test.
- No ordinary native or browser frame performs synchronous vegetation-plan
  compilation.
- Shared diagnostics can prove equivalent tree source identity, admitted
  records, instances, and final readiness.
- The standalone dependency firewall remains small. Any new shared dependency
  is justified explicitly rather than importing the full game runtime.

## Non-Goals

- Designing Explorer menus, player-facing map controls, or an Enter Here flow.
- Importing `mclone-scene`, the full game web shell, or the complete game
  runtime.
- Solving exact/procedural masking, the deferred clipmap seams in Tactical
  [`252`](252-procedural-horizon-seams-and-transition-admission.md), or
  vegetation edit persistence.
- Requiring identical OS window chrome or browser chrome.
