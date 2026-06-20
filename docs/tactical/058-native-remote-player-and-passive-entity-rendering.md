# 058: Native Remote Player And Passive Entity Rendering

Status: proposed.

## Purpose

Make native multiplayer and entity state visible in the world.

The server now has per-player chunk tracking, remote-player publication, and a
real two-client native smoke. The missing feature is obvious in play: other
players exist in protocol/client state but are invisible. Passive entities are
also still a native feature gap, even though the project has legacy creature
research and vanilla references.

This tactical is a product-facing rendering and entity-presentation lane:

1. render remote dedicated players first
2. add a narrow native entity snapshot path
3. render one passive animal, likely cow first, then chicken

Do not start with natural spawning, mob caps, full AI, or entity persistence.
Those are important later, but the next high-value step is seeing living actors
in the world.

## Current Native State

Relevant landed pieces:

- `mclone-protocol` already has `RemotePlayerAdd`,
  `RemotePlayerUpdate`, and `RemotePlayerRemove`.
- `mclone-client::ClientRuntime` stores remote players by
  `RemotePlayerId`.
- `mclone-server::remote_players` routes remote-player lifecycle and movement
  updates to observers whose accepted chunk view tracks the subject.
- `mclone-native-client` can run two concurrent remote screenshot clients and
  assert each retained one remote player.
- `mclone-render` currently renders sky, chunk sections, and GUI. It has no
  entity/actor render pass.
- `docs/creatures.md` is useful legacy/reference context, but native Rust has
  not ported the TypeScript entity runtime or creature renderer.

The immediate visual gap is therefore on the client/render side, not in TCP
connectivity.

## Reference Shape

