# Tactical 262: World Explorer Exact/Procedural Composition

Status: active 2026-07-26. Slice 0 records the revised proof-first direction.
Implementation should stop at Human Review 1 after one true native
exact-plus-procedural frame is inspectable under movement and delayed exact
admission. Browser and Terrain Lab promotion remain later slices of this
tactical after that review.

Topic: `procedural-horizon-clipmap`

Parent:

- [`261`](261-procedural-horizon-product-integration-roadmap.md) is the global
  procedural-horizon parent. This tactical implements PH-1 and PH-2 before
  game-scene adoption.

Related directions:

- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns exact-painted coverage, masking, frontier treatment, and horizon
  residency;
- [`world-view-navigation.md`](../topics/world-view-navigation.md) already
  sequences a reusable exact-view source and exact/procedural Explorer
  composition before the player-facing map-to-world view;
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) owns the
  Terrain Lab comparison evidence and source-quality work; and
- [`244`](244-lod-native-vegetation-presentation.md) is now the completed
  footprint-summary and vegetation-representation proof rather than an open
  composition owner.

## Objective

Prove the missing reusable compositor before introducing full game lifecycle:

```text
locally compiled exact near field
              |
              v
renderer-neutral exact-painted snapshot
              |
              +------> fixed exact coverage mask
              |                 |
procedural clipmap -------------+
              |
              v
one color target + one reversed-Z depth target
              |
              v
Horizon | Exact | Composed | Coverage diagnostic
```

World Explorer must render a bounded exact near field and the existing
procedural horizon through one device, encoder, color target, depth target,
camera, and source identity. `Composed` is the acceptance mode. A side-by-side
comparison is useful diagnostics but cannot substitute for real masking,
shared depth, and frontier geometry.

The proof must produce shared contracts that the game can consume later. It
must not import `mclone-scene`, the client/server runtime, React, Terrain Lab
UI policy, or an app-local copy of exact/procedural arbitration.

## Why World Explorer First

World Explorer already owns the newest proven horizon session:

- fixed ten-level toroidal residency;
- requested/staged/committed terrain and vegetation presentations;
- native-thread and browser-Worker vegetation execution;
- deterministic native/offscreen/browser movement receipts;
- shared view controls; and
- a deliberately small dependency firewall.

Terrain Lab already owns the mature canonical comparison path, but its exact
renderer and coordinator are currently browser-app-local and use a separate
surface, device, depth target, and canvas from each procedural pane. A full
Lab renovation would mix reusable extraction with React layout and
browser-only lifecycle.

The useful Terrain Lab work in this tactical is narrow: extract its canonical
mesh compiler/session and exact-painted readiness facts into a reusable Rust
boundary. World Explorer then proves the compositor. A later slice may feed
the same runtime-horizon/composed view back into Terrain Lab without replacing
its CPU/GPU/reference research panes.

The ordinary game already has the production exact renderer, but adopting the
horizon there first would combine the unproven compositor with world
lifecycle, settings, render-section scheduling, browser/Android construction,
and XR admission. The game follows only after the smaller proof is accepted.

## Binding Contracts

### Exact-painted means drawable now

An exact chunk enters `ExactPaintedCoverageSnapshot` only after every packed
opaque/cutout resource required by the proof renderer has been accepted for
the current source and generation. Requested, generated, compiled, resident,
or queued chunks do not mask procedural terrain.

The snapshot carries:

- terrain source identity;
- monotonically changing coverage generation;
- the exact painted chunk set; and
- bounded mask dimensions or an equivalent validated coverage encoding.

Late results from an older source, desired footprint, or generation are
rejected before upload or mask publication.

### One caller-owned target

The horizon renderer must support a caller-owned color and reversed-Z depth
target with explicit clear/load/store behavior. Its current convenience path
may retain an internal depth target for existing callers, but the composed
path cannot clear or replace depth between procedural and exact draws.

The first order is:

1. clear and draw masked procedural terrain plus eligible tree proxies;
2. load color/depth and draw exact opaque, cutout, and translucent sections;
3. retain enough depth for the existing capture proof; and
4. expose one coherent frame report.

Later game composition may split actors and translucent water into more
precise phases. This proof must not make that later ordering harder.

### Coverage mask, not depth arbitration

The procedural fragment path maps world X/Z to exact chunk coordinates and
consults a bounded GPU mask. In `Composed` mode it discards painted chunks. In
coverage diagnostics it visualizes the same ownership without changing which
chunks the CPU considers painted.

Depth testing is still required, but it is not ownership. Approximate terrain
may otherwise cover exact geometry or z-fight.

### Explicit frontier treatment

The first exact near field retains a narrow boundary wall, skirt, or collar
where exact and approximate heights can disagree. The implementation must
name and diagnose that treatment rather than hiding cracks with accidental
overdraw.

### Shared source, host-specific production

The proof exact source may compile canonical chunks locally. The later game
produces exact sections from `ClientRuntime`. Both publish the same
renderer-neutral exact-painted snapshot and consume the same coverage/mask
contract.

Canonical generation and mesh preparation may be reused from Terrain Lab.
React scheduling, Lab URLs, browser canvas ownership, and the Lab-specific
coordinator are not reusable engine policy.

## Diagnostic Modes

World Explorer exposes these Rust-owned modes through native hotkeys and
query/raw-key browser mechanics:

| Mode | Required result |
|---|---|
| `Horizon` | Existing procedural-only result, byte/pixel compatible except for intentional target-contract refactoring. |
| `Exact` | Bounded canonical near field over the normal background with no procedural draw. |
| `Composed` | Procedural horizon outside the exact-painted mask and exact chunks inside it on one shared target. |
| `Coverage` | A conspicuous visualization of the same painted mask/frontier used by `Composed`. |

