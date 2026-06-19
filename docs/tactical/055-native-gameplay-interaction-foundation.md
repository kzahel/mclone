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
  gameplay state. `PlayerInput` mirrors Java `Input`; extra no-clip descend and
  sprint controls stay outside that struct.
- 2026-06-19: Added client-owned local player pose with Java-style position,
  `yRot`, `xRot`, eye height, eye position, and view-vector semantics. The
  native window app now syncs its render camera from that pose for movement,
  look, and block picking.
- 2026-06-19: Added shared `Aabb` primitives and a client-side local player
  collision clipping lane. `LocalPlayerPose` now exposes the Java standing
  player bounding box, and `LocalPlayerController::move_colliding` resolves
  movement against loaded full-cube block snapshots with Java-style Y then X/Z
  clipping order while tracking horizontal collision, vertical collision, and
  `on_ground`.
- 2026-06-19: Wired native window movement through collision-backed walking by
  default. `LocalPlayerController` now carries Java-style `deltaMovement`,
  applies first-pass walking input, sprint, jump, gravity, and drag constants,
  and keeps no-clip behind an explicit native debug toggle.
- 2026-06-19: Added a client-owned block shape fact module for the current
  terrain-MVP `BlockStateId` lane. Raycast now clips against Java outline
  shapes for snow and simple plants instead of treating every non-air block as
  a full cube, and player collision now uses Java empty collision facts for
  fluids, one-layer snow, plants, large ferns, and glow lichen.
- 2026-06-19: Added a small server `game_mode` lane for debug creative
  interactions. Break/place commands validate Java-shaped reach and
  max-build-height bounds on the server and use `BlockPlaceContext`-style
  clicked-vs-relative replacement before mutating chunks.
- 2026-06-19: Added a first Java-shaped movement-sync command and server-owned
  local player state. Native window/headless paths sync player feet position,
  yaw, pitch, and `onGround` before interactions; server reach checks no longer
  trust position carried by block action commands.
- 2026-06-19: Added a server-side `UseOnContext`/`BlockPlaceContext` placement
  lane for debug block items. Clicked-vs-relative target choice and the current
  terrain-MVP replaceability facts now live outside `game_mode`.
- 2026-06-19: Added the first Java-shaped selected hotbar lane. Native number
  keys update a client selected slot, client interaction emits
  `SetCarriedItem` on the next carried-item sync when the slot changed,
  `UseItemOn` now carries only hand and hit result, and the server resolves
  placement from its own debug hotbar.

## Current Native State

Native code already has several useful pieces:

- `native/crates/mclone-core/src/pos.rs`
  - owns Java-shaped block position, direction, hit-result, vector, and AABB
    primitives that are independent of renderer, server, assets, and platform
    code
- `native/apps/mclone-native-client/src/app.rs`
  - owns `winit` window/input events, cursor lock, UI gating, redraw cadence,
    runtime polling, and render upload
  - maps `WASD`, `Space`, `Shift`, `X`, and `Ctrl` into platform-neutral player
    input keys; walking is the default mode, `Ctrl` feeds sprint, `Shift` feeds
    Java-shaped shift/sneak state, and `X` is used only while no-clip debug mode
    is enabled
  - owns the native-only movement-mode toggle; no-clip remains a debug mode and
    the client gameplay controller owns the actual movement calculation
  - syncs the local player pose to the server after movement and immediately
    before mouse interactions
  - maps number keys into selected hotbar slots on the client interaction
    controller
  - left/right mouse buttons request mouse lock first, sync carried-item
    selection when needed, then send debug break or held-item use commands while
    the world is active and locked
- `native/apps/mclone-native-client/src/camera.rs`
  - owns `SpectatorCamera`, look math, speed adjustment, chunk-interest center,
    and conversion to `ChunkCamera`
  - this is now a native render-camera adapter over client-owned player pose,
    not the gameplay movement owner
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
  - owns the first client interaction, local player input, local player pose,
    and current terrain-MVP block shape modules
  - exposes a loaded-snapshot block lookup used by raycast and collision;
    unloaded chunks are currently treated as empty for local collision
- `native/crates/mclone-client/src/inventory.rs`
  - owns the first client selected-hotbar state and Java-shaped carried-item
    sync guard
  - emits `SetCarriedItem` only when the selected hotbar slot differs from the
    last slot sent to the server
- `native/crates/mclone-client/src/block_shapes.rs`
  - maps current generated terrain `BlockStateId`s to Java-shaped outline and
    collision boxes without depending on renderer, assets, server, or `winit`
  - keeps the current limited terrain id table small until gameplay can consume
    the full block-state registry
  - implements one-layer snow outline/collision, simple plant outline boxes,
    flower X/Z selection offset, empty block-fluid outline for `Fluid.NONE`
    picking, and Java `noCollission` collision emptiness
