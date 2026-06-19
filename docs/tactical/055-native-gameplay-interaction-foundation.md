# 055: Native Gameplay Interaction Foundation

Status: in progress.

## Purpose

Start the native gameplay loop for player movement, block picking, block
breaking, and block placement without baking desktop-only or renderer-only
shortcuts into the engine.

The target shape is Java-shaped and modular:

- platform adapters collect raw input and cursor-lock state
- a client player/controller layer turns input and the current hit result into
  gameplay commands
- shared core/protocol types carry block positions, hit faces, and action
  requests
- the server applies authoritative world mutations
- existing section block deltas publish results back to the client
- the renderer observes the client replica and dirties affected sections

This should replace the current spectator-camera-only interaction path with the
first real gameplay loop, while leaving room for survival movement, inventory,
items, multiplayer acknowledgements, entities, block entities, and block-specific
placement rules.

## Progress

- 2026-06-19: Added shared block-position/direction/hit-result primitives,
  client-side full-cube block raycast, debug break/place protocol commands,
  integrated-server authoritative mutation through section block deltas, native
  mouse-click wiring, and scripted headless interaction capture.
- 2026-06-19: Added a platform-neutral local player input/controller layer in
  `mclone-client`. The native app now maps `winit` keys into Java-shaped input
  impulses and no-clip movement displacement instead of storing raw key state as
  gameplay state.

## Current Native State

Native code already has several useful pieces:

- `native/apps/mclone-native-client/src/app.rs`
  - owns `winit` window/input events, cursor lock, UI gating, redraw cadence,
    runtime polling, and render upload
  - maps `WASD`, `Space`, `X`, and `Shift` into platform-neutral player input
    keys before the client controller produces no-clip movement displacement
  - left/right mouse buttons request mouse lock first, then send debug
    break/place commands while the world is active and locked
- `native/apps/mclone-native-client/src/camera.rs`
  - owns `SpectatorCamera`, look math, speed adjustment, chunk-interest center,
    and conversion to `ChunkCamera`
  - this is camera/app scaffolding, not a player entity or Java movement model
- `native/apps/mclone-native-client/src/scene_runtime.rs`
  - owns local/remote client-server exchange, client snapshot application,
    section render dirtying, render compile queue, and helper block lookups
  - has private `block_state_at_world`/`snapshot_block_state_at_world` helpers
    used for camera-inside-occluder checks and spawn placement
  - handles `ServerUpdate::SectionBlockUpdates` by dirtying only affected render
    sections and their direct block-neighborhood dependencies
- `native/crates/mclone-client/src/lib.rs`
  - stores authoritative client chunk snapshots
  - applies section block deltas to loaded snapshots and ignores unknown chunks
  - owns the first client interaction and local player controller modules
- `native/crates/mclone-protocol/src/lib.rs`
  - has chunk-view plus player-action/use-item-on client commands
  - already has `ServerUpdate::SectionBlockUpdates`
- `native/crates/mclone-server/src/integrated.rs`
  - routes client commands into `IntegratedServer`
  - handles chunk-view plus debug break/place commands through scheduler-owned
    mutation APIs
- `native/crates/mclone-server/src/scheduler.rs`
  - already has `block_at_world` and `set_block_at_world`
  - `set_block_at_world` mutates live chunk storage, patches the published
    snapshot, bumps revisions, marks chunks dirty for persistence, and records
    section block deltas for visible chunks
  - this is the right first authoritative mutation primitive for break/place
- `native/crates/mclone-server/src/types.rs`
  - aliases `WorldBlockPos` to shared `mclone_core::BlockPos`
- `native/crates/mclone-worldgen/src/block.rs`
  - has the terrain-MVP `RawBlockId` set and `generated_block_state_id`
  - it is not yet a full vanilla block-state/item registry

Important constraint: the section-delta path from
[`031-native-section-block-delta-updates.md`](031-native-section-block-delta-updates.md)
is already the permanent runtime mutation publication path. Gameplay should
reuse it rather than mutating meshes or client snapshots directly.

