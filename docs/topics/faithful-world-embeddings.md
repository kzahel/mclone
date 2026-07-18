# Faithful World Embeddings

Topic: `faithful-world-embeddings`

Status: **design exploration recorded 2026-07-18; no implementation tactical is
allocated. The working direction is to complement intrinsic bounded topology
with dimensions whose global slab, hinge, tube, cuboid, or later orthogonal
patch embedding is directly visible to the player. Canonical authority remains
integer and patch/sector-local rather than using one planet-sized floating-point
frame. The smallest gameplay target is a two-sided slab with opposing gravity;
the smallest renderer target is two grids meeting at a visible right angle. A
six-sector cuboid shell, bounded by an inaccessible magma mantle, is the leading
larger experience.**

This topic records player-visible, exact embeddings of ordinary voxel grids,
their authoritative local gravity frames, bounded underground shells, and the
relationship between canonical coordinates and globally visible placement. It
is separate from, and complementary to,
[`bounded-world-topology.md`](bounded-world-topology.md): that topic owns
intrinsic finite and glued topology, observer-local lifts, and optional visual
bending that cannot affect simulation. This topic asks what changes when the
player can see the exact global fold and gravity follows it.

It is also distinct from [`embedded-worlds.md`](embedded-worlds.md), which owns
placing one dimension or region inside another, warm world handoff, dioramas,
and shrink-and-fall transitions. A later hierarchy of worlds inside blocks may
combine both concerns without making either renderer recursion or simultaneous
simulation unbounded.

## Motivation

An intrinsic cylinder, torus, or cube atlas can preserve ordinary local
Minecraft movement while hiding the global construction behind observer-local
coordinates. That is technically useful, but the player may experience only a
flat world with surprising returns or frame changes. A faithful embedding makes
the shape itself part of play:

- an adjacent face is a visible destination rather than a concealed seam;
- buildings, rails, waterfalls, actors, and terrain can continue around a
  visible edge;
- top, bottom, and side settlements acquire a comprehensible spatial
  relationship;
- the finite world becomes a landmark and navigation system;
- underground gravity faults can become deliberate geology and exploration
  targets; and
- a small world can read as a physical toy or planet, especially from an
  elevated view, a parent diorama, or XR.

This does not remove the need for canonical identity, exact transitions,
topology-aware generation, or seam-aware meshing. It replaces an invisible
observer-local fiction with a visible physical explanation and moves some of
the hard policy into gravity, camera orientation, patch placement, and
subsurface ownership.

## Vocabulary

- **Faithful embedding** places every claimed patch through an exact rigid
  transform in one visible composition space. It does not smoothly bend,
  shrink, shear, or otherwise deform ordinary blocks.
- **Patch** is a bounded or unbounded local square-grid region with exact edge
  transitions and, when physically embedded, an exact placement frame.
- **Surface face** is one exposed side of a slab, hinge, tube, cube, or cuboid.
- **Face sector** or **gravity sector** is the volume extending inward from a
  surface face. In a cuboid shell it is a truncated wedge, not a constant-size
  rectangular prism.
- **Embedding frame** is an exact signed three-axis permutation plus integer
  translation used to place a patch relative to another patch or an observer.
- **Gravity frame** identifies the authoritative local down direction used for
  movement and other gravity-sensitive behavior.
- **Gravity fault** is the equal-distance boundary between two face sectors.
- **Mantle boundary** is the authoritative inner limit of a shell. Magma,
  lava, bedrock, crystals, or another fiction may explain it, but ordinary
  hazardous blocks do not replace authoritative denial.
- **Face priority** is a persisted total order used to give one canonical
  owner and orientation to an exactly equidistant block.

## World Family

### Two-sided slab planet

The smallest gameplay experiment is an unbounded or very large horizontal slab
with two inhabitable surfaces and a finite shared underground:

```text
top surface                         gravity inward
==================================================
                  upper underground
---------------- gravity divide -----------------
                  lower underground
==================================================
bottom surface                      gravity inward
```

The same canonical blocks form both worlds' crust. A player who digs far enough
from the top reaches the gravity divide. After the authoritative frame flips
180 degrees, continuing physically toward the other surface is locally digging
up. The player eventually emerges beneath the slab into the bottom-face world.

