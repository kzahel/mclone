# Falling Tree Physics

Topic: falling-tree-physics

Status: initial reference investigation. `reference/dynamic-falling-tree/` and
`reference/sable/` are local, gitignored source references only. Do not copy
Sable code into mclone without an explicit license review.

## Scope

This topic tracks the current understanding of the Dynamic Falling Tree mod and
Sable physics library as references for future mclone tree-felling, moving voxel
assemblies, and impact effects.

The immediate question is how "tree physics" works in the current mod ecosystem:
whether the tree mod owns physics itself, what Sable provides, and which ideas
are useful for mclone's Rust engine.

## Reference Clones

- `reference/dynamic-falling-tree/`
  - Upstream: `https://github.com/Cukkoo12/dynamic-falling-tree.git`
  - Branch: `main`
  - Commit: `e11e90944a31e737ee7129f8419221fcdeab67dd`
  - License: MIT
  - Target: Fabric, Minecraft 1.21.1
  - Sable dependency: `sable >= 1.2.2`

- `reference/sable/`
  - Upstream: `https://github.com/ryanhcode/sable.git`
  - Branch: `main`
  - Commit: `76e9ae7c0a54b3890c1a0e0eb8e06cf383e8d2ae`
  - License: PolyForm Shield License 1.0.0 unless otherwise stated
  - Target: Fabric and NeoForge, Minecraft 1.21.1
  - Native physics: Rust JNI bridge to a forked Rapier 3D backend

Sable is source-available and useful as an architectural reference, but its
license is not a normal permissive or copyleft open-source license. Treat it as
"read for concepts, do not derive code" unless the project explicitly accepts
that license boundary.

## Dynamic Falling Tree Shape

Dynamic Falling Tree is mostly gameplay glue around Sable:

- `CommonEvents.register()` hooks Fabric's server-side block break event.
- It ignores client events, non-log blocks, sneaking players, and non-axe tools.
- If the block is already tracked as part of a falling tree sublevel, it only
  decrements the tree log count.
- Otherwise it calls `TreeUtil.trySplit(level, pos)` to convert the tree into
  one or more Sable `ServerSubLevel`s.
- After assembly, it gets each sublevel's `RigidBodyHandle` from
  `SubLevelPhysicsSystem` and adds a small linear velocity plus angular velocity
  based on the player's look direction.
- `ServerTreeManager.tick()` polls Sable velocities each server tick. When a
  tree has lived long enough and stops moving, or times out after 600 ticks, it
  drops the sublevel's blocks as items and removes the sublevel.

Important files:

- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/CommonEvents.java`
- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/TreeUtil.java`
- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/api/flood_fill/TreeFloodFill.java`
- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/api/manager/ServerTreeManager.java`
- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/collision_callback/LogCallback.java`
- `reference/dynamic-falling-tree/src/main/java/com/cukkoo/dynamicfallingtree/collision_callback/LeafCallback.java`

## Tree Detection

Tree detection is a custom flood fill, not vanilla tree generation knowledge.
It starts from a log and explores the 26 neighboring offsets around each block.
Rules decide which neighboring blocks can join the result:

- logs connect to logs, constrained to the first log block type found;
- logs connect to leaves;
- leaves connect to same-type leaves only when the vanilla `LeavesBlock.DISTANCE`
  increases;
- optional tags add attached blocks that stay on the tree or fall separately.

The default `dynamicfallingtree:tree` block tag is logs plus leaves. The
`roots` tag includes dirt-like ground blocks. By default, a valid tree needs a
log with a root block below it. In rootless mode, validation also requires
non-persistent leaves.

When the chopped block is ignored during the split flood fill, any connected
component with no remaining root becomes a falling sublevel. This avoids turning
the whole rooted stump into a moving object.

## Sable Shape

Sable's core abstraction is a sublevel: an isolated grid of Minecraft chunk data
stored in a `LevelPlot`, plus a logical pose with position and orientation. A
`SubLevelContainer` is attached to each level by mixins and owns a grid of
sublevels in a far-away "plotyard" coordinate region.

The assembly helper performs the handoff from world blocks to a moving island:

- allocate a new server sublevel and plot;
- move selected blocks, block entities, and some related entities from world
  coordinates into the plot;
- compute mass and center of mass from Sable block physics properties;
- add the server sublevel to the physics pipeline;
- teleport the physics body to the logical pose;
- update tracking points so dependent systems can follow the sublevel.

Important files:

- `reference/sable/common/src/main/java/dev/ryanhcode/sable/api/SubLevelAssemblyHelper.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/sublevel/SubLevel.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/sublevel/ServerSubLevel.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/api/sublevel/SubLevelContainer.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/api/sublevel/ServerSubLevelContainer.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/sublevel/plot/LevelPlot.java`

## Sable Physics Pipeline

`SubLevelPhysicsSystem` observes the sublevel container. When a server sublevel
is added, it builds mass data and calls `PhysicsPipeline.add`. Every tick it:

- runs block-entity sublevel actors;
- ticks pipeline bookkeeping;
- runs one or more physics substeps;
- lets sublevels update forces before each substep;
- calls the native pipeline step;
- processes removals caused by collision callbacks;
- reads updated poses back into each server sublevel.

`PhysicsPipeline` is an abstraction. The active implementation in this clone is
`RapierPhysicsPipeline`, backed by `sable_rapier`. The Java side loads compressed
platform-native libraries into `.sable/natives`, then calls JNI functions such
as `initialize`, `createSubLevel`, `step`, `getPose`, `setCenterOfMass`,
`setLocalBounds`, `applyForceAndTorque`, and `addLinearAngularVelocities`.

The Rust side owns the Rapier scene: rigid bodies, colliders, broad/narrow
phase, joints, ropes, collision hooks, voxel collider maps, and chunk-section
collision data. A Sable sublevel becomes a dynamic Rapier body with a custom
level collider. The collider uses voxel/octree data and block physics properties
to collide a moving block island against the main world and other bodies.

Important files:

- `reference/sable/common/src/main/java/dev/ryanhcode/sable/sublevel/system/SubLevelPhysicsSystem.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/api/physics/PhysicsPipeline.java`
- `reference/sable/common/src/main/java/dev/ryanhcode/sable/api/physics/handle/RigidBodyHandle.java`
- `reference/sable/sable_rapier/src/main/java/dev/ryanhcode/sable/physics/impl/rapier/RapierPhysicsPipeline.java`
- `reference/sable/sable_rapier/src/main/java/dev/ryanhcode/sable/physics/impl/rapier/Rapier3D.java`
- `reference/sable/sable_rapier/src/main/rust/rapier/src/lib.rs`
- `reference/sable/sable_rapier/src/main/rust/rapier/src/scene.rs`
- `reference/sable/sable_rapier/src/main/rust/rapier/src/voxel_collider.rs`

## Collision Effects

Sable supports block-level collision callbacks. Dynamic Falling Tree injects
callbacks for logs and leaves through `BlockWithSubLevelCollisionCallback`.

- Logs trigger on higher impact velocity, play a wood break sound, and spawn
  custom brown dust particles.
- Leaves trigger at low impact velocity, spawn vanilla block particles, and
  remove the leaf block without drops.
- Sable's `FragileBlockCallback` normally checks impact velocity, resolves the
  currently stepping physics system, reads the block state, and can request
  collision removal.

The tree mod deliberately performs "auto break on land" in the main server tick
instead of inside the physics collision callback. That is a useful separation:
impact effects are collision-time, while destructive world mutation is deferred
to ordinary server-thread logic.

## mclone Takeaways

For mclone, the useful idea is not Sable's Minecraft integration. The useful
idea is a product feature decomposition:

1. Detect a connected tree component after the chop block is removed.
2. Convert that component into a transient moving voxel assembly.
3. Preserve a local block grid plus a world pose for rendering and collision.
4. Simulate the assembly as a rigid body with mass, center of mass, bounds, and
   block-derived collision.
5. Emit impact effects and optionally settle the assembly into drops or placed
   debris.

mclone should implement this, if pursued, in shared engine crates rather
than app code. Likely ownership:

- tree detection and felling gameplay: `mclone-server` or a future shared
  gameplay/content crate;
- transient moving voxel assembly contract: shared simulation crate, not
  `mclone-native-client`;
- physics integration: a shared physics boundary, probably using Rapier directly
  rather than a Java/JNI bridge;
- render representation: `mclone-render-session`/`mclone-render` with
  single-view, stereo, and XR multiview support;
- input trigger and HUD feedback: `mclone-input` and `mclone-ui` only where
  player interaction needs wiring.

Do not add this as a desktop-only feature. A falling tree is gameplay and visual
state that should replicate consistently to flat desktop, XR, Android, and web
once the engine owns the right abstractions.

## Open Questions

- Should falling trees become item drops, physical debris, or placed logs after
  settling?
- Should leaves break during collision, remain attached, or decay after the
  assembly settles?
- Should the first implementation use true rigid-body collision or a cheaper
  authored fall animation with block drops?
- What is the minimum shared physics abstraction mclone needs before adding
  moving voxel assemblies?
- Can web/WASM support the same physics path, or does it need a compatible
  fallback behind the same engine contract?
