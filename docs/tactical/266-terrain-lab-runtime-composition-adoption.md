# Tactical 266: Terrain Lab runtime composition adoption

## Status

Active; Slices 0-2 are complete and Slice 3 is at the hosted human-review
gate. This is the bounded PH-3 child of
[Tactical 261](261-procedural-horizon-product-integration-roadmap.md),
following the accepted runtime composition and focus-anchor work recorded by
[Tactical 262](262-world-explorer-exact-procedural-composition.md).

## Objective

Add the proven exact-plus-procedural runtime presentation to Terrain Lab as a
reusable consumer of shared Rust ownership. The Lab must expose a subjective
browser review surface without turning World Explorer app code into an engine
dependency or replacing the Lab's existing research panes.

## Binding decisions

- Terrain Lab gains an optional `runtime` pane labelled **Runtime composed**.
- The existing `canonical`, `cpu`, `macro`, and `gpu` panes and their default
  selection remain available and retain their current meanings.
- The runtime pane uses the same focus-centered exact anchor, shared depth
  convention, exact coverage collar, whole-tree ownership, and procedural
  horizon presentation accepted in Tactical 262.
- The exact runtime renderer/session moves out of the World Explorer app and
  into a shared engine owner before Terrain Lab consumes it.
- Browser code owns canvas, `requestAnimationFrame`, Web Worker construction,
  and URL/UI glue only. Compilation, admission, coverage, ownership, and
  composition policy remain in Rust.
- The runtime pane follows the Lab's shared world-view navigation state. It
  does not introduce a second camera or a pane-local orbit target.
- Existing Terrain Lab canonical research infrastructure remains in place.
  Adoption does not require replacing or redesigning that pane.

## Non-goals

- Integrating the presentation into the live game scene or XR frame driver.
- Removing or consolidating existing Terrain Lab comparison panes.
- Fixing the accepted minor z-fighting at the outermost exact blocks. That
  remains a known seam/skirt follow-up.
- Changing the exact terrain radius or making camera-eye anchoring the default.
- Moving rendering or world ownership policy into React.

## Slices

### Slice 0: shared runtime exact owner

- Extract the exact renderer, coverage/admission state, separated natural-tree
  ownership, and executor contract from `mclone-world-explorer`.
- Keep native and browser transport construction at platform/app rims.
- Adapt World Explorer to the shared owner with no user-observable change.
- Lock the boundary with tests and validate native plus Wasm compilation.

Completed in the first implementation commit:

- `TerrainRuntimeExactRenderer`, its diagnostics, canonical request/result
  contract, and `CanonicalExactExecutor` now live in `mclone-terrain-view`.
- `TerrainRuntimeSession` and its focus/viewer-forward composition projection
  policy now live beside the shared horizon renderer, so consumers do not
  depend on World Explorer to obtain the accepted camera/depth alignment.
- World Explorer keeps only its native/browser transport adapters and aliases
  the shared runtime types for compatibility.
- The domain-neutral browser exact executor and vegetation executor/actor are
  shared as well. Consumer packages retain asset loading, Worker construction,
  and their thin exported exact Worker actor.
- The shared crate's 73 non-GPU unit tests pass, the new Explorer ownership
  lock passes, and both the native Explorer and its Wasm library compile.

### Slice 1: Terrain Lab runtime composition host

- Add a Terrain Lab Wasm host owning the shared exact renderer and procedural
  horizon renderer on one color/depth target.
- Connect exact and vegetation browser Workers through the shared executor
  contracts.
- Feed both representations the same source identity, focus anchor, camera,
  projection, viewport, and texture/color presentation.

Completed:

- `TerrainRuntimeCompositionLab` owns one browser surface, shared runtime
  session, shared exact renderer, and shared exact/vegetation executors.
- It renders both sources against the exact renderer's reversed-Z target,
  applying painted-coverage discard and whole-tree ownership before drawing.
- The main host and exact Worker load the same selected Lab visual profile.

### Slice 2: optional Lab consumer

- Add the `runtime` pane to URL state and pane controls.
- Add a React canvas adapter that supplies the existing shared navigation state
  and reports composition readiness/diagnostics.
- Preserve current pane defaults and existing comparison behavior.

Completed:

- `panes=runtime` selects **Runtime composed**; the four prior pane names remain
  valid and the default remains `canonical,cpu,gpu`.
- The pane uses the existing shared Lab navigation hook and publishes
  readiness, exact coverage, and tree-ownership facts without reconstructing
  policy in TypeScript.
- Static `reviewYaw` and `reviewPitch` parameters make a deterministic review
  link reloadable while ordinary camera motion remains outside terrain state.

### Slice 3: browser evidence and checkpoint

- Run focused Rust, Wasm, TypeScript, unit, and browser smoke validation.
- Capture and inspect headed WebGPU pixels at the first drawable milestone and
  again after Worker completion.
- Deploy the hosted Terrain Lab and provide a fixed query URL showing the
  runtime pane for subjective review.

Local objective evidence passes:

- the 73-test shared terrain-view suite;
- Terrain Lab's five browser-ownership locks and 16 URL/state tests;
- Terrain Lab Wasm compilation, TypeScript checking, and production build;
- the headed WebGPU host capability probe; and
- desktop runtime smoke at seed `12345`, focus `(0, 0)`,
  `blocksAcross=96`, yaw `3.1415927`, pitch `0.12`, and radius `2`, including
  `25/25` exact chunks, non-empty exact/proxy tree ownership, all `160` horizon
  slots, and a completed synchronized pan/rebuild.

The local headed Chrome screenshot remains the already-documented invalid
one-color white wgpu-canvas capture seen by World Explorer on this host. It is
not counted as pixel evidence. The hosted interactive review below remains the
subjective pixel gate.

## Human review checkpoint

The checkpoint is ready when the hosted Terrain Lab can show the runtime pane
at a deterministic seed, focus, scale, and perspective. Review should verify:

1. exact terrain is centered on the visible orbit focus;
2. exact terrain, hills, and trees correctly depth-occlude the surrounding LOD;
3. trees cross the exact/approximate ownership boundary as whole objects;
4. orbit, pan, and zoom remain synchronized with any adjacent Lab panes; and
5. only the already-recorded outermost-block z-fighting is visible at the seam.

## Exit condition

PH-3 is complete when Terrain Lab consumes the shared runtime composition
owner, the existing pane set remains intact, automated browser evidence is
green, and the hosted checkpoint is suitable for human acceptance before live
game/XR integration.
