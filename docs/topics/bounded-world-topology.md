# Bounded World Topology

Topic: `bounded-world-topology`

Status: **Cylinder v0 complete 2026-07-18. Tactical 195 landed the exact
Euclidean baseline, finite-bound authority canary, and a persisted/networked
32-chunk X-periodic Flat Grass dimension across shared server, client,
renderer, desktop, browser, Android, and available XR lanes. Authority stores
one canonical ring; observation chooses a nearby lift of the same chunks,
actors, meshes, and uploads. Movement, collision, targeting, lighting, fluids,
multiplayer, save/reopen, TCP/WebSocket transport, mono/stereo/multiview
rendering, and opt-in diagnostics agree on the seam. Crossing performs no bulk
geometry shift or remesh. Tactical 196 subsequently completed the exact
384-chunk periodic Mclone terrain, surface, and vegetation caller. Future
distant-terrain presentation, natural spawning, multiple visible lifts, torus
topology, nonlinear horizon bending, and patch atlases remain explicit
follow-ups. The selected long-term model remains an exact locally
Euclidean voxel world with dimension-owned finite, periodic, or later
patch-glued horizontal topology. A flat torus follows the cylinder; a six-face
cube atlas with inaccessible vertex regions comes before any true spherical
regional-atlas integration.**

A 2026-07-26 design follow-up now records the dimension-scoped context,
canonical/work/presentation-lift split, wrapped 3D-density direction,
periodic macro-planning obligations, seam-feature policy, and
topology-specific distant-terrain constraints. These are contracts for later
work, not claims that torus generation or periodic horizon rendering has
landed.

[`Tactical 257`](../tactical/257-topology-worldgen-conformance-probe.md) is
active. It adds a hidden adversarial generation profile that guarantees
terrain, water, bounded geometry, lighting, and persistence canaries at
periodic seams, plus a reusable conformance suite for future terrain
families. It complements rather than replaces the Flat Grass runtime proof
and the real Mclone cylinder.

## Cylinder v0 Support

| Family | Plane | Finite | Cylinder X | Later topology |
|---|---|---|---|---|
| Descriptor, protocol, persistence | supported | supported | supported | torus/atlas pending |
| Authority, views, tickets, gameplay | supported | supported | supported | pending |
| Client replica, mesh, light, fluids | supported | supported | supported | pending |
| Flat Grass generation | supported | supported | supported | torus pending |
| Authored Only generation | supported | fixture support | fixture support | design only |
| Mclone Overworld generation | supported | unsupported | supported at 384 chunks | design only |
| Reference/Alpha/Beta/Small Island | supported | unsupported | unsupported | unsupported |
| Distant terrain presentation | no current product | unclaimed | unclaimed | new architecture pending |
| Natural spawning | supported | unclaimed | explicit rejection | pending |
| Particles | supported baseline | unclaimed | unclaimed | pending |

The cylinder production path is validated through SQLite and IndexedDB,
in-memory/TCP/WebSocket sessions, desktop, the production browser Worker,
Android APK/AVD, synthetic stereo, OpenXR compile, and Quest APK packaging. A
physical Quest was not attached for the closeout, so device execution remains
unclaimed rather than inferred from the proxy lanes. The one-lift invariant
still rejects views wide enough to show two images of a canonical chunk.

This topic owns the continuing product and engine direction for finite worlds,
looping worlds, exact grid-edge identifications, inaccessible topology regions,
topology-aware terrain generation, observer-local lifts, and optional visual
curvature. It does not make the separate spherical voxel laboratory a source
dependency or declare that its open regional-rasterization questions are
solved.

## Motivation

Mclone should be able to host dimensions that are bounded or loop back on
themselves without changing ordinary Minecraft-like movement, collision, block
size, or local distance. Useful examples include:

- a finite rectangular authored or generated world with authoritative edges;
- a cylinder that loops east/west but is unbounded north/south;
- a finite cylinder whose north/south extents become inaccessible polar plains,
  fog banks, walls, oceans, or other deliberate caps;
- a flat torus that loops on both horizontal axes and therefore has a finite,
  edgeless canonical block set;
- a cube-surface atlas whose six ordinary square grids meet across rotated
  edges while regions around the eight non-Euclidean vertices remain
  inaccessible;
- later exotic exact grid gluings using translations, quarter turns, or
  reflections; and
- only after those exact cases, a true spherical experiment with frozen
  regional voxel ownership and approximate observer rasterization.

The flat periodic cases are useful independently of the sphere. They provide
finite storage, finite exploration, stable loop return, and seamless multiplayer
without first solving how an ordinary square voxel presentation can approximate
a continuously curved sphere.

