# Embedded Worlds

Topic: `embedded-worlds`

Design north-star for showing a *second* world inside the current one: a lobby
diorama, a tabletop seed explorer, a "palantir" window into a network-hosted
world, and the shrink-and-fall transition between nested worlds.

Status: **design, a non-rendering dual-host proof, and a Slice 1 architecture
checkpoint.** The engine does not yet compose or switch live worlds, but
`mclone-app-runtime/tests/dual_integrated_hosts.rs` retains two native
integrated-server runners, connection adapters, and client replicas at once.
Tactical 174 now also classifies all 81 flattened scene-host fields, locks the
current startup/reset/asset-epoch contracts, and records renderer-shell,
thread/cadence, offscreen, frame-budget, and synthetic-stereo baselines. A
maintainer manual desktop-flat smoke found ordinary single-world behavior
normal. A five-run audit on the current M4 Pro Mac accepts the 240-frame 120 Hz
frame-budget probe as the flat release timing anchor and marks timedemo frame
timing too variable for regression decisions on this host. This records the
larger vision, the fidelity ladder, concrete engine seams, and the warm-world
burn-down so future slices do not have to re-derive them. The bounded first
implementation milestone is
[`174-warm-world-hot-swap.md`](../tactical/174-warm-world-hot-swap.md): retain
two drawable local worlds and switch through an opaque gate, with an explicit
stop before simultaneous rendering.

Last reconciled: 2026-07-13 (Tactical 174 Slice 1 checkpoint).

## Motivation

For XR immersion, spawning into a menu is weak; spawning into a built room is
strong. From there the idea grows: a room whose tabletop holds a live preview
of another world (zoo-diorama style), a command-center that is really a
worldgen/seed console, a texture room that exposes engine internals as
manipulable objects, and eventually a portal into a *live, network-hosted*
world — "the nether-portal idea, but better." The signature effect is diving at
the diorama and continuously shrinking, falling from its sky into the real
world.

The unifying primitive under all of it is: **render a second world's content
inside the frame the first world occupies.** Everything else is a flavor of
that.

## Two techniques (pick per feature)

These are genuinely different, with different physics. Do not conflate them.

1. **RTT / portal-to-texture** — world B renders in its own pass with its *own*
   depth buffer, is flattened to a texture, and that texture is drawn on a
   surface. The worlds never share space. Correct for a **window into a
   disconnected place** (a palantir on a wall showing a remote server). Cannot
   give occlusion against world A (no hand-in-front, no walk-around); it is a
   picture. Requires new GPU work the engine lacks today: a sample-able
   offscreen target and a scene-color-sampling composite shader (see Seams).

2. **Shared-coordinate embedding** — world B's *actual geometry* is transformed
   (scale + offset, e.g. ÷16 onto a table) and submitted into the **same render
   pass, same depth buffer, same view-projection** as world A. Correct for
   anything that shares physical space: diorama, seed explorer, shrink-and-fall.
   Strictly more physical *and* less bespoke shader code than RTT — no offscreen
   target, no sample-back. It is "draw more chunk sections with a different
   model transform."

Rule of thumb: same physical space → embed; disconnected window → RTT. A
"reach-through" palantir is embedding, not RTT.

Shared-coordinate geometry is the primary architectural target, especially in
XR. RTT remains a deliberately limited presentation tool for a literal screen
or disconnected window; it must not become the representation of a spatial
world, diorama, walkable boundary, or traversable portal. Those cases require
real geometry submitted into the eye targets with correct stereo and depth.

## Fidelity ladder (a world need not be a full live runtime)

Each embedded world picks a tier. Cheaper tiers exist precisely so low-end and
XR budgets can host the effect, and so most instances cost almost nothing.

- **T0 — baked static mesh.** Snapshot world B's chunks, mesh once, draw with a
  model transform. No second runtime, no simulation, no updates. Cheapest;
  ideal for "last-visited world on the table" and for degrading higher tiers
  under budget pressure. Self-lights correctly for free (see below).
- **T1 — live-local read-only.** A local worldgen preview: run
  `NoiseBasedChunkGenerator` for a small radius on a scrubbable seed, re-mesh on
  change. No server, no network, no tick. This is the **seed explorer / worldgen
  console** — the dials are the real noise parameters.