This proof can retain one ordinary column coordinate system and finite vertical
sections. Terrain chunks need not be rotated merely to draw the slab; the main
new work is inverted player/entity orientation and gravity-sensitive behavior.
It therefore isolates gravity, camera, collision, placement, and upside-down
presentation before adding visible orthogonal terrain frames.

The exact divide remains a gameplay decision. It could be a sharp transition,
a short animated transition with authoritative hysteresis, a distinctive
zero-gravity or crystal layer, or a later player-selectable gravity latch.

### Right-angle hinge

The smallest faithful renderer experiment is two bounded or half-infinite
square-grid patches meeting at one visible 90-degree edge:

```text
face A  __________________
                         |
                         |
                         |  face B
                         |
```

One face uses an exact quarter-turn rigid transform relative to the other. The
first proof may be an authored, non-traversable fixture. It tests rotated chunk
placement, shared depth, culling, seam meshing, lighting, interaction mapping,
and mono/stereo/multiview presentation without requiring a closed planet,
corners, or a core.

A traversable follow-up adds a 90-degree gravity-frame change, player and actor
orientation, velocity policy, camera reconcile, and authoritative interaction
across the edge.

### Four-face square tube

Four grids can form a square-prism belt. A player can walk a complete visible
loop through four right-angle transitions while the dimension remains
unbounded or separately bounded along the prism axis. This proves closure and
stable frame return without the three-face corners or caps of a cuboid.

It is the faithfully embedded counterpart of an intrinsically periodic
cylinder: the canonical loop may use related exact transition machinery, but
the player sees four rigid faces rather than a locally flat quotient or smooth
presentation bend.

### Cuboid shell

A **cuboid** is a rectangular box; a cube is the equal-sided special case. The
product-facing names may simply be *cuboid world*, *cube planet*, or *cubical
shell world*.

The leading larger experience has six exposed faces and a finite playable shell
surrounding an inaccessible inner cuboid. The world does not model an infinite
depth or the entire solid core. Its ordinary underground ends at a configured
mantle thickness followed by an authoritative boundary explained through
magma, lava, bedrock, crystals, or another world fiction.

In a two-dimensional cross-section, face sectors are separated by diagonal
equal-distance boundaries:

```text
                         top sector
              +-----------------------------+
              |\                           /|
              | \                         / |
 left sector  |  +-----------------------+  |  right sector
              |  |  inaccessible mantle |  |
              |  |       and core        |  |
              |  +-----------------------+  |
              | /                         \ |
              |/                           \|
              +-----------------------------+
                        bottom sector
```

The same construction extends in three dimensions. Fault planes descend from
each outer cuboid edge toward the matching edge of the inner mantle. Three
sectors meet along the corresponding corner loci. If the mantle were removed,
the sectors would eventually collapse at the center; the bounded shell
deliberately stops first.

At depth `d`, a face sector's valid cross-section shrinks rather than remaining
a constant-size face prism. For an equal-sided cube of width `N`, its linear
extent is conceptually `N - 2d`, subject to the exact cell-boundary convention.
This is the triangular or wedge-shaped inward cut required by a literal cuboid
embedding.

### Mixed orthogonal patch worlds

The same contracts could express built-in arrangements beyond planets:

- two faces joined at a right angle;
- an inside-out room or box;
- stair-step and zigzag worlds;
- finite polycube surfaces;
- selected orientable square-grid patch graphs; and
- abstract gluings that intentionally have no non-overlapping physical
  embedding.

An exact topology graph and a faithful physical embedding are separate
validation claims. A patch graph may be intrinsically valid while overlapping,
self-intersecting, or failing to close in composition space. Initial faithful
worlds should be validated built-ins with no user-authored arbitrary graph.
Traversable manifold edges should initially have exactly two incident patches.

## Canonical Authority Without Planet-Sized Floats

A visible global embedding does not require authoritative global floating-point
coordinates. Block and object identity should stay integral and patch-local.
One conceptual address is:

```text
SectorBlockAddress {
    sector_or_patch
    u
    v
    normal_height_or_depth
}
```