## Core Decision

Every accessible gameplay neighborhood behaves as an ordinary Euclidean voxel
grid. Global topology comes from finite extents and distance-preserving grid
gluings, not from changing the simulation metric.

```text
canonical dimension topology and object identity
    finite cells, periodic identities, patch ownership, exclusions
                         |
                         v
observer-local Euclidean lift
    ordinary xyz movement, blocks, collision, ray casts, and simulation
                         |
                         v
optional presentation transform
    flat, cylindrical horizon bend, cube overview, or another visual effect
```

One walked block remains one block. Blocks do not shrink toward a pole or an
inner radius. Collision boxes, simulation velocity, reach, lighting adjacency,
fluid adjacency, entity distance, and path costs use the exact grid topology.
No server or generator computes a curved visual embedding.

The mathematical description is **locally isometric to Euclidean space**. A
cylinder and a flat torus are flat Euclidean quotients. A cube surface is
piecewise Euclidean: face interiors and non-vertex edge neighborhoods unfold
exactly into a plane, while curvature is concentrated at its eight vertices.
The engine contract needs only integer grid isometries and canonical ownership;
it does not need metric tensors, Gaussian-curvature evaluation, arbitrary smooth
charts, or geodesic integration.

## Vocabulary

- **Topology descriptor** is immutable, persisted dimension metadata describing
  finite axes, periodic axes, or a later validated patch atlas.
- **Canonical address** is the sole authoritative identity of a block, chunk,
  entity location, scheduled tick, or other dimension-local spatial fact.
- **Local lift** is an observer-relative Euclidean image of a canonical address.
  A canonical address can have more than one visible lift in an intentionally
  small periodic world, but those images do not create more simulation objects.
- **Grid transition** maps one patch edge to another through an exact integer
  translation and an optional square-grid frame isometry.
- **Frame transform** is a signed horizontal-axis permutation: initially the
  identity, then quarter turns, and only later an intentional reflection.
- **Finite boundary** means the topology contains no traversable cell beyond a
  configured extent. Terrain and presentation may explain the edge, but the
  authority enforces it.
- **Excluded region** is canonical space retained as an explicit non-playable
  topology fact. Simulation does not rely only on a visual fog effect to keep
  players out.
- **Presentation transform** changes submitted visual positions after a local
  lift is chosen. It cannot change canonical identity or simulation results.

## Topology Family

The first descriptor should express each horizontal axis independently. Names
and encoding remain an implementation decision, but the semantic vocabulary is:

```text
AxisExtent =
    Unbounded
    | Finite { minimum, maximum_exclusive }
    | Periodic { minimum, period }
```

Initial finite and periodic extents should be aligned to whole chunks. That
keeps canonical chunk identity, persistence, worker inputs, meshing halos, and
interest reconciliation exact before considering partial boundary chunks.
Vertical `min_y` and height remain the existing finite dimension facts rather
than another looping axis.

This small vocabulary produces several useful worlds:

| Horizontal X | Horizontal Z | Result |
| --- | --- | --- |
| unbounded | unbounded | current ordinary plane |
| finite | finite | finite rectangular world |
| finite | unbounded | finite strip |
| periodic | unbounded | infinite cylinder |
| periodic | finite | bounded or capped cylinder |
| periodic | periodic | finite flat torus |

A finite axis has an authoritative hard boundary even when the generator fills
the approach with inaccessible plains, mountains, ocean, bedrock, fog, or a
world-specific border. This prevents missing terrain, unloaded chunks, or a
presentation effect from becoming security or correctness policy.

The later patch-atlas form generalizes axis extents without replacing them:

```text
GridPatch {
    stable patch identity
    finite integer bounds
    edge transition per traversable edge
    excluded canonical regions
}

GridTransition {
    target patch and edge
    integer translation / phase
    horizontal grid-frame isometry
}
```

The first runtime need not persist an arbitrary user-authored patch graph.
Consumers should call topology operations rather than open-code modulo
arithmetic so a validated built-in patch atlas can be added later.

## Exact Grid-Transition Contract

Supported transitions preserve the square voxel lattice and one-block distance.
The horizontal linear part is drawn from the symmetries of the square:

- identity;
- rotations by 90, 180, or 270 degrees; and
- an explicitly non-orientable future reflection.

An integer offset carries grid phase. A transition may therefore translate a
periodic seam, rotate coordinates between cube faces, or later reverse one axis
for a Möbius- or Klein-bottle-like experiment without changing local block
dimensions.

Shear, arbitrary rotation, nonuniform scale, and nonlinear projection are not
simulation transitions. They do not preserve the ordinary voxel metric and
belong, if useful, only to presentation or to the separate approximate
spherical research path.

