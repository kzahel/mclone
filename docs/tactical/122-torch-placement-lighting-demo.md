# 122: Torch Placement Lighting Demo

Status: active; Slice A landed.

## Purpose

Let players place normal torches from the current debug creative palette/hotbar
as a focused lighting demo, without pretending the engine has a full creative
inventory or item registry yet.

Workstream: native Rust, shared implementation, desktop validation first. The
feature must stay in shared content, server placement, client/runtime, mesh, and
render contracts; app crates should only collect platform input.

## Context

This builds on the existing debug block palette and hotbar, not a durable item
system. The current hotbar still has an empty ninth slot, and the block palette
already assigns block-state ids into selected slots. That is enough for a demo
once torch states and placement semantics exist.

This also intentionally references
[`113-block-edit-render-coherence.md`](113-block-edit-render-coherence.md).
Torch placement changes both block state and block light. The current live-light
path republishes refreshed light as a full `ChunkSnapshot`, then separately emits
the block delta. That is acceptable for this first desktop demo, but the
coarser path must not be mistaken for the final live light-delta solution.
Repeated torch placement/removal on web should continue to be treated as part of
tactical 113 until `LightSectionUpdates` or equivalent live light deltas land.

Java 1.17.1 reference files:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/TorchBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/WallTorchBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/item/StandingAndWallBlockItem.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java`
- `reference/minecraft-1.17.1/src/assets/minecraft/blockstates/torch.json`
- `reference/minecraft-1.17.1/src/assets/minecraft/blockstates/wall_torch.json`

## Target Shape

- Add terrain-MVP block states for `minecraft:torch` and
  `minecraft:wall_torch[facing=north/east/south/west]`.
- Use vanilla model/blockstate assets through the existing asset and mesh
  catalog, not custom renderer geometry.
- Give normal torches block-light emission `14`, no collision, non-occluding
  render/light facts, and Java-shaped outline boxes.
- Extend debug block placement so the torch item behaves like
  `StandingAndWallBlockItem`: floor placement uses `torch`, side placement uses
  the matching `wall_torch[facing=*]`, and invalid unsupported placements fail.
- Add a "Torch" entry to the debug palette and put torch in the empty default
  hotbar slot only after placement semantics are correct.
- Validate with desktop-first screenshots at night or in shadow, plus tests for
  registry coverage, shape/fact parity, placement choice, and runtime light
  refresh.

## Phasing

### Slice A: Torch Content And Facts

Status: landed 2026-07-01.

Register torch and wall-torch states in the terrain-MVP registry and shared
block id table. Add shared block facts for no-collision, opacity `0`, emission
`14`, block names, mesh render facts, and outline/collision shapes. Do not
expose the item in the hotbar yet.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p
  mclone-assets -p mclone-blocks -p mclone-mesh`
- no screenshot required for this slice because no player-facing placement path
  is exposed yet

### Slice B: Standing/Wall Placement Semantics

Status: pending.

Extend debug `BlockItem` placement to support torch-specific placement-state
selection. Preserve the current generic block-item path for normal cube blocks
and rotated pillars. Add server tests for top-face floor torch placement, side
face wall torch placement by facing, unsupported placement rejection, and block
delta publication.

### Slice C: Palette/Hotbar Exposure And Visual Validation

Status: pending.

Add "Torch" to the debug palette and default hotbar slot 9. Capture and inspect
a native screenshot showing placed torch geometry and visible light contribution
with lighting enabled and fullbright disabled. Run the focused server/runtime
tests plus the normal native smoke lane that is practical for the touched
surface.

### Slice D: Web/XR Follow-Up

Status: pending.

Verify web uses the same shared content and placement contracts. Track any
coarse live-light snapshot/remesh cost under tactical 113 rather than fixing it
inside this torch feature. XR controller placement should consume the same
shared interaction command once XR block interaction is wired.

## Non-Goals

- A real creative inventory, item stack model, crafting, or survival item
  consumption.
- Torch particles, flame animation, sounds, or item icons beyond the existing
  GUI icon extraction.
- Live light-delta protocol work; that belongs to tactical 113.
- Desktop-only placement or renderer code.
