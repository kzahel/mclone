# 112 - Asset Lab Runtime Actor Geometry

Status: active; Slice 1 static authored player mesh landed.

## Purpose

Promote one disposable asset-lab figure into the native runtime just far enough
to prove that AI-authored figure JSON can produce visible shared-engine actor
geometry.

This is not the final character asset pipeline. The first runtime bridge should
stay small, reviewable, and easy to delete if the figure DSL changes. It should
also avoid adding more policy to `mclone-native-client`: actor geometry belongs
in shared render/session boundaries, while platform apps keep owning only window,
surface, input, and lifecycle glue.

## Workstream

Shared implementation, desktop validation first.

Initial code target:

- `native/crates/mclone-render` for static actor mesh construction.
- `native/crates/mclone-render-session` only when actor selection or runtime
  presentation policy must change.
- `tools/asset-lab` remains the authoring and review lab, not a runtime
  dependency.

## Direction

Use the existing `FigureAsset` JSON contract from
[`111-ai-figure-asset-lab.md`](111-ai-figure-asset-lab.md) as input.

The early bridge may embed a generated player figure JSON inside the renderer to
avoid designing the full runtime asset registry too soon. That is intentionally
tactical. The durable path is:

1. asset-lab authoring file
2. exported figure JSON
3. native asset-pack or content-registry entry
4. renderer/session-resolved compiled actor model
5. optional animation clip sampling from exported clip metadata

## First Runtime Contract

Slice 1 should support only what the current player asset needs:

- schema version `1`
- named materials with hex colors
- named ASCII textures
- box primitives
- part parent relationships and local offsets
- per-face box texture overrides for face pixels
- model normalization to actor feet and actor height
- local asset-lab forward mapped to native actor forward

For the first slice, ASCII textures can be converted into tiny colored face
overlay quads instead of stitched into the actor texture atlas. This preserves
visible face details while deferring real atlas/import ownership.

## Non-Goals

- No general entity system work.
- No runtime animation sampling yet.
- No skinning, IK, glTF, or arbitrary mesh import.
- No app-local character definitions in `mclone-native-client`.
- No dependence on TypeScript or Three.js from the native runtime.

## Implementation Slices

### Slice 1 - Static Authored Player Mesh

- [x] Export or check in the current asset-lab `player` figure JSON as a small
      runtime fixture.
- [x] Add a native parser/compiler for the narrow box-only subset.
- [x] Render local and remote players with the compiled authored model.
- [x] Preserve the existing hardcoded humanoid as a fallback/debug shape.
- [x] Interpret ASCII face textures as colored front-face overlay geometry.
- [x] Add mesh tests for bounds, authored colors, and face detail placement.
- [x] Capture a native screenshot and inspect the result.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-asset-lab-player.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-camera-view third-person
git diff --check
```

Landed validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-asset-lab-player.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-camera-view third-person
git diff --check
```

Screenshot inspected:

```text
/tmp/mclone-asset-lab-player.png
```

The screenshot path currently shows the authored player from behind because the
runtime camera exposes first-person and third-person-back only. Front-face ASCII
eye/mouth placement is validated by mesh tests in this slice. A dedicated actor
turntable or front/side screenshot lane should be added before judging authored
face quality visually inside the engine.

### Slice 2 - Real Asset Ownership

- [ ] Move figure JSON loading behind `mclone-assets` or a small shared
      content-registry contract.
- [ ] Define how actor presentations select a figure asset id.
- [ ] Keep platform apps out of the model-selection policy.
- [ ] Add missing asset diagnostics for unknown figure ids.

### Slice 3 - Animation Metadata Import

- [ ] Load exported walk clips and locomotion metadata.
- [ ] Sample joint rotations on the native side.
- [ ] Preserve gait cycle distance/contact metadata for future movement-speed
      matching.
- [ ] Add visual smoke captures for side-view walk review.

### Slice 4 - Broader Primitive Set

- [ ] Add capsule, sphere, and cylinder support only after the box-only path is
      useful in-engine.
- [ ] Decide whether non-box primitives are native mesh primitives or generated
      authoring-time meshes.
- [ ] Keep generated geometry bounded and deterministic for tests.

## Open Questions

- Should the first durable figure registry live in `mclone-assets`, a new small
  shared figure crate, or `mclone-render-session`?
- Should ASCII textures stay as source data in runtime assets, or should asset
  packing derive PNG/atlas regions while preserving ASCII only in authoring?
- How much animation state belongs in render-session versus gameplay/client
  actor presentation data?