At minimum, shared topology ownership must eventually provide operations with
these meanings:

- validate and persist a topology descriptor;
- canonicalize continuous poses, blocks, and chunks in one dimension context;
- step from one canonical cell to an adjacent cell, returning the accumulated
  frame transform when a boundary is crossed;
- reject traversal beyond a finite boundary or into an excluded region;
- compute the nearby/shortest lift of a canonical pose relative to an observer;
- enumerate a view's canonical chunks and their presentation lifts while
  deduplicating authority and scheduling work;
- compute topology-aware local displacement for movement, collision, entity
  proximity, interpolation, and path cost; and
- map local interaction hits back to one canonical block or entity identity.

`BlockPos`, `ChunkPos`, or `Vec3d` must not silently apply a period. They lack
dimension context, and equal coordinates can live in dimensions with different
topologies. Existing Euclidean values may remain efficient leaf representations
inside a known runtime, but topology-dependent boundary operations belong to
the owning dimension.

## Dimension-Scoped Topology Context

Several dimensions may be loaded, generated, simulated, observed, or
transferred concurrently. There must therefore be no process-global,
thread-local, app-local, or otherwise implicit “current topology.” Every
topology-dependent operation receives the immutable dimension fact that owns
its coordinates.

A useful conceptual split is:

```text
DimensionSpatialContext
    dimension key
    horizontal topology
    vertical bounds and environment facts

WorldgenContext
    dimension spatial context
    seed
    generation profile descriptor
```

These names and exact Rust types are not locked. The contract is that a
server/runtime dimension owns the immutable source facts, asynchronous jobs
carry a stable snapshot or reference, and samplers/planners retain the compact
facts they repeatedly need. `HorizontalTopology` is already a small value and
need not acquire reference counting merely to avoid copying it; a larger
descriptor may be shared when that is useful.

Topology-aware interfaces should not make every arithmetic leaf accept a
large context:

- samplers and planners are constructed from the appropriate dimension
  context and retain topology;
- bounded algorithms use one coherent target-relative Euclidean work lift;
- interpolation, local signed-distance arithmetic, and density composition
  remain ordinary local calculations;
- reads, writes, ownership, neighborhoods, queries, and cache keys cross
  through topology-aware operations; and
- presentation chooses observer lifts only after canonical identity is known.

The reusable operation vocabulary should grow around semantic boundaries
rather than raw modulo:

- canonical owner for a position, cell, chunk, or plan;
- topology neighbor and finite/excluded-region rejection;
- shortest displacement and deterministic half-period tie;
- nearest observer or target-relative lift;
- wrapped bounds or the finite set of local lifts intersecting a query;
- unique canonical cells intersecting a lifted region; and
- canonical dependency/feature enumeration with stable deduplication.

This keeps three coordinates distinct:

1. **canonical identity** owns persistence, caching, random identity, and
   authoritative output;
2. **work lift** gives one local Euclidean neighborhood to a planner,
   generator, feature, or mesher; and
3. **presentation lift** places canonical content relative to one observer.

Cache and worker identity must include dimension/profile facts and every
topology value that affects output. GPU procedural evaluators receive an
explicit serialized topology mode and periods or reject the profile/topology
pair; shaders must not infer topology from coordinates or use an unrelated
floating-point wrap convention.

## Canonical Identity and Stable Objects

Canonical identity is independent of observer presentation:

- one periodic block has one stored state even if two lifts are visible;
- one entity has one authority identity even if a diagnostic view draws two
  images;
- generated and authored blocks use the same canonical addressing rules;
- scheduled ticks, lighting nodes, fluids, block entities, path nodes, and
  persistence records address the same canonical neighbors; and
- a structure crossing a seam retains one stable owner/root and canonical cell
  set rather than being generated or saved once per lift.

For a periodic axis, canonical continuous coordinates remain in a bounded
fundamental interval. Ordinary movement near a seam uses the lift closest to
the previous accepted pose, so crossing from the maximum coordinate to the
minimum is not interpreted as a high-speed teleport. Explicit teleport,
dimension transfer, respawn, and join operations reset that continuity anchor
instead of guessing a winding history.

Objects larger than half a periodic extent and views wide enough to contain
multiple lifts are deliberately ambiguous presentation cases. The first proof
should use extents comfortably larger than twice the interaction and chunk
tracking radii. Later tests should intentionally admit multiple lifts, draw
them as images of one identity, and select exactly one eligible interactive
lift under an explicit nearest-lift policy.

## Finite Boundaries and Inaccessible Regions

