# 104 Shared Interaction Target Outline

Status: active

## Goal

Add the Minecraft-style selected-block outline while tightening flat/XR gameplay
input convergence around a shared interaction target.

The implementation target is shared implementation, desktop validation first:
flat and XR may produce different world-space rays, but they should consume the
same client interaction controller, block picking rules, action command creation,
and renderer-facing outline payload.

## Reference Shape

Minecraft Java 1.17.1 keeps a current `hitResult` on the client, refreshes it
during `GameRenderer.renderLevel(...)`, and renders a block outline from the
target block's outline shape:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/GameRenderer.java`
  - `pick(...)` computes the camera/entity hit result.
  - `shouldRenderBlockOutline(...)` gates outline visibility.
  - `renderLevel(...)` calls `pick(...)` then passes the block hit to the level
    renderer.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - `renderHitOutline(...)` renders `BlockState.getShape(...)` with alpha `0.4`.

Mclone should keep the same behavioral shape but not copy Java's renderer
ownership. The client/runtime layer owns the current target; the renderer only
draws explicit outline geometry supplied for the active view.

## Current Mclone Shape

- `mclone-input` owns flat input actions/intents and stays world-agnostic.
- `mclone-client::ClientInteractionController` owns pick range, carried item
  sync, block picking, and break/place command creation.
- Flat desktop currently invokes `camera.pick_block(...)` from
  `mclone-native-client`.
- XR already has controller aim poses and stage-to-world conversion for the
  shared world-panel menu, but gameplay block interaction is not yet wired.
- `mclone-client::block_shapes` already has Java-shaped outline/collision boxes
  for current terrain MVP blocks; selection rendering should use that same
  outline source so targeting and visualization agree.

## Design

### Shared Interaction Target

Add a shared target payload in `mclone-client`:

```text
BlockInteractionTarget {
  hit: BlockHitResult,
  outline_boxes: Vec<Aabb>,
}
```

The vector is intentional even though the current shape table mostly returns one
box. Stairs, fences, walls, panes, cauldrons, and other multi-box shapes can then
land without changing flat/XR/render plumbing.

`mclone-input` should not depend on `mclone-core`, `mclone-client`, or renderer
types just to express input. World-space rays and block hit results belong in
client/render-session/XR scene code, not in input binding data.

### Rays

Flat produces an interaction ray from the local camera/crosshair.

XR produces an interaction ray from controller aim pose after `XrStageToWorld`
transform. Menu pointer rays remain menu-specific; gameplay controller rays
should reuse the transform math, then call the same `ClientInteractionController`
path as flat.

### Rendering

Add a small world-line outline renderer in `mclone-render`:

- input: `SelectionOutline { boxes, color }`
- draw: line-list edges in world space
- view: existing explicit `ChunkRenderView`
- target: existing explicit color/depth frame target

Flat renders one outline after terrain/actors and before/alongside GUI overlay.
XR renders the same outline for each eye. The renderer must not compute the hit
target itself.

## Slices

### Slice 1: Shared Target And Outline Rendering

- Expose client outline boxes for the current `BlockHitResult`.
- Add `BlockInteractionTarget` to `mclone-client`.
- Add a shared `SelectionOutlineRenderer` in `mclone-render`.
- Wire flat desktop to compute the current target once per frame and use it for
  both action commands and outline rendering.
- Wire XR scene rendering to draw a shared outline payload where a gameplay
  target exists.
- Validate with Rust tests and at least a desktop rendered-output smoke.

### Slice 2: XR Gameplay Ray And Actions

- Convert right-controller aim pose to an interaction ray when the menu is
  closed.
- Feed trigger/select state through shared interaction actions.
- Decide controller action policy: likely right trigger = attack, right squeeze
  or secondary button = use, with hysteresis matching menu pointer input.
- Keep target/action command creation shared.
- Validate desktop OpenXR/Android XR with headset smokes when available.

### Slice 3: Shape Parity And Polish

- Expand outline shape data beyond the current single-box table.
- Add multi-box outline tests for stairs/fences/walls as those block facts land.
- Tune depth/blend behavior against Java screenshots.
- Add headless pixel probes for visible outline color/position.

## Status

- 2026-06-28: Tactical opened. Reference Java `GameRenderer` and
  `LevelRenderer` paths inspected. Implementation starting with Slice 1.
- 2026-06-28: Slice 1 implemented. Added shared
  `BlockInteractionTarget`, client outline-box lookup from the block shape
  table, a reusable world-space `SelectionOutlineRenderer`, flat action/render
  wiring, and XR per-eye outline rendering from controller aim rays.
- 2026-06-28 validation:
  `cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render -p mclone-render-session -p mclone-app-runtime -p mclone-xr-scene`
  passed;
  `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
  passed; `cargo fmt --manifest-path native/Cargo.toml --all -- --check`
  passed. A desktop headless screenshot at
  `/tmp/mclone-selection-outline-eye.png` was visually inspected and showed the
  world-space outline, though the camera was close to leaves and should be
  replaced by a cleaner probe capture.
- 2026-06-28 validation gap: full
  `cargo test --manifest-path native/Cargo.toml` currently reaches unrelated
  `mclone-web-client` smoke fixture failures: expected update counts/checksum no
  longer match the produced runtime reports (`update_count` 3/6 instead of 2/4).
  No web client files changed in this slice.

Next implementation step: Slice 2 should wire XR controller button state into
shared attack/use actions with press hysteresis, then add a cleaner outline
pixel/screenshot probe for both flat and XR views.