- **T2 — live remote spectator.** A second, read-only runtime observes a remote
  server around a point of interest and renders its updates live. This is the
  **palantir into a network world**. It is a target tier, not a claim that the
  current command-coupled wire protocol already supports observer
  subscriptions; that gap is recorded under Concrete Seams.

- **T3 — live joined warm world.** A complete second local or remote runtime is
  retained while another world remains primary. It continues to pump updates
  and hold a ready client replica, but only the primary world receives physical
  player movement, interactions, and authoritative camera corrections. This is
  the foundation for quick world switching, a warm Nether/sky world, and later
  traversable boundaries. T3 has two distinct future levels: one-active-
  authority handoff is tractable; simultaneous or federated authority requires
  explicit server cooperation and state-transfer rules.

T0-T2 avoid the truly hard thing (two *authoritative* simulations). T3 permits
multiple live joined simulations but initially keeps exactly one authority for
the physical player. Inventory, health, velocity, identity, and persistence do
not cross unrelated hosts merely because both connections are warm.

## Immediate Foundation: Warm Worlds Before Composition

The first useful target does not need multi-world pixels. Keep two integrated
hosts alive, warm both client replicas, select one as primary, and switch which
already-ready runtime drives the existing single-world renderer. This attacks
vanilla-style abrupt dimension/world loading without first taking on portal
clipping or cross-world physics.

The first proof deliberately lives below `McloneSceneHost`: two real
`NativeIntegratedServerRunner`s feed two independent
`IntegratedRunnerConnection`s and `SingleViewRuntime`s concurrently. The test
uses different seeds and disjoint chunk views, waits for both hosts to become
idle/warm, verifies neither replica receives the other's chunks, changes the
logical active selection without reconstructing either runtime, and relies on
ordinary owned-value drop for independent shutdown.

This proves instanceability and isolation, not yet product switching. It does
not include assets, mesh compilers, GPU terrain stores, persistence roots,
camera reconciliation, or scene lifecycle. Those are the next ownership seams.
Tactical 174's Slice 1 receipt now identifies the exact scene ownership split
and pre-refactor evidence for those seams; no production slot extraction has
landed yet.

The near-term product shape is:

```text
McloneSceneHost
  one physical input/UI presentation
  one direct single-world render path
  WarmWorldOwner
    active DrawableWorldSlot -> existing renderer
    optional standby DrawableWorldSlot -> prepare under background budgets
    future N-world registry only after the bounded milestone needs it
```

`SceneSessionRuntime` should remain the one-world leaf. A new host-level
owner should retain the active and optional standby leaf runtimes; a later
registry can generalize the same slot. Turning `SceneSessionRuntime` itself into
a bag of worlds would mix connection mechanics with composition and selection
policy.

Warm switching must define what transfers. A first local proof may preserve
camera pose and map it into the destination spawn while leaving each world's
inventory/player state independent. Seamless dimension-like travel eventually
needs an explicit transfer record for position, orientation, velocity,
inventory, health, appearance, and authority acknowledgements.

## N worlds, not two

Nothing about the design is limited to two. The host holds a small **registry**
of embedded worlds, each with a transform and a fidelity tier, under an explicit
**count/perf budget** (tighter on low-end and XR). Graceful degradation:
downgrade live tiers toward T0, then reduce count, before dropping frames. The
budget owner should live with the frame-budget machinery, not ad hoc.

## What is free vs. what is net-new

Confirmed against the renderer (see Seams for file:line):

- **Free from shared-coordinate embedding:**
  - *Occlusion* between worlds — single shared depth buffer, correct by
    construction (your hand/wall/table occludes the tiny world and vice versa).
  - *Self-lighting* — light is baked per-vertex (`packed_light`, block + sky)
    and colored in-shader independent of where geometry sits, so world B lights
    itself correctly at any position/scale with zero work.