Finite worlds are first-class topology, not merely generators that happen to
return empty chunks. The authority must distinguish:

- a valid loaded cell;
- a valid but currently unloaded cell;
- a canonical excluded cell or region; and
- a coordinate outside the dimension's finite topology.

Movement, teleport, spawn, placement, pathfinding, entity spawning, scheduled
simulation, chunk tickets, and observer interest must all reject or clip the
last two cases consistently. The client may present a world border, opaque fog,
mountain wall, ocean horizon, impassable ice/plain, or another fiction, but it
may not manufacture traversable authority beyond the bound.

A bounded cylinder is the motivating composition: its periodic axis provides
seamless circumnavigation, while its finite axis approaches two inaccessible
caps that can read as approximate north/south polar regions. The caps can be
wide generated transition areas inside valid canonical space, followed by an
authoritative final boundary hidden under terrain and fog.

Excluded regions are also the intended cube-vertex policy. Keeping them as
typed canonical facts rather than deleting them from the topology allows the
dimension to remain conceptually one bounded cube-surface world while clearly
denying gameplay semantics where the ordinary grid contract is unavailable.

## Topology-Aware Generation

Topology and generation profile are orthogonal dimension facts:

```text
DimensionDefinition
    seed
    generation profile
    topology descriptor
    vertical/environment facts
```

The topology determines canonical identity, bounds, adjacency, and observation.
The generation profile determines terrain content. Vanilla Overworld remains
on the unbounded Euclidean topology and retains its reference-locked output.
Alternate internal profiles may opt into finite or periodic topology without
changing the meaning of the Overworld profile.

Generation must respect the selected topology:

- finite generation plans cannot schedule outputs outside canonical bounds;
- periodic generation must produce compatible values and features at identified
  edges rather than merely applying modulo after unrelated noise samples;
- dependency halos and feature regions cross seams through canonical neighbor
  operations and deduplicate wrapped chunks;
- structures and decorations use deterministic canonical ownership at seams;
- spawn selection excludes hard boundaries and inaccessible caps;
- persistence stores only canonical chunks, never observer lifts; and
- generator diagnostics distinguish topology seams, generation discontinuities,
  and presentation artifacts.

A flat profile is the safest first seam proof. The landed Mclone cylinder uses
generator-owned wrapped lattice fields whose values and first derivatives meet
at the seam. Circle embedding remains a possible fallback for an exceptional
field whose required scales cannot fit an accepted period; it is not the
default periodic-noise direction. A cube atlas can derive a shared canonical
sample from face coordinates so neighboring faces agree along an edge. These
are generator implementation techniques, not topology APIs.

Generator profiles used with a topology need an explicit compatibility policy.
Not every existing generator must support every topology, and unsupported
profile/topology pairs must fail at dimension creation or load rather than
silently produce seams or invalid schedules. Any persisted identity or output
promise must be entered in the compatibility safety ledger in
[`world-generation-profiles.md`](world-generation-profiles.md) when allocated.

## Periodic Fields, Density, And Feature Seams

Periodicity does not inherently raise a field's dimensionality. A
three-dimensional density lattice on an X-periodic cylinder remains a 3D
lattice: its X lattice identity wraps while Y and Z retain ordinary local
coordinates. A flat torus wraps both horizontal lattice identities.

Required cylinder identities include:

```text
D(x + period_x, y, z) = D(x, y, z)
dD/dx(x + period_x, y, z) = dD/dx(x, y, z)
```

A torus adds the equivalent Z identities. Matching gradients matters for
surface normals, slope-derived classification, interpolated density cells, and
seam-free isosurface geometry, not merely for matching scalar samples.

The current wrapped-lattice implementation requires each periodic horizontal
scale to divide the block period. The supported 6,144-block Mclone cylinder
admits many useful scales, including 32, 48, 64, 96, 128, 192, 256, 384, 512,
768, 1,024, 1,536, and 2,048 blocks. A future density tactical should prefer a
compatible cell/field vocabulary before introducing higher-dimensional noise.
Circle embedding would turn a cylinder-periodic 3D field into a 4D evaluation
and a two-axis-periodic field into a 5D evaluation; that cost requires an
explicit quality reason and benchmark.

Domain warps are periodic only when every warp affecting a periodic axis and
every field receiving the warped coordinate preserve the same period. An
ordinary planar warp feeding a wrapped field is not topology support.

Bounded features and volumetric formations should cross a periodic seam through
the same canonical-owner/work-lift contract as current stream structures:

- select and randomize one canonical start;
- choose the local lift relevant to the target query;
- evaluate ordinary Euclidean bounds, signed distance, or density there;
- canonicalize outputs and dependency ownership once; and
- query other features through topology-aware displacement and bounds.