## Java 1.17.1 Reference Shape

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/player/Input.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/KeyboardInput.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/GameRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/ClipContext.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/BlockGetter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/HitResult.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/BlockHitResult.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/shapes/VoxelShape.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/AABB.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerPlayerGameMode.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/item/context/UseOnContext.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/item/context/BlockPlaceContext.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/item/BlockItem.java`

Key facts from the reference:

- `KeyboardInput` is raw movement input state: left/right, forward/back,
  jumping, and shift. It is not a `winit` or keyboard event owner.
- `LocalPlayer` consumes `Input`, updates local movement state, and sends
  movement/input packets. It does not own platform input events.
- `Entity.move`, `LivingEntity.travel`, and `Player.travel` are the real
  collision/friction/gravity movement stack. A basic creative/no-clip first
  slice should not pretend to be this full stack.
- `GameRenderer.pick` computes `Minecraft.hitResult` from the camera entity.
  It block-picks via `Entity.pick` and then optionally entity-picks closer
  targets.
- `Entity.pick` uses eye position plus view vector and calls
  `Level.clip(ClipContext(..., Block.OUTLINE, Fluid.NONE, entity))` for normal
  crosshair block picking.
- `BlockGetter.clip` uses DDA block traversal, compares block and fluid shape
  hits, and returns `BlockHitResult` or a miss.
- `VoxelShape.clip` handles start-inside-shape and exact face direction. For the
  current terrain-MVP block set, a full-cube shape is enough for the first
  block-pick slice, but the API should not preclude future outline/collision
  shape parity.
- `Minecraft.handleKeybinds` turns key-use/key-attack state into
  `startAttack`, `continueAttack`, `startUseItem`, and `pickBlock`.
- `MultiPlayerGameMode` owns client-side interaction state such as destroy
  progress, destroy delay, pick range, and use-item-on behavior.
- `ServerPlayerGameMode` validates distance/reach/world bounds and applies
  authoritative block break and item-use-on behavior.
- Real placement is item-shaped:
  `useItemOn -> UseOnContext -> BlockPlaceContext -> BlockItem.place`.
  Mclone's first placement slice can use a debug creative block, but the target
  architecture should keep that later item path obvious.

## Target Native Shape

Keep desktop ownership in the app adapter:

- `app.rs` should only translate `winit` events into platform-neutral input
  facts and UI/cursor-lock decisions.
- `winit::KeyCode`, `MouseButton`, cursor positions, and device events should
  not leak into shared gameplay crates.

Add shared primitives before gameplay commands depend on server-local types:

- `mclone_core::BlockPos` or an equivalent Java-shaped world block position
  type with `x`, `y`, `z`, chunk conversion, section conversion, local
  coordinate helpers, and offset helpers.
- `mclone_core::Direction` with `Down`, `Up`, `North`, `South`, `West`, `East`,
  opposite, step vector, and compact protocol encoding.
- `mclone_core::HitResult` / `BlockHitResult` or a small interaction-specific
  equivalent carrying type, hit location, block position, face direction, and
  inside flag.

Recommended module ownership:

- `mclone-core`
  - block positions, directions, hit-result data, and lightweight ray/AABB
    primitives that do not depend on assets, renderer, server, or `winit`
- `mclone-client`
  - Java-shaped client interaction controller state, current hit result, destroy
    progress/delay, pick range, and command construction
  - client-world block lookup over loaded `ChunkSnapshot`s
- `mclone-protocol`
  - client action commands and codecs
  - server acknowledgement/update types only as needed
- `mclone-server`
  - authoritative command validation and mutation through scheduler-owned world
    storage
  - early debug creative break/place methods on `IntegratedServer` or a small
    server game-mode module
- `mclone-native-client`
  - `winit` input adapter, cursor lock, UI gating, and temporary HUD/debug
    display of the current hit result
- `mclone-web-client`
  - loopback smoke coverage for the shared command/protocol path

Avoid a new `mclone-gameplay` crate until the shared concepts are too large for
`mclone-client`/`mclone-server`/`mclone-core`. A crate split is likely later, but
the first slice can stay simpler.

## First Implementation Slice

The first slice should make block interaction real without trying to ship the
entire Java survival stack.

1. Shared primitives
   - add `BlockPos`/`Direction` to `mclone-core`
   - either move `mclone-server::WorldBlockPos` to the shared type or bridge it
     through explicit conversions
   - add protocol encode/decode coverage for direction and block position

2. Client block lookup and raycast
   - expose a client-world block lookup from loaded snapshots
   - add a first `ClipContext`/`BlockHitResult` path using Java DDA traversal
   - begin with full-cube outline shape for non-air terrain-MVP blocks and empty
     shape for air-like blocks
   - keep the function signature shape-compatible with future
     outline/collision/fluid shape selection

3. Client interaction controller
   - add a small Java-shaped controller mirroring the role of
     `MultiPlayerGameMode`
   - store pick range, current hit result, destroy state, and selected debug
     placement block
   - on left-click, send a break command for the hit block
   - on right-click, send a debug creative place command for the block adjacent
     to the hit face
   - do not mutate client chunks directly

4. Protocol and server commands
   - add client commands for basic break/place requests
   - carry enough context to validate on the server: target block position,
     clicked face for placement, and the requested block state/raw block for the
     debug placement path
   - integrated and dedicated server paths should both accept the commands
   - server applies changes with `ChunkScheduler::set_block_at_world`
   - server drains/returns the resulting `SectionBlockUpdates`

5. Native app wiring
   - desktop adapter maps mouse buttons to interaction controller actions only
     when the UI is inactive and the mouse is locked/requested
   - camera look can remain the current spectator/creative movement for this
     slice, but the state should be named as player/controller state where it
     becomes gameplay-facing
   - keep chunk interest centered from the camera/player position as today

6. Validation hook
   - add a deterministic headless or scripted runtime path that:
     - loads a small world
     - raycasts a known visible block
     - breaks it
     - places a known block against a neighboring face
     - waits for section deltas/render rebuilds
     - captures `/tmp/mclone-gameplay-interaction.png`

## Protocol Shape

Do not overfit the first protocol to final Java packets, but keep names and
fields close enough that later parity is straightforward.

Recommended first shape:

```rust
pub enum ClientCommand {
    SetChunkView(ChunkView),
    PlayerAction(PlayerActionCommand),
    UseItemOn(UseItemOnCommand),
}