- `native/crates/mclone-client/src/block_clip.rs`
  - owns the reusable ray-vs-AABB clipping helper used by outline shapes,
    including inside-shape hits and face selection
- `native/crates/mclone-client/src/player.rs`
  - mirrors Java `Input` impulses, local player `position`/`yRot`/`xRot`, eye
    position, standing dimensions, `deltaMovement`, and collision flags
  - provides no-clip movement for debug mode plus the first collision-backed
    walking tick with Java-derived speed, jump, gravity, friction, and drag
    constants
  - builds a Java-shaped move-player command carrying feet position, rotation,
    and `onGround`
  - gathers loaded block collision boxes through `block_shapes.rs`; exact
    multi-box and fully registry-backed shapes are still later parity work
- `native/crates/mclone-protocol/src/lib.rs`
  - has chunk-view, move-player, set-carried-item, player-action, and
    use-item-on client commands
  - keeps `UseItemOn` Java-shaped as hand plus block hit result instead of
    carrying a client-chosen block id
  - already has `ServerUpdate::SectionBlockUpdates`
- `native/crates/mclone-server/src/player.rs`
  - owns the first server-side local player state: feet position, yaw, pitch,
    and `onGround`
  - applies the narrow `ServerboundMovePlayerPacket.PosRot`-style command shape
    with Java horizontal/vertical clamps and wrapped rotations
- `native/crates/mclone-server/src/inventory.rs`
  - owns the first server-side selected hotbar state and debug hotbar block
    source
  - applies `SetCarriedItem` only for Java-valid hotbar slots `0..9`
  - exposes the currently held debug block state to the placement lane until a
    real item stack and block/item registry exist
- `native/crates/mclone-server/src/game_mode.rs`
  - owns the first server-side interaction validation lane for debug creative
    gameplay
  - mirrors Java's break/use reach and overworld max-build-height checks for
    the terrain-MVP path
- `native/crates/mclone-server/src/placement.rs`
  - owns the first item-shaped placement lane for debug creative block items
  - mirrors Java `UseOnContext`, `BlockPlaceContext`, and the early
    `BlockItem.place` decision flow for clicked-vs-relative placement
  - carries the current limited replaceable-block facts for air, fluids,
    one-layer snow, simple plants, large ferns, and glow lichen
  - rejects air/cave-air debug block items before placement after the server
    hotbar has resolved the held block
- `native/crates/mclone-server/src/integrated.rs`
  - routes client commands into `IntegratedServer`
  - handles chunk-view plus debug break/place commands through the server
    game-mode validation and placement lanes before using scheduler-owned
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

Movement-sync constraint: the current native move command updates a single
server-owned local player state but does not yet run authoritative server
collision, speed checks, teleport acknowledgements, or reconciliation. Those
remain later movement-authority work.