Keeping an ordinary bounded influence below half the relevant period avoids
nearest-image ambiguity. Larger or non-contractible formations need an
explicit quotient-space design instead of an oversized planar bounding box.

A periodic seam is not a physical world boundary. Suppressing expensive or
complex features near the canonical wrap would create a permanent featureless
meridian and make an arbitrary coordinate convention visible. That is not an
accepted general fallback. Legitimate exceptions are:

- finite edges, polar caps, and excluded patch-atlas regions that are real
  topology facts;
- a deliberately authored ocean belt, tectonic scar, wall, or other named
  landscape family; or
- a temporary bounded tactical whose seam exclusion and removal condition are
  explicit.

Until a family is seam-safe, rejecting that family for the whole periodic
profile/topology pair is more honest than silently deleting it near one seam.

## Periodic Macro Planning And Hydrology

Macro planners must operate on topology graphs, not planar rectangles whose
final coordinates happen to be canonicalized.

- canonical planning-cell sizes should divide periodic extents where practical;
- halos and adjacency wrap and deduplicate at every seam and torus corner;
- routing heuristics use shortest topology displacement and a stable
  half-period tie;
- plan IDs, starts, random identity, claims, and density budgets are canonical;
- local route geometry uses one coherent work lift; and
- no point sample performs a new global traversal merely because the query is
  near a seam.

An X-periodic cylinder remains unbounded in Z, so drainage may cross the X seam
while retaining ordinary north/south extent and outlets. A flat torus has no
external edge: every drainage path must terminate in an ocean, lake, wetland,
closed basin, or other explicit sink, and a finite routed graph must detect
cycles. A river cannot descend monotonically around a closed
non-contractible loop and return to its starting elevation. Such a loop must
be rejected, broken through basin/spill semantics, or deliberately classified
as flat lake-like, tidal, or otherwise non-river water.

Periodic worlds can make some global summaries easier because their canonical
domain is finite. That does not justify an unbounded flood fill during a column
sample. Whole-domain or hierarchical products still need a revisioned bounded
planner, deterministic construction boundary, and cache policy.

## Observer Lifts, Interest, and Rendering

Server authority, the client replica, and presentation have distinct ownership:

```text
server: canonical chunks, poses, entities, adjacency, and tickets
client replica: canonical published facts for the active dimension
scene/render session: observer anchor and selected local lifts
renderer: lifted Euclidean geometry plus optional visual transform
```

Chunk interest near a periodic seam asks for the canonical neighborhood, not an
unbounded coordinate rectangle. Equal wrapped chunks are scheduled, generated,
lit, persisted, and published once. The client may associate one or more
presentation lifts with that canonical content without cloning the replica
fact or mesh authority.

Terrain section identity and draw identity therefore cannot be assumed to be
the same forever. Conceptually a draw is:

```text
canonical render section + observer-relative lift + presentation transform
```

The common large-world case should retain one lift and its current fast path.
Multiple lift instances remain opt-in work for intentionally tiny worlds.

Meshing must query canonical topology neighbors at a seam so face culling,
ambient occlusion, biome sampling, and packed light do not expose an artificial
edge. Renderer traversal, future distant terrain, actors, particles, world
overlays, block outlines, fluids, shadows, and interaction presentation must
all choose lifts coherently.

## Periodic Distant Terrain

The planar horizon budget cannot be applied to a small periodic world by
merely canonicalizing every sample. The supported Mclone cylinder is 6,144
blocks around X. A 131,072-block planar footprint spans more than 21 canonical
laps, while a 524,288-block footprint spans more than 85. The current one-lift
product would reject such a view, and generating those samples as unrelated
terrain would duplicate one world many times.

A future periodic distant-terrain product must select an explicit policy:

- map the canonical fundamental domain once;
- cap a flat observer horizon to the admitted one-lift range;
- draw several presentation lifts from one canonical terrain/summary
  residency without duplicating worldgen or authority; or
- apply a separately accepted topology-specific visual embedding.

Filtering and LOD neighborhoods also wrap. Footprint taps crossing a seam use
canonical neighbors, while a footprint comparable to or larger than a period
summarizes the canonical domain once rather than counting repeated lifts as
additional terrain. At spacing 2,048 the present cylinder has only three
unique samples around its circumference; a planar clipmap tile at that spacing
is therefore not a truthful periodic terrain product merely because its point
samples repeat.

