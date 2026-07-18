# Tactical 195: Periodic Cylinder Topology Proof

Status: planned after Tactical 192 closes its first mountain/valley family. It
does not block that bounded terrain work, but the topology contract and flat
cylinder proof must land before the next river, hydrology, climate, or major
structure-breadth tactical. No runtime topology code has landed yet.

Topic: `bounded-world-topology`

Workstream: shared native Rust dimension topology, server authority, worldgen
planning, client replica/presentation, and renderer seams; desktop/offscreen
validation first, followed by browser and platform closeout.

## Result

Land the first real non-Euclidean dimension topology: an intrinsically flat
cylinder whose X axis is periodic, whose Z axis is unbounded, and whose ordinary
Minecraft-like Y range remains finite.

The proof is deliberately generated with Flat Grass or a tightly bounded
authored derivative. It must nevertheless be a real playable dimension rather
than a coordinate-unit test:

- one persisted topology descriptor travels through every host and client;
- one finite canonical X chunk set is generated, scheduled, stored, and saved;
- observer-local lifts draw canonical chunks continuously across the seam;
- movement, collision, interaction, meshing, lighting, fluids, actors, remote
  players, and interpolation agree on the wrapped neighbor;
- a complete lap and save/reopen retain stable object identity;
- interactive diagnostics make the invisible canonical seam observable; and
- ordinary Euclidean dimensions and reference Overworld output remain exact.

This tactical stops before a flat torus, multiple simultaneously visible lifts,
nonlinear cylindrical visual bending, a faithfully embedded square-prism or
cuboid world, arbitrary patch graphs, and true spherical work.

## Sequencing With Terrain Generation

Tactical 192 may finish the first Mclone mountain/valley family because its raw
fields and production sampler remain isolated and `mclone-overworld-v1` is
internal-mutable. Tactical 195 should then establish the topology seam before
new rivers, hydrology, climate breadth, or structures multiply raw planar
coordinate assumptions.

The cylinder proof does **not** make current Mclone terrain periodic. It gives a
later terrain tactical the exact generation-space contract it must consume:

- canonical output and persistence identity;
- target-relative lifted work coordinates for rectangular feature algorithms;
- topology-aware dependency halos and deduplication;
- a worker descriptor that cannot leak caches across topology changes; and
- explicit profile/topology compatibility at dimension creation and load.

Current reference Overworld, Alpha, and Beta generators must not be silently
wrapped. Their topology remains unbounded Euclidean unless a later explicit
target changes them.

## Product Fixture

The first internal cylinder uses a chunk-aligned periodic X axis:

```text
HorizontalTopology {
    x: Periodic {
        minimum_chunk: 0,
        period_chunks: P,
    },
    z: Unbounded,
}
```

The provisional fixture uses `P = 32` chunks, a circumference of 512 blocks,
and a normal render/tracking radius no greater than 6 chunks. Final constants
may change during Slice 0, but the one-lift invariant is fixed:

```text
2 * accepted_tracking_radius + 1 <= period_chunks
```

A session must cap or reject a requested view that violates this bound. The
first implementation does not draw two images of one canonical chunk in one
view.

The canonical interval is `0 <= chunk_x < P`, with block X in
`0 <= block_x < 16P`. Negative and positive external coordinates use Euclidean
remainder. Z and Y preserve their ordinary meanings.

The fixture provides both ordinary play and deliberate seam evidence:

- coordinate-independent Flat Grass supplies the base terrain;
- a stable colored road, stripe, gateway, or equivalent authored marker spans
  canonical chunks `P - 1` and `0`;
- one emissive/opaque arrangement exercises mesh culling, AO, and light reads;
- one controlled fluid arrangement attempts to cross the seam;
- editable blocks on both sides prove targeting, mutation, and persistence;
- one scripted passive entity repeatedly crosses the seam; and
- an optional second scripted or connected player approaches and crosses from
  the opposite side.

The authored evidence is a fixture layered through ordinary authoritative
block/entity mechanisms. Presentation diagnostics must not mutate arbitrary
user saves.