Large worlds should split `u` and `v` into an integer chunk address and bounded
block-local coordinates. A continuous pose can similarly retain a canonical
cell/chunk anchor, a bounded fractional offset, and an exact local frame. The
configured face extent may be extremely large, but a closed cube or cuboid is
necessarily finite; increasing it far beyond observation makes its ordinary
face experience approach a plane.

Crossing a patch boundary canonicalizes the address and returns an exact signed
three-axis permutation. A faithful 90-degree edge cannot use only the
horizontal frame transform proposed for a locally unfolded cube atlas: the old
vertical axis becomes tangent to the adjacent face and the new face normal
becomes vertical for gravity and local movement.

Rendering rebases canonical positions around the observer before converting to
GPU floats. A face or section transform then places the small relative value in
composition space. A distant view capable of seeing the entire large planet
uses a topology-aware LOD representation; it does not require simultaneous
block precision at planetary scale.

Neither persistence nor protocol facts store an observer's embedding-space
image as canonical identity. One block or actor remains one object even if a
diagnostic or distant presentation supplies another representation.

## Cuboid Sector Ownership and Block Orientation

For the first cuboid model, the closest outer face owns an underground block.
Exactly equal claims are resolved by a persisted total face priority:

```text
owner(block) =
    minimum by (distance to outer face, face priority rank)
```

The precise order is a dimension-definition choice. It must be explicit and
persisted rather than inherited accidentally from an enum discriminant. A
total order automatically resolves both two-face edge ties and three-face
corner ties; an arbitrary per-edge table could create contradictory cycles.

The winning face owns all initial semantics for that block:

- canonical generation, persistence, and scheduled work;
- block-model orientation and the direction treated as up for grass;
- local lighting, liquid, falling-block, and other directional policy;
- the default gravity sector at the block center; and
- mesh and interaction ownership so the block is drawn and targeted once.

This intentionally breaks perfect cuboid symmetry along the exact ownership
fault in exchange for a much smaller and deterministic first contract. An
exterior edge grass block uses the winning face's grass-top orientation; its
other exposed side uses the ordinary side texture. Multi-face coatings or
explicit edge/corner strata may be added later if the visual result warrants
their complexity, without changing the one canonical owner rule.

Continuous player poses use the same nearest-face rule. A small authoritative
hysteresis band should prevent gravity and input from oscillating when noise or
movement leaves the player near a fault. Face priority resolves exact equality;
hysteresis stabilizes traversal history but does not create a second block
identity.

Underground faults may otherwise appear arbitrary because the exterior edge is
not visible. Generator profiles can communicate them through distinctive
strata, crystals, magma, particles, audio, compass behavior, or deliberate
transition chambers. Such content is an explanation of the gravity policy, not
its authority.

## Rendering Contract

The exact draw concept is:

```text
canonical mesh
    + observer-relative lift/rebase
    + exact patch embedding frame
    + per-view projection
```

A signed axis permutation and integer translation preserve block size and
right angles. They are lower risk than a nonlinear curvature warp: the same
terrain mesh and material pipeline can be reused through a per-patch or
per-draw rigid transform, and hits can use the exact inverse. The renderer must
nevertheless carry patch identity and transformed placement coherently.

The existing placed-world work in [`embedded-worlds.md`](embedded-worlds.md)
already proves transformed geometry, shared depth, actor placement, and
per-eye/full-frame composition. Faithful embeddings require reusable rigid
rotation in addition to the current placement cases, plus topology-aware seam
ownership rather than two unrelated world slots.

All claimed world-space consumers must agree:

- opaque, cutout, and translucent terrain;
- boundary-aware mesh faces, AO, biome tint, and packed light;
- actors, player bodies, held items, particles, fluids, and block outlines;
- ray casts and interaction hit conversion;
- shadows and screen/world effects when those features exist;
- sky, fog, clouds, Far LOD, frustum culling, and interest selection; and
- mono, separate per-eye, and full-frame multiview paths.

The scene can inverse-transform an observer into each nearby patch for culling,
interest, and interaction queries. A common one-face view should retain a
direct fast path; intentionally small worlds and edge views may draw several
patches at once.

## Gravity and Transition Policy

Gravity is authoritative gameplay state in this topic, not a visual-only
effect. At minimum the selected policy must define:

- which sector currently owns the player or actor frame;
- whether gravity changes sharply or through a bounded transition state;
- whether velocity rotates with the frame, remains embedding-space inertial,
  or is damped during a transition;
- how yaw, pitch, body orientation, step/sneak, jumping, swimming, flight, and
  camera reconcile behave;
- whether fluids, falling blocks, projectiles, items, mobs, and particles use
  the same spatially varying gravity immediately or through later capability
  stages; and
- how teleport, respawn, join, and dimension transfer choose a deterministic
  initial frame.

The smallest axis-aligned implementation should keep gravity cardinal and use
an animated presentation transition around a discrete authoritative frame
change. Smooth arbitrary-angle gravity is a separate and significantly larger
physics contract. Player-selectable cardinal gravity may later be a creative or
puzzle mechanic built on the same frame representation.

Flat-screen and XR presentations may need different comfort treatments while
retaining the same authoritative transition. A short camera roll may work on a
monitor; XR may require a vignette, blink, tunnel, deliberate latch, or other
comfort presentation rather than rotating the user's horizon unexpectedly.

## Nested and Fractal World Extension

A later product can make selected blocks contain another cuboid, slab, or other
dimension and use the completed embedded-world placement and warm-handoff work
to enter it. The dimension graph may be recursively addressable without
recursively rendering or simulating every descendant:

- a parent dimension plus stable anchor identifies a child dimension;
- unvisited children need not be provisioned or persisted;
- the current world plus a bounded parent/child preview is sufficient;
- selection can scale or cover into an ordinary full-size destination runtime;
- an edge transition may carry an orientation and gravity-frame relationship;
  and
- scale belongs to the dimension handoff, not to ordinary within-dimension
  topology, so one local block remains one block on either side.

The existing non-recursive composition invariant should remain until a concrete
experience and measured recursion budget justify changing it. A conceptual
fractal universe does not require an infinitely recursive frame. Rare explicit
world-bearing blocks are a safer first product than assigning an eagerly live
world to every ordinary block, especially because anchor destruction,
persistence, player occupancy, and return routing need explicit rules.

## Hard Invariants

1. Canonical authority never depends on planet-sized floating-point positions.
2. A claimed faithful embedding uses exact lattice-preserving rigid transforms;
   nonlinear bending remains a separate presentation effect.
3. Each authoritative block, entity, tick, and structure has one canonical
   owner and is simulated, persisted, drawn, and interacted with once.
4. The initial cuboid contract resolves equal-distance block claims through one
   explicit persisted total face priority.
5. Gravity and local frame changes that affect movement or simulation are
   authoritative shared-engine facts, not renderer policy.
6. A mantle, magma sea, fog, or other fiction cannot replace an authoritative
   finite-depth boundary.
7. Generation, meshing, lighting, liquids, collision, actors, interest,
   persistence, and interaction agree on patch transitions and gravity-sector
   ownership.
8. The ordinary unbounded Euclidean Overworld and intrinsic bounded-topology
   profiles remain unchanged unless explicitly opted into a faithful embedding.
9. Platform apps do not own topology, gravity, or embedding semantics.
10. Any visible world-space feature implements mono, per-eye, and full-frame
    multiview paths or explicitly records why it is unavailable.

## Proposed Proof Ladder

No tactical is allocated. A future plan should keep each proof independently
reviewable:

### 0. Contract and ordinary baseline

- Lock the direct Euclidean one-world path and representative movement/render
  traces.
- Define exact signed three-axis frames, canonical patch address concepts, and
  opt-in dimension metadata without changing ordinary dimensions.
- Keep topology, generation profile, faithful embedding, and gravity policy as
  distinct validated dimension facts even when a built-in profile selects them
  together.

### 1. Static right-angle renderer hinge

- Draw two authored ordinary terrain patches at a visible 90-degree hinge.
- Keep the fixture non-traversable and use exact shared-depth placement.
- Inspect mono and stereo/offscreen captures; cover the full-frame multiview
  path on capable hardware.
- Prove the direct one-patch path remains unchanged.

### 2. Two-sided slab gameplay

- Use one finite-depth slab with top and bottom surfaces and ordinary column
  storage.