A flat torus may eventually maintain one finite periodic multiresolution
pyramid whose coarsest product summarizes the complete canonical world. That
can be cheaper than an unbounded plane map. In-world observation remains
harder because multiple lifts, occlusion, interaction eligibility, and optional
visual curvature still need one coherent presentation policy.

Periodic CPU preview primitives exist, but periodic game-facing distant
terrain and GPU viewport products remain unclaimed until this representation
contract is implemented and validated. They must fail or remain unavailable
rather than silently use the unbounded product.

## Presentation-Only Visual Curvature

Visual curvature is optional and cannot feed back into topology or simulation.
The same periodic world should be loadable with flat presentation or a fake
cylindrical horizon while producing the same authoritative command/update and
persistence trace.

For a cylinder, an observer-local render warp may bend the periodic coordinate
around a chosen visual radius:

```text
angle = local_x / visual_radius
render_x = visual_radius * sin(angle)
render_y = world_y + visual_radius * (cos(angle) - 1)
render_z = local_z
```

This is an illustrative presentation equation, not a required implementation.
A continuous vertex warp gives smooth terrain but slightly deforms cubes; a
rigid per-column or per-section transform preserves shapes but can create gaps
or overlaps. The selected compromise must be judged visually and may remain a
diagnostic effect.

Presentation bending must:

- occur after the observer-local lift is selected;
- remain anchored to the topology's periodic direction rather than camera yaw;
- apply consistently to terrain, actors, outlines, particles, fluids, and any
  other world-space feature it claims to support;
- preserve per-eye view data and the full-frame multiview path when enabled in
  XR;
- leave collision, reach, movement, server poses, storage, generation, and
  network data unchanged; and
- define interaction projection honestly if a strong bend makes an unwarped
  camera ray visibly disagree with the displayed target.

The first proof should use the smallest supported capture path that exercises
the changed presentation contract and be inspected at the first drawable
milestone. XR enablement is a separate explicit acceptance step because a
camera-relative world warp can be uncomfortable even when technically correct.

## Fog-Capped Cube Atlas

The cube experiment precedes the true sphere because it keeps exact square-grid
ownership almost everywhere:

- six finite square patches represent the faces;
- ordinary face crossings use exact quarter-turn transition maps;
- terrain and features share canonical edge ownership;
- a configurable canonical neighborhood around each of the eight cube vertices
  is excluded from movement, placement, spawning, and simulation interest; and
- fog, terrain, or a deliberately strange debug visualization communicates that
  the excluded region is off limits.

Across a non-vertex cube edge, two faces can be unfolded into one ordinary
Euclidean plane. At a cube vertex, three right-angle face sectors total 270
degrees rather than the ordinary 360 degrees. The missing 90 degrees is a
conical defect, so the engine does not pretend that the vertex is an ordinary
voxel neighborhood.

Excluding the vertex neighborhood removes the local invalid gameplay area but
does not erase global frame behavior. A loop around one cap can accumulate a
quarter-turn. Canonical blocks and frozen object frames remain unchanged; the
player/observer frame policy must explicitly choose whether to carry that
rotation, relock to a deterministic gauge, or absorb a correction near the
excluded region. The cube experiment is therefore a controlled exact-grid test
of the 90-degree return problem without approximate spherical rasterization.

The cube atlas is not permission to claim a true sphere implementation. It is a
bounded, piecewise-Euclidean cubical world with eight inaccessible exceptional
regions.

## True Spherical Work Is Last

The independent `../spherical-voxel-lab` experiment explores a different and
harder problem: canonical continuous spherical geography, frozen regional
voxel ownership, observer-local rasterization, grid phase, and owner relock
under spherical holonomy. Its successful concepts may later be reimplemented
deliberately in shared Rust, but it remains independent and disposable.

The production order intentionally defers that work until finite bounds,
periodic identity, observer lifts, rotated patch transitions, stable loop
return, excluded regions, and presentation separation are already proven. A
future spherical tactical must state which exact-grid contracts remain valid,
where approximation begins, and how stable canonical objects survive chart
reconciliation.

## Shared Ownership and Routing

This is a shared Rust engine concern. No platform app owns topology,
bounds, wrapping, generation seams, object identity, or presentation policy.

- a foundational shared contract below server and client owns validated
  topology values, canonical transition math, and frame transforms; it may
  begin in `mclone-core` and move to a focused crate only when its dependency
  shape justifies one;
- `mclone-protocol` owns cross-boundary topology descriptors and qualified
  canonical address facts needed by configuration and updates;
- `mclone-server::DimensionRuntime` owns the active topology, authoritative
  bounds, canonical spatial state, interest, adjacency, and validation;
- `mclone-worldgen` consumes topology-aware canonical requests while each
  generation profile owns its content and seam-smooth sampling technique;