Suggested native bindings are `1` through `4`. The visible product remains
UI-less; title/diagnostics may name the mode, and browser smoke may select it
through an explicit query parameter.

## Slice Plan

### Slice 0: parent correction and execution contract

- [x] Record the proof-first sequence in Tactical 261.
- [x] Close Tactical 244's stale representation-decision ledger.
- [x] Define the shared target, exact-painted, mask, frontier, and diagnostic
  contracts here.
- [x] Preserve game-scene and XR adoption as later parent phases.

Gate: implementation has one small proof owner and no host-local arbitration.

### Slice 1: reusable target and coverage substrate

- [ ] Add a platform-neutral exact-painted snapshot with source/generation
  checks and negative-coordinate mask packing tests.
- [ ] Let the horizon encode into caller-owned color/depth attachments with
  explicit clear/load/store behavior.
- [ ] Add a bounded exact-coverage GPU resource and procedural terrain/tree
  mask path.
- [ ] Preserve the existing Horizon convenience path, allocation report, and
  native/browser builds.

Gate: unit/offscreen tests prove the mask identity and one shared depth
attachment without exact generation.

### Slice 2: reusable canonical exact near field

- [ ] Move the canonical mesh compiler/session and packed batch codec out of
  Terrain Lab app-local ownership without changing its behavior.
- [ ] Add a bounded native threaded exact producer for World Explorer.
- [ ] Admit no more than one exact result per frame and retain valid overlap
  while the desired footprint moves.
- [ ] Publish exact-painted chunks only after accepted mesh upload.
- [ ] Retain explicit frontier walls/collar in the composed proof.

Gate: `Exact` mode moves through a bounded near field without presentation-
thread generation or stale uploads.

### Slice 3: composed World Explorer and Human Review 1

- [ ] Add `Horizon`, `Exact`, `Composed`, and `Coverage` modes.
- [ ] Align exact and procedural cameras, color transfer, source identity, and
  reversed-Z target.
- [ ] Exercise stationary, movement, negative coordinates, exact
  admission/eviction, deliberately delayed exact work, and teleport.
- [ ] Capture and inspect native window/offscreen pixels at the first composed
  frame and after movement.
- [ ] Record mask population, exact resident/painted chunks, stale results,
  frontier mode, exact draw counts, and fixed/transient bytes.

Gate: pause for Human Review 1. Do not begin game-scene integration.

### Slice 4: browser proof

- [ ] Reuse the shared canonical compiler/session behind an isolated browser
  Worker and domain-blind transport.
- [ ] Keep exact compilation off `requestAnimationFrame`.
- [ ] Add desktop and Pixel 7 headed-Wayland `Composed`/`Coverage` receipts and
  inspected captures.
- [ ] Preserve the UI-less browser host and payload/dependency accounting.

Gate: native and browser agree on source, painted chunks, mask generation,
and composition behavior.

### Slice 5: Terrain Lab adoption

- [ ] Add the proven runtime horizon/composed presentation as a shared Lab
  consumer only if the comparison improves the research workflow.
- [ ] Preserve existing canonical, CPU LOD, fast CPU LOD, and GPU LOD panes.
- [ ] Do not move compositor, source identity, or mask policy into React.
- [ ] Close the remaining world-view-navigation exact-source extraction step.

Gate: Terrain Lab can inspect the runtime composition without becoming its
engine owner.

### Slice 6: closeout and game handoff

- [ ] Update Tactical 261 evidence and mark PH-1/PH-2 complete.
- [ ] Record the exact construction seam for `mclone-scene`.
- [ ] Preserve the exact-only game baseline and app-rim executor rule.
- [ ] Open a separate game-scene adoption tactical after human acceptance.

## Human Review 1

Review the native World Explorer at fixed and moving views:

1. toggle `Horizon`, `Exact`, `Composed`, and `Coverage` without moving the
   camera;
2. confirm composed terrain is continuous at all four exact edges;
3. look for cracks, vertical curtains, z-fighting, duplicate water, tree
   duplication, color/lighting discontinuity, or camera-like jumps;
4. move slowly across a chunk boundary while exact work is delayed;
5. zoom so the exact footprint is small enough to inspect against multiple
   clipmap levels;
6. inspect coast, steep terrain, water, and dense forest anchors; and
7. compare negative-coordinate and post-teleport behavior.

Subjective acceptance does not require final game lighting or a perfect
frontier treatment. It does require that the ownership model is visually
credible and that any remaining artifact is stable, localized, and explained
by diagnostics.

## Acceptance At The Review Checkpoint

- One native World Explorer frame contains exact and procedural terrain on one
  color/depth target.
- The procedural mask equals the published exact-painted snapshot.
- Incomplete exact chunks leave procedural coverage visible.
- Exact admission removes procedural terrain atomically for that chunk.
- Exact eviction restores already committed procedural coverage.
- Procedural and exact natural trees do not visibly co-render inside painted
  chunks in the reviewed footprint.
- Negative coordinates and teleport do not shift mask ownership.
- Horizon-only remains the default and preserves its fixed budget.
- Exact-only performs no procedural draw work.
- Composition policy is reusable without `mclone-world-explorer`,
  `mclone-terrain-lab`, `mclone-scene`, DOM, `winit`, Android, or OpenXR types.
- Captures and a concise manual test recipe are available under `/tmp`.

## Non-Goals Before Human Review 1

- Full game, server, persistence, or `mclone-scene` integration.
- Browser exact Worker promotion.
- Terrain Lab UI changes.
- Authoritative edit invalidation.
- Multiple active/standby game worlds.
- Device-loss recovery beyond preserving existing Explorer behavior.
- Synthetic stereo, OpenXR, Quest, or multiview.
- Final water, fog, lighting, or vegetation quality.
- Distant player-build summaries.