pub enum PlayerActionKind {
    StartDestroyBlock,
    StopDestroyBlock,
    AbortDestroyBlock,
    DebugInstantBreak,
}

pub struct PlayerActionCommand {
    pub pos: BlockPos,
    pub direction: Direction,
    pub kind: PlayerActionKind,
}

pub struct UseItemOnCommand {
    pub hit: BlockHitResult,
    pub action: UseItemOnKind,
}

pub enum UseItemOnKind {
    DebugPlaceBlock { block_state: BlockStateId },
}
```

For the first playable slice, `DebugInstantBreak` and `DebugPlaceBlock` are
acceptable. Preserve the `Start`/`Stop`/`Abort` enum space because real destroy
progress follows Java's `ServerboundPlayerActionPacket` flow.

Open question for implementation: whether the debug placement command should
carry `BlockStateId` or `RawBlockId`. `BlockStateId` is protocol/core-friendly,
but the current server mutation primitive takes `RawBlockId`. If conversion is
kept as the terrain-MVP identity mapping, document it and keep it local to the
server command handler.

## Raycast Scope

First slice:

- block-only raycast from eye position along view vector
- pick range `5.0` for creative-style mode
- full-cube outline for current full-block terrain states
- empty outline for air/cave air and current non-solid plants/snow/glow-lichen
  if shape facts are immediately available; otherwise start with air vs
  non-air full cubes and list the shape gap in the status log
- return miss with Java-style end-position block and nearest direction
- no entity picking
- no fluid picking
- no block-specific outline boxes beyond full cube

Later parity:

- exact `ClipContext.Block.OUTLINE` / `COLLIDER` / `VISUAL`
- fluid modes `NONE`, `SOURCE_ONLY`, `ANY`
- non-cubic `VoxelShape` tables for slabs, plants, snow layers, fluids, and
  model-dependent outlines
- entity picking from `GameRenderer.pick`
- crosshair block outline rendering

## Movement Scope

First slice:

- keep current creative/no-clip style movement usable
- move platform key state into a Java-shaped `Input`/`KeyboardInput` equivalent
  so future movement code consumes input facts rather than `winit` keys
- keep movement and camera position as the player/camera state feeding pick
  origin and chunk interest
- do not attempt full collision-backed survival movement yet

Later parity:

- `Entity` AABB, pose, eye height, on-ground, velocity, and collision flags
- `Entity.move` collision resolution against block collision shapes
- `LivingEntity.travel` gravity, friction, fluids, ladders, jump, and sprint
- `Player.travel` creative flying, swimming adjustments, stats, and abilities
- server-authoritative player movement and reconciliation

## Block Break/Place Scope

First slice:

- left click: debug instant break of the picked block to air
- right click: debug creative place of a selected terrain-MVP block adjacent to
  the picked face
- server rejects unloaded chunks, out-of-height positions, air break no-ops, and
  same-state placements by returning no mutation
- server uses existing section block delta publication
- renderer rebuilds through existing dirty section flow

Out of scope:

- inventory
- item stacks
- tools and destroy speed
- drops
- block entities
- block-specific `use` behavior
- survival reach/permission model beyond a simple distance guard
- neighbor/block update cascade beyond the current fluid and light hooks
- client prediction rollback and block-break ack packets

## Validation

Required tests for the first slice:

- `mclone-core`: `BlockPos`, `Direction`, and DDA ray traversal edge cases,
  including negative coordinates and starting inside a block
- `mclone-protocol`: roundtrip for new action commands
- `mclone-client`: raycast against loaded snapshots, miss behavior, and ignored
  unloaded chunks
- `mclone-server`: break/place commands mutate live chunks through
  `set_block_at_world` and emit section deltas
- `mclone-native-client`: scripted interaction dirtying render sections
- `mclone-web-client`: loopback codec/command smoke still passes

Required commands:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-core -p mclone-protocol -p mclone-client -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
pnpm --silent native:web:build
pnpm --silent native:web:smoke
```

Pixel-producing validation is required before closing a rendered slice:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-gameplay-interaction.png --width 960 --height 540 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2
```

The final command will need the scripted interaction flag added by the
implementation slice. Inspect the screenshot before marking the slice done; it
must show the broken/placed block result, not only a nonblank world.

## Follow-Up Slices

Likely order after the first playable block-interaction pass:

1. Crosshair block outline and hit-result debug/HUD plumbing.
2. Held-dig destroy progress with `StartDestroyBlock`, `StopDestroyBlock`, and
   `AbortDestroyBlock`.
3. Basic inventory/selected hotbar block source, replacing debug placement.
4. Non-cubic outline/collision shape facts shared by raycast and movement.
5. Collision-backed creative/survival movement using player AABB.
6. Dedicated streaming transport upgrade if request/response commands become a
   visible interaction bottleneck.
7. Entity picking once entities exist.

## Status Log

- Created: parent tactical grounded in current native client/server/protocol
  state and Java 1.17.1 interaction/movement references.