- `mclone-client` retains canonical replica facts and topology-aware movement,
  collision, interaction, interpolation, and actor proximity;
- `mclone-mesh` consumes topology-resolved neighbor and halo facts rather than
  inventing a second wrapping policy;
- `mclone-scene` and `mclone-render-session` own observer anchoring, lift
  selection, frame reconcile, and neutral presentation configuration;
- `mclone-render` applies lift placement and optional visual bending through
  mono, per-eye, and multiview-aware paths; and
- desktop, web, Android, and XR apps provide only their existing input,
  lifecycle, target, and presentation adapters.

`DimensionDefinition` is the durable home for topology selection beside, not
inside, its generation profile. Realm and dimension scoping remain unchanged:
two dimensions may use different topology descriptors, and equal canonical
addresses in different dimensions remain isolated by `DimensionKey`.

## Hard Invariants

1. Every accessible gameplay neighborhood uses ordinary Euclidean block size,
   distance, collision, and local simulation rules.
2. A topology transition preserves the integer voxel lattice. Simulation does
   not accept arbitrary scale, shear, or nonlinear transforms.
3. Every authoritative spatial fact has one canonical identity per dimension,
   independent of how many observer lifts are rendered.
4. Finite boundaries and excluded regions are enforced by authority and cannot
   be bypassed by missing terrain, teleport, network input, or presentation.
5. Topology and generation are orthogonal persisted dimension facts; invalid
   combinations fail explicitly.
6. World generation, features, lighting, fluids, meshing, collision, entities,
   pathfinding, interest, and persistence agree on topology neighbors.
7. Presentation transforms never alter authoritative poses, distances, chunks,
   object identity, generation, persistence, or protocol semantics.
8. Vanilla Overworld remains unbounded Euclidean unless an explicit target
   change says otherwise.
9. Platform clients consume the same shared topology and canonical facts.
10. A fog-capped cube does not silently treat an exceptional vertex as an
    ordinary cell or claim true spherical voxelization.
11. True spherical integration remains a separate final experiment with an
    explicit approximation contract.
12. Concurrent dimensions never depend on a global or thread-local current
    topology; jobs and retained services carry their owning dimension facts.
13. A periodic seam is not a content-exclusion boundary. Supported families
    cross it through canonical identity and coherent work lifts.

## Proposed Implementation Ladder

[`Tactical 195`](../tactical/195-periodic-cylinder-topology-proof.md) completed
Stages 0-2 as independently reviewable slices: identity topology and
persistence, a finite-bound canary, then a real Flat Grass cylinder with
interactive seam diagnostics. Tactical
[`196`](../tactical/196-periodic-mclone-terrain-fields.md) completed the first
procedural periodic terrain caller on 2026-07-22. Subsequent Mclone
river/wetland, climate, vegetation, and bounded-stream additions preserve the
same exact plane/cylinder sampling modes. Later topology stages remain
unallocated.

### 0. Contract and Euclidean baseline

- Lock current unbounded Euclidean behavior with topology-operation property
  tests and representative server/client traces.
- Add the persisted descriptor and configuration path with an exact legacy
  default.
- Route a narrow set of shared coordinate operations through the identity
  topology without changing pixels, chunks, movement, or Overworld output.

### 1. Finite hard-bound proof

- **Landed 2026-07-18:** add chunk-aligned finite X/Z extents to an internal
  Flat Grass dimension.
- **Landed:** enforce bounds in generation admission, interest, spawn,
  teleport, movement,
  placement, and persistence.
- **Landed:** keep visual barriers/fog absent from and independent of the
  authoritative denial contract.

### 2. Periodic-cylinder authority

- **Landed 2026-07-18:** add one chunk-aligned periodic axis comfortably larger than
  twice the tracking radius.
- **Landed:** canonicalize storage, generation, tickets, chunks, blocks, player/entity
  poses, collision, interaction, interpolation, meshing, lighting, and fluids.
- **Landed:** mark the observer-local seam, complete a player lap, and retain
  edits and actor identity after save/reopen.
- **Landed:** prove two players and one stable entity can interact across the
  identified edge through local and remote transports.

### 3. Flat torus and finite scheduling

- Make both horizontal axes periodic through the same axis contract.
- Prove the canonical chunk set is finite and work is deduplicated near both
  seams and corners of the fundamental rectangle.
- Save, reopen, and revisit seam-crossing edits and structures.
- Intentionally test a small view that admits multiple lifts after the ordinary
  one-lift path is stable.

### 4. Optional cylindrical presentation