- **Net-new, and orthogonal to nesting:**
  - *Cross-world light and shadow* ("your shadow falls across the little
    world"). The engine has **no dynamic shadow system at all** — lighting is
    Minecraft baked lightmaps, faithful to 1.17.1. So world A's torch will not
    spill into world B and no shadow crosses the boundary. This is not a
    boundary to bridge; it is a dynamic shadow-mapping subsystem to *invent*,
    and a deliberate vanilla-parity divergence. Good news: it is independent of
    nesting — build dynamic shadows once for either world and, because geometry
    already shares coordinates and depth, they cross the boundary naturally.
    Track shadow work as its own concern, not a sub-task of this one.

## Concrete seams

- **Embed transform (cheap hook).** Terrain vertices are pre-baked to absolute
  world coords; the vertex shader multiplies by *only* view-projection
  (`native/crates/mclone-render/src/shaders/chunk_textured.wgsl:105`). There is
  **no model matrix today**. Add a `model` mat4 (or scale+offset) to the
  `Uniforms` block, change that line to `view_projection * model * position`,
  and bind a distinct `model` for world-B section draws (the draw loop already
  issues per-section `draw_indexed`). **No re-meshing** — world B meshes carry
  their own world coords, so `model = translate * scale` composes. Widen the
  fixed 128-byte uniform layout to fit the mat4 (`chunk.rs:64-68`,
  `mclone-render/src/uniform.rs`), and add the twin to the multiview shader
  (`chunk_textured_multiview.wgsl`) so XR stereo is correct.
  This is enough only for an unbounded opaque terrain proof. The shader must
  also publish the transformed position as `world_position`; otherwise fog is
  computed from world B's original coordinates. CPU culling, traversal,
  distance ordering, translucent sorting, chunk interest, and Far LOD currently
  use untransformed section/camera coordinates. They need a world-local camera
  derived with the inverse placement transform, or transformed bounds. Actors,
  particles, outlines, world UI, debug geometry, and audio need the same
  placement contract before this is a complete embedded world.
- **Shared depth / occlusion:** one `depth_view` per pass
  (`mclone-render/src/chunk.rs:376`, bound at `:2441` opaque / `:2539`
  translucent) — second-world draws added to the pass occlude for free.
  Separate render passes may load the same color/depth attachments and preserve
  opaque occlusion, but the compositor must own global phase ordering: sky,
  every world's opaque/cutout geometry, actors, then translucency. Current
  translucent sorting is per terrain store and is not globally correct for
  overlapping worlds.
- **Lighting is baked:** vertex carries `packed_light: u32`
  (`mclone-mesh/src/data.rs:54-60`), unpacked in
  `chunk_textured.wgsl:109-112`; CPU reference math in
  `mclone-render/src/light_texture.rs`. No shadow-map / depth-from-light pass
  exists anywhere in `mclone-render` or `mclone-xr-graphics`.
- **RTT primitive (for the window flavor) is new:** the only offscreen target is
  the headless-capture `OffscreenTarget`
  (`mclone-render/src/headless.rs:1042-1075`), created `COPY_SRC`-only for PNG
  readback — not `TEXTURE_BINDING`, so no shader can sample it. A palantir needs
  that flag plus a scene-color-sampling composite pass that does not exist
  today.
- **Second runtime is instanceable, not a rewrite:** ownership is
  encapsulated with no globals; `McloneSceneHost` holds `runtime:
  Option<SceneSessionRuntime>` and one session coordinator
  (`mclone-scene/src/lib.rs`), and `EngineRenderSession`/`SingleViewRuntime`
  each cleanly wrap one `ClientRuntime`. Keep those single-world leaf types and
  put a collection above them. Each future `WorldSlot` must own or reference its
  runtime, terrain draw store, traversal/readiness cache, upload coordinator,
  Far LOD state, actor source, placement, lifecycle, authority role, and budget.
  A plain `Vec<SceneSessionRuntime>` inside today's host is insufficient because
  the surrounding scene fields are just as world-specific as `runtime`.
- **Visibility is separate from placement:** a general submission needs a
  world-to-composition transform plus an unbounded, half-space, convex-volume,
  or portal-aperture visibility policy. A model matrix cannot implement the
  `x=0` split or a portal by itself. Current `Depth32Float` targets have no
  stencil component, so stencil portals would also require a target-format
  change; clip-volume techniques are another option.
- **Cut boundaries affect meshing:** fragment clipping can expose holes because
  the mesher may have omitted a face against a same-world neighbor that is
  later clipped away. Axis-aligned seams need boundary-aware remeshing or
  retained cap faces. Arbitrary moving cuts require true geometry clipping and
  cap generation or a deliberately constrained visual contract. The earlier
  "no re-meshing" observation applies to unbounded transformed dioramas, not
  clipped worlds.
- **Aggregated identities need a namespace:** independent replicas can retain
  plain `RenderSectionKey`, `EntityId`, and `RemotePlayerId`. Anything collected
  across worlds must qualify them with a client-side `WorldInstanceId`. The
  network protocol does not need that id when the connection/slot already
  supplies the namespace.
- **Remote spectator is a target, not current wire behavior:** native remote
  updates are still paired with commands rather than an independent observer
  subscription/server-push stream. A real T2 needs an observer protocol or an
  ordinary joined connection that deliberately maintains view/keepalive
  commands. `ClientRuntime` alone is not sufficient; the connection, update
  pump, render session, compiler, and uploads must remain alive too. See
  `docs/session-network-architecture.md` and
  `docs/topics/multiplayer-networking.md`.
- **Fixed lobby room (no worldgen):** the chunk pipeline is persistence-first,
  worldgen-only-on-miss, so a custom `WorldStore` returning authored chunks
  keeps worldgen from firing; alternatively stamp the room with
  `set_block_at_world` at spawn (precedent: `mclone-server/src/physics_terrain.rs`
  builds fixed worlds block-by-block).
- **Disable block destruction (lobby mode):** one authoritative choke point —
  gate `handle_player_action_for_target` / `set_block_debug` in
  `mclone-server/src/integrated.rs`. No game-mode system exists yet; this would
  be the first per-world flag, natural to sit beside `movement_mode` in
  `StartupSceneOptions`. The client option may request a lobby profile, but the
  behavior must reach an authoritative shared server policy so local and
  dedicated hosts cannot diverge or be bypassed by a client.

## Runtime Ownership Target

The likely shared shape is intentionally above the existing leaf runtime:

```rust
struct WorldSlot {
    id: WorldInstanceId,
    runtime: SceneSessionRuntime,
    terrain: TexturedSectionDrawResources,
    traversal: TraversalReadySectionCache,
    uploads: RenderSectionUploadCoordinator,
    far_lod: FarTerrainLodRenderer,
    placement: WorldPlacement,
    lifecycle: WorldLifecycle,
    authority: PlayerAuthorityRole,
    budget: WorldBudgetClass,
}

struct WorldComposition {
    worlds: WorldRegistry<WorldSlot>,
    primary: WorldInstanceId,
    interaction_target: WorldInstanceId,
    transition: Option<WorldTransition>,
}
```

The composition camera is the physical flat/XR camera. Each slot receives an
inverse-transformed local camera for culling, traversal, interest, block/fluid
queries, and interactions. A portal derives a distinct destination view for
each eye; a scale/offset diorama normally reuses the physical view after model
placement. Per-eye mutable uniforms must remain distinct in both cases.

Only the primary authority consumes movement and interaction commands and may
reconcile the physical camera. Warm worlds continue to pump ordered updates and
may retain requested chunk views. If two unrelated servers both correct the
same camera, they will fight; selection and handoff are therefore explicit
runtime policy, not a rendering detail.

## Single-World Performance Invariant

The feature is not allowed to tax ordinary play significantly. The normal case
must stay structurally equivalent to today's direct path:

```text
one WorldSlot -> existing poll/cull/compile/upload/draw path
```

Composition is an optional outer scheduler used only when more than one slot is
warm or visible. In particular:

- do not add world-id hashing to per-section maps or hot draw loops;
- do not make every fragment evaluate dynamic portal/clip logic;
- keep specialized unbounded and clipped pipeline variants;
- keep `SceneSessionRuntime`, replicas, and render caches single-world;
- avoid per-frame allocation or virtual dispatch in the one-world path;
- give background worlds explicit update/compile/upload quotas rather than a
  full independent frame budget;
- share immutable assets, atlases, and worker capacity where practical, while
  keeping per-world dirty/cache state isolated;
- retain before/after frame-accounting and offscreen/XR performance evidence
  for every extraction that touches the existing hot path.

Tactical 174 fixes the flat release comparison anchor from five clean 240-frame
120 Hz frame-budget runs: 2.346 ms median average and 3.916 ms median p95. Their
within-batch ranges were 6.2% and 5.2% of the medians, respectively. One
isolated 9.765 ms maximum is retained in the evidence; the other maxima were
4.148–4.467 ms, and no run had an accounting violation. Ownership and
frame-path checkpoints rerun five release samples on the same machine. A range
greater than 10% is unstable, while a candidate median slowdown greater than
10% triggers investigation. Timedemo work counts remain useful, but its five
averages ranged from 1.889–3.699 ms despite an idle host and identical work, so
its frame timing is not currently a regression gate.

The first ownership proof adds only an integration test and no production-frame
collection, branch, namespace lookup, shader uniform, or render pass.

## The shrink-and-fall transition

Does **not** require two simultaneously authoritative simulations. Camera
scale-sweeps toward the diorama while the target world loads (as T0/T1/T2 or a
warm T3), then control hands off to that world's runtime and you "fall from its
sky." Watch depth precision: a large scale disparity compresses the nested
world's depth range — bounded and fine for a physically small tabletop, but the
transition (scale sweeping through orders of magnitude, world B filling the
view) is the z-fighting stress point; plan for a separate depth range or
reversed-Z there. A fully faked version (dive → fade → teleport into the single
real world spun up during the fade) buys most of the wow with zero nesting, and
is a valid first milestone.

## Feature → technique/tier map

| Feature | Technique | Tier |
|---|---|---|
| Lobby spawn room (+ no-destroy) | n/a | authored `WorldStore` + interaction gate |
| Last-world diorama on a table | embed | T0 baked |
| Seed explorer / worldgen console | embed | T1 live-local |
| Palantir into a network world | RTT window (or embed if reach-through) | T2 remote spectator |
| Warm Nether/sky/alternate generator | direct render after warm selection | T3 joined warm world |
| Walkable `x=0` seam | embed + complementary half-space visibility | T3 authority handoff |
| Traversable live portal | direct geometry + per-eye portal view/aperture | T3 authority handoff |
| Texture room (paint the atlas live) | n/a (atlas hot-reload) | orthogonal |
| Shrink-and-fall into a world | embed → runtime handoff | any; fakeable first |
| Cross-world shadows | requires new dynamic shadow subsystem | orthogonal, large |

## Recommended next direction

Ship independently valuable increments while keeping the one-world path direct:

The executable Slice 0–7 plan for items 1–3 and the opaque-gate proof lives in
[`174-warm-world-hot-swap.md`](../tactical/174-warm-world-hot-swap.md).

1. Review Tactical 174's Slice 1 field grouping, especially the decision to
   retain canonical `McloneSceneHostOptions` whole during the mechanical
   extraction. Then extract exactly one concrete drawable slot with no standby
   allocation or new frame branch.
2. Keep the dual-integrated-host ownership smoke green; extend it through
   persistent storage-root isolation and explicit shutdown evidence when the
   detached standby lands. Generalize active-plus-optional-standby to an N-world
   registry only after the bounded hot-swap milestone needs one.
3. Prove warm local switching: both worlds reach drawable readiness, switching
   selects an already-built terrain store, and neither runtime is reconstructed.
   Record switch latency and single-world before/after frame accounting.
4. Lobby spawn: authoritative world behavior profile + authored room.
5. T0 baked diorama: placement transform, transformed fog/culling, shared depth,
   and both mono/per-eye/multiview validation.
6. T1 seed explorer on the same geometry-placement path.
7. Add a real remote observer/subscription mode, then T2.
8. Add half-space visibility and boundary-aware meshing for an `x=0` render
   proof before attempting traversal.
9. Add one-active-authority local handoff, then a traversable seam or portal.
10. Treat federated remote authority and dynamic shadows as separate later
    campaigns.

Keep dynamic shadows as a separate topic when that work opens; it is the biggest
and most novel piece and is orthogonal to embedding. Every new render feature
here must carry both a per-eye and a multiview path per the XR render-path
guardrail — an XR portal that is not per-eye correct breaks stereo and defeats
the immersion that motivates the whole feature.