## Canonical and Lift Contract

`BlockPos`, `ChunkPos`, and `Vec3d` remain efficient context-free leaf values.
They do not gain implicit modulo behavior. A validated dimension-owned topology
service performs every topology-dependent operation.

Conceptually:

```text
canonical_chunk_x(x) =
    minimum + (x - minimum).rem_euclid(period_chunks)

canonical_block_x(x) =
    minimum_block +
    (x - minimum_block).rem_euclid(period_chunks * 16)
```

The service must provide, at minimum:

- descriptor validation and exact legacy Euclidean defaulting;
- block, chunk, and continuous-pose canonicalization;
- canonical neighbor stepping across finite, periodic, and identity axes;
- shortest periodic displacement with an explicit half-period tie rule;
- canonical view enumeration with observer-relative lift offsets;
- topology-aware chunk/block distance and scheduler priority;
- dependency-halo canonicalization and stable deduplication;
- nearest-lift selection for chunks, blocks, poses, actors, and hits; and
- finite-bound rejection for the contract canary preceding the cylinder.

Near canonical X chunk zero, a radius-two observer view contains:

| Canonical chunk X | Observer lift X |
|---:|---:|
| `P - 2` | `-2` |
| `P - 1` | `-1` |
| `0` | `0` |
| `1` | `1` |
| `2` | `2` |

The authority generates, stores, ticks, and publishes each canonical chunk
once. The client replica retains one canonical fact. Scene/render submission
adds the selected lift; it does not clone authority or persistence.

Player and actor interpolation use the shortest periodic displacement. A
canonical correction from nearly `16P` to nearly zero is a small seam crossing,
not a high-speed teleport. Join, explicit teleport, respawn, and dimension
transfer reset continuity rather than guessing a winding history.

## Persisted and Protocol Contract

Topology is immutable dimension metadata beside seed, generation profile, and
vertical/environment facts. Exact existing records decode as unbounded
Euclidean. Native SQLite and browser IndexedDB persist the same descriptor and
reject unsupported or malformed values consistently.

The server sends the active dimension topology during configuration or
dimension replacement before any chunk, pose, actor, or interaction fact that
requires it. Local in-memory, TCP, and WebSocket paths carry the same semantic
ordering. TypeScript may transport the encoded descriptor but cannot interpret
or implement wrapping.

The client resets dimension-local topology, lift anchors, chunk draw instances,
actor interpolation, and render caches together on a true dimension change.
Asset replacement and ordinary renderer rebuild preserve the active topology
and reconstruct derived lift state without changing canonical facts.

The immutable generation-worker descriptor must eventually distinguish
topology facts that affect planning or output. A resident cache may not survive
a seed, profile, or relevant topology change merely because canonical chunk
coordinates overlap.

## Generation Compatibility Contract

Topology and generation remain separate dimension facts. Each supported pair
must be explicit:

| Profile/family | Euclidean plane | Finite | Cylinder X | Torus | Patch atlas |
|---|---|---|---|---|---|
| Flat Grass | supported | first proof | first proof | pending | unsupported |
| Authored Only | supported | fixture-only proof | fixture-only proof | pending | design only |
| Mclone Overworld | supported | pending | later periodic sampler | pending | design only |
| Small Island | supported bounded content | pending | unsupported initially | unsupported | unsupported |
| Reference Overworld | supported/reference-locked | unsupported | unsupported | unsupported | unsupported |
| Alpha/Beta | supported internal profiles | unsupported | unsupported | unsupported | unsupported |

Unsupported combinations fail at dimension creation or load. They do not
generate independent planar edges and apply modulo afterward.

Flat Grass is target-only and coordinate-independent, so it proves topology
without hiding a generator seam. A small synthetic planning/feature fixture
must still prove that a write centered in canonical chunk `P - 1` can target
canonical chunk `0`, and that the scheduler requests and persists the target
once.

For later procedural terrain, canonical identities alone are insufficient.
The generator needs a coherent local lift around the target so rectangular
algorithms do not see `P - 1` and `0` as distant. A future planning value may
therefore distinguish:

```text
GenerationChunkRef {
    canonical: ChunkPos,
    work_lift: ChunkPos,
}
```

Names remain an implementation choice, but the two meanings may not be
collapsed.

The current `ValueNoise2d` hashes absolute lattice coordinates and is not
periodic. A later periodic-lattice implementation may wrap lattice X when the
block period is exactly divisible by the field scale. The six current Mclone
field scales have a least common multiple of 6,144 blocks, or 384 chunks; that
is useful evidence for one possible sampler, not a topology minimum or a frozen
terrain decision. Circle-embedded or revised-scale samplers remain alternatives
and require their own field-map and seam review.

## Interactive Topology Observability

Wrapping is intentionally invisible during ordinary flat presentation, so the
proof needs an opt-in shared diagnostic surface rather than relying on a player
to infer it.

The diagnostic mode should expose:

- topology kind, canonical period, and accepted one-lift view limit;
- canonical player block/chunk X and continuity/lift X;
- distance and direction to the nearest canonical seam image;
- current canonical chunk count, visible draw-instance count, and duplicate
  canonical-identity violations;
- selected lift and shortest displacement for targeted actors/players;
- entity id, canonical pose, chosen lift, interpolation delta, and seam-crossing
  count for the scripted actor; and
- last interaction's lifted hit and mapped canonical block/entity identity.

Use two complementary presentations:

1. A shared UI/HUD readout provides exact values without adding world geometry.
2. An optional world-space seam treatment marks the observer-local seam plane,
   adjacent canonical chunk borders, direction arrows, and the marker fixture.

The world-space treatment should reuse an existing multiview-aware overlay or
terrain/debug path. If the first implementation is intentionally flat/offscreen
only, XR enablement must be explicitly disabled and recorded until mono,
per-eye, and full-frame multiview paths are present. It may not accidentally
appear in only one XR eye or mutate authoritative blocks outside the dedicated
fixture.

The scripted actor should loop slowly through `P - 1 -> 0 -> 1` and back while
retaining one stable entity identity. A remote-player variant should permit an
interactive observer to stand at canonical zero, watch the other player
approach from `P - 1`, cross, reverse, and repeat without disappearance,
duplication, a long interpolation sweep, or a camera correction.

Offscreen receipts should include:

- a normal flat view with diagnostics disabled;
- a seam view with the marker and diagnostics enabled;
- the frame immediately before an actor crossing;
- the first frame after crossing; and
- a post-reopen view proving the same edited seam blocks.

All captures remain under `/tmp`.

## Fixed Boundaries

- Keep ordinary Euclidean Overworld chunks, pixels, movement, persistence,
  generation, and protocol traces unchanged.
- Do not add modulo operations directly to `BlockPos`, `ChunkPos`, generic
  noise samplers, `FeatureRegion`, or renderer section identity.
- Keep server authority and canonical storage independent from observer lifts.
- Keep the first period chunk-aligned and larger than every admitted ordinary
  view; multiple visible lifts are out of scope.
- Do not make Mclone Overworld terrain periodic in this tactical.
- Do not add rivers, hydrology, climate breadth, structures, or a generic
  arbitrary-topology worldgen framework.
- Do not implement cylindrical horizon bending or a faithful curved embedding.
- Do not add winit, browser, Android, or OpenXR dependencies to shared topology
  owners.
- Keep platform apps limited to existing lifecycle, transport, input, target,
  and presentation adapters.
- Preserve the direct one-world/one-lift render fast path and measure any added
  per-frame or per-section overhead.

## Current Retrofit Surface

The implementation should begin with a complete call-site inventory rather
than a narrow scheduler modulo. Current high-value seams include:

- `mclone-server::DimensionDefinition`, which has no topology fact;
- context-free `mclone-core::{BlockPos, ChunkPos, Vec3d}`;
- `mclone-protocol::ChunkView` and session/dimension configuration;
- raw square enumeration in `mclone-server::player_chunk_tracking`;
- Euclidean scheduler priority, loading progress, distance/ticket, spawn, and
  dependency calculations;