## Java 1.17.1 Reference Shape

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/player/Input.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/KeyboardInput.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Inventory.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/GameRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/ClipContext.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/BlockGetter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/HitResult.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/BlockHitResult.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/shapes/VoxelShape.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/phys/AABB.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockBehaviour.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/SnowLayerBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/TallGrassBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/FlowerBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/DeadBushBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/DoublePlantBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/MultifaceBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerPlayerGameMode.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundMovePlayerPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundSetCarriedItemPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundUseItemOnPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java`
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
- `Entity.move` resolves requested movement by clipping against collision shapes
  on Y first, then resolving X/Z in the order selected by the relative
  horizontal movement magnitudes.
- `GameRenderer.pick` computes `Minecraft.hitResult` from the camera entity.
  It block-picks via `Entity.pick` and then optionally entity-picks closer
  targets.
- `Entity.pick` uses eye position plus view vector and calls
  `Level.clip(ClipContext(..., Block.OUTLINE, Fluid.NONE, entity))` for normal
  crosshair block picking.
- `BlockGetter.clip` uses DDA block traversal, compares block and fluid shape
  hits, and returns `BlockHitResult` or a miss.
- `VoxelShape.clip` handles start-inside-shape and exact face direction. For the
  current terrain-MVP block set, the native shape lane covers full cubes,
  one-layer snow, simple plants, flowers, and empty fluid/block-air outlines;
  the API still needs future multi-box and registry-backed shape parity.
- `BlockBehaviour.getShape` defaults to `Shapes.block()`.
  `getCollisionShape` returns that outline shape only when the block has
  collision; `Properties.noCollission()` makes collision empty and disables
  occlusion.
- `Entity.pick` uses `ClipContext.Block.OUTLINE` with `Fluid.NONE`, so liquid
  blocks contribute no block outline hit in the current normal crosshair path.
- `SnowLayerBlock` defaults to `layers=1`: outline is a 2/16 block-high slab,
  but collision is `SHAPE_BY_LAYER[layers - 1]`, which is empty for one layer.
- `TallGrassBlock` and `DeadBushBlock` use a 2..14 by 0..13 by 2..14 outline
  box; `FlowerBlock` uses a 5..11 by 0..10 by 5..11 outline box plus the Java
  X/Z block offset.
- `DoublePlantBlock` does not override `getShape`, so current large fern states
  use the default full-cube outline while retaining empty collision via
  `noCollission`.
- `Minecraft.handleKeybinds` turns key-use/key-attack state into
  `startAttack`, `continueAttack`, `startUseItem`, and `pickBlock`.
- `Minecraft.handleKeybinds` maps number keys to `player.getInventory().selected`
  hotbar slots `0..8` when no creative hotbar save/load modifier is active.
- `MultiPlayerGameMode` owns client-side interaction state such as destroy
  progress, destroy delay, pick range, and use-item-on behavior.
- `MultiPlayerGameMode.ensureHasSentCarriedItem` sends
  `ServerboundSetCarriedItemPacket` only when `Inventory.selected` differs from
  the last carried index sent to the server.
- `Inventory.getSelected` returns the selected hotbar item only when
  `selected` is a valid `0..9` hotbar slot; otherwise it returns empty.
- `ServerboundMovePlayerPacket` has position, rotation, and status-only variants
  with `hasPos`, `hasRot`, and `onGround` flags. The current native command uses
  the `PosRot` subset only.
- `ServerboundSetCarriedItemPacket` carries only a slot number. The server
  accepts slots `0..9` and ignores invalid values after logging.
- `ServerboundUseItemOnPacket` carries `InteractionHand` plus
  `BlockHitResult`; it does not carry the block/item being placed.
- `ServerGamePacketListenerImpl.handleMovePlayer` rejects invalid movement
  values, clamps horizontal coordinates to +/-30,000,000 and vertical
  coordinates to +/-20,000,000, wraps rotations, and then applies much more
  authority logic than this slice ports.
- `ServerPlayerGameMode` validates distance/reach/world bounds and applies
  authoritative block break and item-use-on behavior.
- `ServerPlayerGameMode.handleBlockBreakAction` rejects block breaks outside
  squared reach `36.0` and positions at or above max build height.
- `ServerGamePacketListenerImpl.handleUseItemOn` rejects clicked positions at or
  above max build height and block-center distance squared `>= 64.0` before
  routing to game mode with the server player's item in the requested hand.
- Real placement is item-shaped:
  `useItemOn -> UseOnContext -> BlockPlaceContext -> BlockItem.place`.
  `BlockPlaceContext` chooses the clicked block when the clicked block can be
  replaced, otherwise the face-relative block. `BlockItem.place` then checks
  the target can be placed and is unobstructed before mutating the level.
  Mclone's first placement slice can use a debug creative block, but the target
  architecture should keep that later item path obvious.
- `UseOnContext` carries the hit result plus player, hand, level, and item
  stack accessors. Native currently keeps the hit result and server-resolved
  debug block item; player, level, and full item stacks are still future work.
- `BlockBehaviour.canBeReplaced` defaults to material replaceability and avoids
  replacing a block with the same item. Native mirrors that shape with a narrow
  terrain-MVP replaceability table until the real block/item registry exists.
- `SnowLayerBlock.canBeReplaced` is special: a snow item can target the clicked
  snow block only from the up face; from side faces it places in the relative
  block unless the relative block is replaceable.

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
    progress/delay, pick range, selected hotbar slot, carried-item sync guard,
    and command construction
  - client-world block lookup over loaded `ChunkSnapshot`s
- `mclone-protocol`
  - client movement, carried-item, player-action, and use-item-on commands plus
    codecs
  - server acknowledgement/update types only as needed
- `mclone-server`
  - authoritative command validation and mutation through scheduler-owned world
    storage
  - early debug creative break/place methods split between a small server
    game-mode validation module, selected-hotbar module, and item-shaped
    placement module
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
   - use `block_shapes.rs` for terrain-MVP outline and collision facts instead
     of hard-coding non-air blocks as full cubes in raycast or movement
   - keep the function signature shape-compatible with future
     outline/collision/fluid shape selection

3. Client interaction controller
   - add a small Java-shaped controller mirroring the role of
     `MultiPlayerGameMode`
   - store pick range, current hit result, destroy state, selected hotbar slot,
     and carried-item sync state
   - on left-click, send a break command for the hit block
   - on right-click, sync carried-item selection if needed, then send a
     hand-plus-hit use-item-on command
   - do not mutate client chunks directly

4. Protocol and server commands
   - add client commands for basic break/place requests
   - carry enough context to validate on the server: target block position,
     clicked face for placement, selected hotbar slot updates, and use-item-on
     hand/hit data
   - add a first move-player command so server interaction validation can use
     server-owned player state instead of trusting block action payloads
   - server resolves held placement blocks from its own debug hotbar rather than
     trusting a block id in the use-item-on command
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
    MovePlayer(MovePlayerCommand),
    SetCarriedItem(SetCarriedItemCommand),
    PlayerAction(PlayerActionCommand),
    UseItemOn(UseItemOnCommand),
}

pub struct MovePlayerCommand {
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub on_ground: bool,
}

pub struct SetCarriedItemCommand {
    pub slot: u8,
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
    pub hand: InteractionHand,
    pub hit: BlockHitResult,
}

pub enum InteractionHand {
    MainHand,
    OffHand,
}
```