- Add cardinal positive/negative gravity frames, inverted camera/input/player
  presentation, and a deterministic gravity divide.
- Dig from one surface, cross under an explicit transition presentation, and
  emerge on the other side.
- Bound the first proof to player movement and name every gravity-sensitive
  subsystem not yet enabled.

### 3. Traversable hinge

- Add exact canonical adjacency and seam-aware mesh/lighting facts.
- Cross the hinge with a 90-degree authoritative frame change and explicit
  velocity/camera policy.
- Interact with blocks and another actor across the visible seam.

### 4. Four-face square tube

- Close four hinges into a loop without caps or three-face corners.
- Prove stable block identity, frame return, save/reopen, generation seams,
  interest deduplication, and multiplayer interaction around a full circuit.

### 5. Cuboid shell

- Add six face sectors, a finite depth, diagonal fault ownership, persisted
  face priority, and a hard inner mantle.
- Generate an intentionally small or authored world whose adjacent faces and
  overall shape are visually legible.
- Exercise surface edges, underground faults, two- and three-way ties, and
  movement near the mantle without representing the core.
- Add topology-aware distant presentation or LOD only after local full-detail
  seams are correct.

### 6. Mixed and nested worlds

- Validate selected built-in orthogonal patch arrangements before exposing an
  arbitrary graph.
- Combine a bounded faithful world with one explicit world-bearing block and a
  bounded embedded preview/handoff.
- Retain non-recursive rendering and bounded live runtimes while proving a
  stable return relationship.

## Validation Themes

- exact frame composition and inverse property tests;
- canonical step-across/step-back identity at 90- and 180-degree transitions;
- deterministic face-priority ownership for edge and corner ties;
- stable block orientation, generation, and persistence across save/reopen;
- camera-relative precision at large face-local chunk addresses;
- no duplicate draw, tick, ticket, interaction, or persistence identities;
- seam face culling, AO, tint, light, fluids, actors, and outlines;
- authoritative gravity agreement between server validation and client
  prediction;
- stable camera and velocity behavior across faults and after full loops;
- player/entity collision and interaction while opposite frames are nearby;
- slab top-to-bottom traversal and cuboid surface/underground traversal;
- visible global placement, frustum/interest selection, sky, fog, and LOD;
- native offscreen capture inspection at the first drawable milestone;
- browser, Android, XR packaging, and platform-neutral normalized traces; and
- mono, per-eye, and full-frame multiview coverage for every enabled
  world-space feature.

## Open Questions

- What exact persisted descriptors separately identify topology, faithful
  embedding, gravity policy, mantle depth, and face priority?
- Does the slab flip gravity at a zero-width divide, across a transition band,
  or through an authored intermediate layer?
- Does velocity rotate, remain inertial, or damp during the first 90- and
  180-degree transitions?
- Which gravity-sensitive systems join the player in the first proof, and
  which are explicitly unavailable?
- What player collision representation can straddle a seam without assuming
  global Y-up AABBs?
- How are face-local chunks and shrinking sector bounds encoded without
  introducing a general 3D chunk scheduler?
- Which total face priority produces the least visually surprising cuboid
  edge and corner ownership?
- Does generated grass use only canonical-owner orientation initially, and
  are edge/corner coatings ever worth a broader content contract?
- How does world generation communicate underground gravity faults and make
  them useful rather than arbitrary surprises?
- What shape and thickness does the mantle require relative to generation,
  mining, fluid, spawn, and interaction radii?
- How do sky, clouds, weather, sunlight, Far LOD, and audio behave when several
  orthogonal faces are simultaneously visible?
- What face width makes the global shape legible while leaving enough ordinary
  Minecraft-like exploration space?
- Which mixed patch graphs receive a faithful non-overlap/closure validator,
  and which remain intentionally intrinsic-only?
- How are child-world identity, anchor destruction, occupancy, persistence,
  scale, orientation, and return routing defined before world-bearing blocks
  become ordinary gameplay?

## Related

- [`bounded-world-topology.md`](bounded-world-topology.md)
- [`embedded-worlds.md`](embedded-worlds.md)
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`../native-engine-architecture.md`](../native-engine-architecture.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../protocol.md`](../protocol.md)
- [`../platforms.md`](../platforms.md)