Read these Java 1.17.1 files before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/EntityRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/LivingEntityRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/player/PlayerRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/model/PlayerModel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/model/HumanoidModel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/model/CowModel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/model/ChickenModel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/CowRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/ChickenRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java`

Reference facts that matter for this lane:

- `EntityRenderDispatcher` owns the renderer registry and renders entities
  after preparing camera/world context.
- `LivingEntityRenderer` applies entity pose transforms, body/head rotation,
  model animation setup, optional render layers, name/hitbox/shadow handling,
  then delegates model drawing.
- `PlayerRenderer` is a `LivingEntityRenderer` using a default or slim
  `PlayerModel`, plus many layers. Native should not port every layer before a
  first visible remote player.
- `CowModel` and `ChickenModel` are cuboid model-part hierarchies with texture
  coordinates. Cow is the simpler first real passive model; chicken adds flap
  and egg-timer state later.
- `Cow` and `Chicken` define goals, attributes, sounds, interaction, and
  type-specific state. Those are server simulation concerns, not prerequisites
  for the first visual pass.

## Native Boundary Direction

Keep the shape Java-inspired but Rust-sized:

- `mclone-client`
  - keeps authoritative remote-player and future entity replicas
  - exposes render-facing presentation snapshots
  - owns interpolation buffers later, not the renderer
- `mclone-protocol`
  - keeps remote-player updates separate from future generic entity updates
  - future entity protocol should use explicit `EntityId`, `EntityKind`,
    position, rotation, on-ground, and type-specific payloads
- `mclone-server`
  - owns authoritative player and entity state
  - routes entity snapshots/updates/removes by player chunk visibility
  - later owns entity ticking/spawn lifecycle
- `mclone-render`
  - owns reusable GPU entity/model rendering primitives
  - should not know about server sessions, chunk tickets, or spawn rules
- `mclone-native-client`
  - composes passes in frame order: sky, chunks, entities, GUI
  - owns native/headless validation wiring and debug counters

Do not put entity rendering into `scheduler.rs`, `integrated.rs`, or the chunk
renderer. This should be a focused render/client/server lane.

## First Slice: Remote Player Rendering

Goal: a second native client is visibly present in the first client's world.

Scope:

1. Add a render-facing remote-player presentation view in `mclone-client` or
   `mclone-native-client`.
2. Add a small `mclone-render::entity` module with a simple actor renderer.
3. Render remote players after chunks and before GUI using the existing camera
   and depth target.
4. Use a deliberately simple first visual:
   - a colored upright box/capsule, or
   - a minimal cuboid humanoid using `steve.png` if texture loading is cheap
5. Orient the actor from remote yaw and place it at the authoritative feet
   position.
6. Count drawn entities in the full-frame render summary and debug pane.
7. Extend the two-client remote smoke to require nonzero rendered entity count
   and inspect both screenshots.

Non-goals for this slice:

- player skins/accounts
- armor, held items, capes, name tags, shadows, fire, hitboxes
- interpolation beyond using the latest authoritative update
- entity picking or combat
- generic mob/entity protocol

This slice should make multiplayer feel real without pretending the full entity
system exists.

## Second Slice: Native Entity Snapshot Foundation

Goal: add the narrow protocol/client/server path needed to display one
server-owned passive entity.

Scope:

1. Add native protocol records:
   - `EntitySnapshot`
   - `EntityUpdate`
   - `EntityRemove`
   - `EntityId`
   - `EntityKind::{Cow, Chicken}` or equivalent narrow enum
2. Store client-side entity replicas separately from chunk snapshots and remote
   players.
3. Add a server-owned entity store with position, rotation, dimensions, chunk,
   kind, age/tick, and alive/removed state.
4. Spawn a deterministic starter passive entity near the first safe spawn or a
   test-only/debug command path. Keep it explicit; do not implement natural
   spawning in this slice.
5. Route entity snapshots/updates/removes only to players tracking the entity's
   chunk.
6. Clear client entity replicas when their chunk unloads or when an explicit
   remove arrives.
7. Render entity placeholders through the same entity pass used by remote
   players.

Non-goals:

- generation-time original mobs
- live natural spawning
- mob caps
- despawn
- AI goals/pathfinding
- persistence
- sounds/items/drops

## Third Slice: First Passive Model

Goal: replace at least one placeholder with a recognizable passive animal.

Preferred order:

1. Cow first: large, readable, mostly static, simple cuboid model, extracted
   texture exists at
   `reference/minecraft-1.17.1/extracted/assets/minecraft/textures/entity/cow/cow.png`.
2. Chicken second: smaller and useful for animation follow-through; extracted
   texture exists at
   `reference/minecraft-1.17.1/extracted/assets/minecraft/textures/entity/chicken.png`.

Implementation direction:

- Add a minimal model-part representation in `mclone-render::entity` rather
  than hand-special-casing every animal draw.
- Port only the cuboid geometry, UVs, transforms, and simplest walk/idle
  animation needed for the selected animal.
- Keep authoritative entity data as the source of truth. The renderer consumes
  presentation state only.
- Add type-specific payload fields only when visible behavior needs them.
  Chicken flap data is useful later, but not required for a first cow.

## Later Entity Work

After visible remote players and one passive animal:

1. Add client-side interpolation for remote players and entities.
2. Add name tags and simple shadows.
3. Add cow/chicken idle/walk animation polish.
4. Add server-owned passive entity ticking at `ENTITY_TICKING` chunk status.
5. Add generation-time original mobs with oracle fixtures.
6. Add persistence for entity sections.
7. Add live natural spawning, player-distance eligibility, mob caps, and
   despawn.
8. Add entity picking/interactions.
9. Add sounds, particles, item drops, and animal-specific gameplay.

## Validation

For remote player rendering:

- `cargo test --manifest-path native/Cargo.toml -p mclone-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `pnpm --silent native:remote:smoke`
- inspect `/tmp/mclone-native-remote-client-smoke.png`
- inspect `/tmp/mclone-native-remote-client-smoke-observer.png`

For entity protocol/rendering:

- `cargo test --manifest-path native/Cargo.toml -p mclone-protocol`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-client`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml`
- `pnpm --silent native:movement:smoke`
- `pnpm --silent native:timedemo:smoke`
- `pnpm --silent native:web:build`
- `pnpm --silent native:web:smoke`

Every pixel-producing slice must save screenshots under `/tmp` and inspect
them before commit.

## Success Criteria

Remote player slice is done when:

- two native dedicated clients can see a rendered representation of each other
- the render summary/debug pane reports a nonzero drawn entity count
- screenshots prove the visible actor is framed and not hidden behind UI
- local integrated singleplayer still renders zero remote players cleanly
- web/WASM still builds

First passive entity slice is done when:

- a server-owned passive entity snapshot reaches local and remote clients
- unload/remove paths clear the entity from client replicas
- the entity renders in headless and native window paths
- entity state remains authoritative-server-owned
- the implementation does not introduce natural spawning or AI as hidden
  side effects