- `ChunkGenerationPlan`, worker descriptor/cache keys, and
  `FeatureRegion`/feature-world coordinate expansion;
- scheduler block/liquid/light neighbor reads and scheduled ticks;
- client movement, collision, interaction raycast, teleport/path helpers, and
  actor interpolation;
- mesh neighbor/AO/light sampling and render-section invalidation;
- scene chunk/actor lift selection and render draw identity;
- SQLite/IndexedDB chunk/entity keys and dimension metadata codecs; and
- Far LOD, sky/fog, particles, overlays, and diagnostics.

Every inventoried caller must be classified as routed in this tactical,
explicitly unavailable in the cylinder profile, or deferred behind a safe
failure. A flat-looking screenshot alone is not completion evidence.

## Execution Checklist

### Slice 0: contract audit and clean baseline

- [ ] Complete the raw-coordinate and neighbor-operation inventory above.
- [ ] Lock exact Euclidean descriptor decode, server/client traces, scheduler
  work, movement, interaction, persistence, and representative pixels.
- [ ] Finalize the internal fixture period, view radius, marker, actor script,
  fluid/light canaries, and debug receipt schema.
- [ ] Record the Mclone topology-support matrix and the pre-river sequencing
  boundary in the living topic docs.
- [ ] Decide which existing renderer/overlay path owns the world-space debug
  treatment through mono, stereo, and multiview.

Gate: every consumer is classified, the ordinary path is locked, and the
fixture can be evaluated without topology code.

### Slice 1: descriptor and identity topology

- [ ] Add the smallest validated finite/unbounded/periodic horizontal-axis
  descriptor in shared Rust.
- [ ] Persist it beside generation and environment facts with exact legacy
  unbounded-Euclidean migration.
- [ ] Carry it through configuration, dimension transfer, native/browser
  storage, and in-memory/TCP/WebSocket paths before dependent play facts.
- [ ] Implement topology operations and property tests under identity topology
  without changing any ordinary call result.
- [ ] Route one narrow set of existing operations through identity topology and
  prove output/pixel/performance invariance.

Gate: topology is a real dimension fact everywhere while all existing worlds
remain observably unchanged.

### Slice 2: finite-bound canary

- [ ] Add a small chunk-aligned finite Flat Grass or authored dimension.
- [ ] Enforce bound rejection in view enumeration, generation admission, spawn,
  teleport, movement, placement, tickets, and persistence.
- [ ] Distinguish outside-topology from valid-but-unloaded cells in diagnostics.
- [ ] Keep the visual wall/fog/marker separate from authoritative denial.

Gate: the topology service proves rejection and migration semantics before its
first identity-forming periodic seam.

### Slice 3: periodic authority and scheduling

- [ ] Enable periodic X for the internal Flat Grass cylinder.
- [ ] Canonicalize views, tickets, holders, priority distance, generation
  outputs, worker requests, block/tick/entity ownership, and persistence keys.
- [ ] Cap or reject views that violate the one-lift period constraint.
- [ ] Canonicalize and deduplicate the synthetic cross-seam dependency/feature
  write while retaining a coherent work lift.
- [ ] Prove one finite canonical X chunk set under repeated laps and opposing
  observers.

Gate: authority, scheduling, generation, and storage have no duplicate seam
identity even before the client draws a wrapped neighbor.

### Slice 4: client replica, observer lifts, and first pixels

- [ ] Install topology before client chunks and retain canonical replica facts.
- [ ] Select one nearest lift per visible chunk and separate canonical render
  section identity from draw placement.
- [ ] Rebase/draw canonical `P - 1` beside canonical `0` without duplicating the
  chunk, mesh, or upload authority.
- [ ] Make seam neighbor snapshots drive face culling, AO, biome tint, packed
  light, dirty invalidation, and render traversal.
- [ ] Add the HUD and initial world-space seam diagnostic through a compatible
  mono/per-eye/multiview-aware path.
- [ ] Capture and inspect the first seam view before adding actor crossings.

Gate: the cylinder is visibly seamless, canonical/draw counts reconcile, and
the ordinary direct render path remains unchanged.