For the first playable slice, `DebugInstantBreak` is acceptable. Preserve the
`Start`/`Stop`/`Abort` enum space because real destroy progress follows Java's
`ServerboundPlayerActionPacket` flow.

Placement should stay server-held-item-shaped: `UseItemOn` carries hand and hit
only, while the server maps its selected debug hotbar slot to the current
terrain-MVP block state and then to `RawBlockId` locally.

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

- keep creative/no-clip style movement usable through an explicit debug toggle
- move platform key state into a Java-shaped `Input`/`KeyboardInput` equivalent
  so future movement code consumes input facts rather than `winit` keys
- keep movement and camera position as the player/camera state feeding pick
  origin and chunk interest
- add shared AABB and loaded-snapshot full-cube collision clipping
- wire native desktop movement through a first walking loop with jump, gravity,
  sprint, sneak slowdown, and collision flag handling

Later parity:

- server/client entity ownership for AABB, pose, eye height, velocity, and
  collision flags
- `Entity.move` collision resolution against block-specific collision shapes
- full `LivingEntity.travel` parity for block friction facts, fluids, ladders,
  step-up, edge backoff, jump effects, and sprint state transitions
- `Player.travel` creative flying, swimming adjustments, stats, and abilities
- server-authoritative player movement and reconciliation

## Block Break/Place Scope

First slice:

- left click: debug instant break of the picked block to air
- right click: debug creative use of the selected server hotbar block using
  Java-shaped carried-item sync, `UseOnContext`/`BlockPlaceContext`
  clicked-block replacement first, otherwise the picked face-relative target
- server rejects unloaded chunks, out-of-height positions, air break no-ops, and
  empty selected hotbar, air/cave-air, or same-state placements by returning no
  mutation
- server uses Java-shaped reach checks against server-owned local player state
- server uses existing section block delta publication
- renderer rebuilds through existing dirty section flow

Out of scope:

- full inventory
- item stacks beyond the temporary debug hotbar block source
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
  unloaded chunks; local player input/pose semantics; full-cube collision wall,
  landing, sliding, and unloaded-space behavior; walking yaw input, jump,
  sneak slowdown, gravity/drag, and velocity zeroing on collision
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
2. Non-cubic outline/collision shape facts shared by raycast and movement.
3. Held-dig destroy progress with `StartDestroyBlock`, `StopDestroyBlock`, and
   `AbortDestroyBlock`.
4. Real item stacks and visible hotbar UI over the current selected-slot lane.
5. Dedicated streaming transport upgrade if request/response commands become a
   visible interaction bottleneck.
6. Entity picking once entities exist.

## Status Log

- Created: parent tactical grounded in current native client/server/protocol
  state and Java 1.17.1 interaction/movement references.
- 2026-06-19: Added debug break/place, Java-shaped local input/pose, and a
  tested collision clipping primitive over loaded full-cube block snapshots.
- 2026-06-19: Native desktop now starts in collision-backed walking mode and
  places the player feet on the loaded surface. `N` toggles no-clip debug mode;
  mouse wheel adjusts no-clip speed. Movement still treats every non-air block
  as a full cube, so foliage/snow/slabs/liquids need shared shape facts before
  this can be called vanilla movement parity.
- 2026-06-19: Server debug interaction commands now validate player reach,
  max-build-height, and replacement target selection through a focused
  `game_mode` module before mutating world storage.
- 2026-06-19: Added `MovePlayer` protocol and `mclone-server::player` state so
  debug interaction validation reads the server-owned local player position.
  This is still a sync-and-validate slice, not authoritative server movement.
- 2026-06-19: Split debug block placement into `mclone-server::placement` with
  native `UseOnContext`, `BlockPlaceContext`, and `DebugBlockItem` types. This
  keeps placement item-shaped while inventory, block survival, unobstructed
  entity checks, and full block-specific placement state remain future work.
- 2026-06-19: Added Java-shaped selected-hotbar sync. `UseItemOn` no longer
  carries a block id; the client sends `SetCarriedItem` for selected slot
  changes and the server resolves placement from its own debug hotbar before
  entering the `UseOnContext`/`BlockPlaceContext` path.