- Draw the same periodic-cylinder save through flat and bent presentation.
- Apply one coherent visual transform to all claimed world-space features.
- Compare authoritative traces and persistence to prove the effect is visual
  only.
- Inspect offscreen captures before any XR enablement.

### 5. General patch-transition proof

- Express an exact known world, preferably the flat torus, as multiple finite
  patches.
- Add quarter-turn frame transforms while preserving exact canonical identity
  and ordinary grid distances.
- Compare canonical block, path, interaction, and loop-return traces against
  the simpler axis-periodic representation where equivalent.

### 6. Fog-capped cube

- Add six face patches and validated edge transitions.
- Exclude canonical neighborhoods around all eight vertices.
- Generate and mesh seamless face edges, enforce cap denial, and inspect the
  intended debug/fog treatment.
- Exercise a loop around an excluded cap and record the selected frame-return
  policy without mutating canonical objects.

### 7. Separate spherical proposal

- Re-read the current spherical lab evidence.
- Write a new tactical that names the remaining voxel-ownership, phase,
  observer disagreement, and reconciliation risks.
- Do not broaden the exact grid-transition implementation implicitly to claim
  approximate spherical support.

## Validation Themes

Each implementation slice should select proportional tests from this ledger:

- descriptor validation, exact legacy decode, and migration/rejection behavior;
- canonicalization idempotence and inverse transition property tests;
- exact one-block neighbor distance across every supported seam;
- step-across/step-back identity including frame composition;
- finite-bound and excluded-region rejection for movement, teleport, spawn,
  placement, pathfinding, and interest;
- canonical chunk-view and ticket deduplication near seams;
- simultaneous jobs for dimensions with different topology descriptors and no
  implicit current-topology state;
- deterministic generation independent of request order and lift;
- periodic 3D density value and derivative equality across supported seams;
- canonical bounded-feature identity and local work geometry on both sides of
  a seam;
- periodic terrain and feature seam inspection;
- periodic filtering that counts the canonical domain once when a footprint
  crosses or covers a period;
- seam-crossing mesh face culling, AO, biome tint, lighting, and fluids;
- local and remote player/entity collision, interaction, interpolation, and
  visibility across a seam;
- stable structure IDs and block states after full loops and save/reopen;
- in-memory, TCP, and WebSocket normalized behavior agreement;
- SQLite and IndexedDB canonical-key agreement;
- flat versus bent-presentation authoritative trace equivalence;
- native offscreen screenshot inspection at the first drawable milestone;
- browser build/smoke and platform-appropriate Android/XR packaging;
- mono, per-eye, and full-frame multiview coverage for any enabled world-space
  presentation feature; and
- unchanged reference Overworld oracle and ordinary Euclidean client behavior.

Rendered debug and smoke captures stay under `/tmp`, following the project
validation policy.

## Open Questions

- What is the minimum authoritative boundary shape needed for finite worlds:
  cell rejection alone, explicit border collision, or both?
- Which generation profiles first support finite and periodic topology, and how
  are invalid combinations represented in the world catalog?
- Does the first cylindrical presentation bend individual vertices, rigid
  columns, or render sections, and how much interaction-ray disagreement is
  acceptable?
- Which deterministic frame gauge should the cube use, and what should a player
  experience after circling an excluded vertex cap?
- How large must the cube exclusion regions be relative to interaction,
  tracking, generation dependency, and fog radii?
- Which dimension-scoped context and lifted-query types are the smallest
  durable interface shared by generation, simulation, and presentation?
- Does the first periodic distant terrain cap at one lift, instance several
  lifts from one canonical residency, or introduce topology-specific
  presentation?
- Which wrapped 3D density scales and cell sizes supply enough formation
  vocabulary before circle-embedded noise becomes worth its cost?
- When multiple visible lifts are admitted, which render/interaction instance
  wins at exact half-period ties without duplicating authoritative identity?

## Related

- [`../tactical/195-periodic-cylinder-topology-proof.md`](../tactical/195-periodic-cylinder-topology-proof.md)
- [`../tactical/196-periodic-mclone-terrain-fields.md`](../tactical/196-periodic-mclone-terrain-fields.md)
- [`../tactical/257-topology-worldgen-conformance-probe.md`](../tactical/257-topology-worldgen-conformance-probe.md)
- [`faithful-world-embeddings.md`](faithful-world-embeddings.md)
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`embedded-worlds.md`](embedded-worlds.md)
- [`../native-engine-architecture.md`](../native-engine-architecture.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../protocol.md`](../protocol.md)
- [`../platforms.md`](../platforms.md)
- the independent `../spherical-voxel-lab` sibling repository when both
  projects are checked out side by side