### Slice 5: movement, interaction, fluids, and actors

- [ ] Canonicalize local-player movement while preserving observer continuity
  and shortest-displacement validation.
- [ ] Cross the seam through collision, raycast, break/place, outlines, and
  authoritative correction without a teleport artifact.
- [ ] Run the light/emissive and controlled fluid canaries across the seam.
- [ ] Cross one stable scripted entity in both directions and retain correct
  tracking, simulation, drawing, and interpolation.
- [ ] Place two players on opposite canonical sides, interact across the seam,
  and cross while each client selects its own nearby lifts.
- [ ] Reset continuity correctly on join, teleport, respawn, death, and
  dimension transfer.
- [ ] Finish the interactive diagnostic receipts and assert no duplicate
  canonical ids or ineligible interactive lifts.

Gate: an interactive user can watch and test the seam directly, and every
claimed gameplay consumer agrees on the same neighbor.

### Slice 6: persistence, platform closeout, and terrain handoff

- [ ] Complete a lap, edit both sides of the seam, save, close, reopen, and
  revisit the same canonical facts through SQLite and IndexedDB.
- [ ] Prove normalized in-memory, TCP, and WebSocket behavior agreement.
- [ ] Re-run native workspace tests, offscreen captures, browser build/smoke,
  Android packaging/AVD canary, and available XR compile/device lanes.
- [ ] Measure direct Euclidean and one-lift cylinder frame, scheduling, mesh,
  upload, storage, and memory costs.
- [ ] Update the topology-support matrix and every deferred/unsupported
  subsystem with an exact reason.
- [ ] Write the next periodic Mclone terrain tactical only after the canonical
  plus work-lift generation contract is demonstrated.

Gate: Cylinder v0 is a persisted, networked, inspectable gameplay dimension;
the next terrain caller does not need to rediscover topology ownership.

## Required Validation

- descriptor validation, codec migration, and invalid profile/topology rejection;
- Euclidean identity-operation equivalence and pixel locks;
- negative/positive Euclidean-remainder property tests;
- canonicalization idempotence and neighbor step-across/step-back identity;
- shortest block/chunk/continuous displacement including half-period ties;
- finite-bound movement, teleport, spawn, placement, ticket, and persistence
  rejection;
- canonical view/ticket/work deduplication at both sides of the seam;
- scheduler priority treating `P - 1` as distance one from zero;
- synthetic feature write and dependency halo across `P - 1 -> 0`;
- mesh face culling, AO, tint, light, fluid, and block-update seam evidence;
- local movement, raycast, break/place, outline, and correction across a lap;
- stable entity and remote-player identity/interpolation across both directions;
- SQLite/IndexedDB edit and actor persistence after reopen;
- in-memory/TCP/WebSocket normalized trace agreement;
- direct path and cylinder performance/accounting comparisons;
- inspected `/tmp` normal, debug seam, actor-before/after, and reopen captures;
- browser production smoke and platform-appropriate Android/XR gates; and
- mono, per-eye, and full-frame multiview coverage for every enabled
  world-space diagnostic or cylinder-visible feature.

## Stop Conditions

Stop and split follow-up work if any slice requires:

- multiple visible lifts of one canonical object;
- nonlinear visual curvature or a smooth cylindrical embedding;
- arbitrary-angle gravity or rotated patch transitions;
- changing reference Overworld generation or persisted identity;
- a general user-authored topology graph;
- a new river, hydrology, climate, structure, or cave system; or
- app-local topology or generator policy.

Do not call the cylinder complete if only worldgen values repeat while tickets,
lighting, fluids, actors, persistence, or rendering still see a hard edge.

## Related

- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
- [`../topics/faithful-world-embeddings.md`](../topics/faithful-world-embeddings.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`191-guarded-generation-planning-refactor.md`](191-guarded-generation-planning-refactor.md)
- [`192-mclone-overworld-mountains-and-valleys.md`](192-mclone-overworld-mountains-and-valleys.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../protocol.md`](../protocol.md)
- [`../platforms.md`](../platforms.md)
