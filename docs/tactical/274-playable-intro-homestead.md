# Tactical 274: Playable Intro Homestead

Status: **active 2026-08-10; implementation has not started. This tactical
replaces open-ended terrain/LOD research as the selected product-integration
slice. Human Review R1 is the first stop after deterministic site scouting and
before farmstead materialization.**

Topic:

- `starter-farmstead-settlement`

## Instruction Synthesis

Regroup around the shortest path to something recognizably playable. Preserve
the completed original-terrain, procedural-horizon, shared-host, persistence,
and Quest/Web work as foundations, but do not continue speculative terrain
representation or worlds-within-worlds work until a player-facing opening is
coherent.

The intro homestead should ultimately work with an arbitrary random or
user-supplied seed. It must deterministically select a gently buildable site
near the base world's provisional spawn, adapt a small authored composition to
that site, and make the homestead arrival the player's actual first spawn. A
flat pad is not the target. The selected site should retain modest relief and
use foundations, short terraces, paths, and bounded grading to belong to the
terrain.

The first review and shareable alpha may use one showcase seed selected by the
same scout. The implementation may not contain seed-specific structure or
terrain behavior, and arbitrary-seed evidence remains part of this tactical.

## Objective

Create the first complete `Homestead Start` world-opening path:

```text
base profile + random or entered seed
  -> ordinary provisional safe spawn
  -> bounded deterministic site survey
  -> scored homestead plan and arrival
  -> persisted starter-content/instance identity
  -> status-aware cross-chunk structure materialization
  -> authored grading, paths, water, planting, and residents
  -> safe first spawn facing the homestead
  -> ordinary editable persistent play
  -> save, close, reopen, and respawn without duplication
```

The acceptance target is a small, attractive, first-party opening rather than
the maximal settlement vision. A player should be able to create the world,
arrive without developer intervention, explore the farmyard, break and place
blocks, encounter cows and chickens, save, quit, and return to the same edited
world.

## Product Contract

### World-start choices

Add a shared world-start choice with two semantic values:

- `Homestead Start` selects the versioned intro overlay and site planner;
- `Wild Start` retains the existing ordinary safe-spawn behavior with no
  starter settlement.

The exact Rust type name may differ, but the selection is persisted world
identity rather than a launch-only UI preference. Desktop, web, Android, XR,
dedicated startup, and future import/export paths must consume the same fact.
Apps may collect or display the choice; they may not interpret its generation
rules.

For the first playable-alpha presentation, a named showcase action may supply
the reviewed seed and `Homestead Start`. Ordinary New World creation remains
seeded randomly when the player leaves the seed blank, and an entered seed
must use the same selector. The showcase is a preset, not a special generator.

### Identity and compatibility

Persist these identities independently:

```text
base WorldGenerationDescriptor
starter-content descriptor: intro-homestead-v1
realized homestead plan identity and revision
```

Pure `overworld` remains the Minecraft Java 1.17.1 reference result.
`overworld + intro-homestead-v1` is an explicit overlay world and must not
change oracle fingerprints for pure `overworld`. `mclone-overworld-v1` remains
internal-mutable under the compatibility safety ledger, but changing its base
terrain does not silently discard an already persisted realized plan.

The same base descriptor, starter descriptor, and implementation revision must
produce the same selected candidate and plan independent of request order,
thread count, Worker completion order, cache warmth, or viewport. Persisting
the chosen plan makes reopen authoritative even if later development changes
the mutable selector before a release freeze.

### Near-spawn selection

“Near spawn” means near the base profile's provisional safe spawn before the
intro overlay changes player arrival. The player is not required to walk from
that provisional point; the accepted homestead arrival becomes the actual
first-spawn location.

Revision 1 uses these bounded planning defaults:

| Fact | Revision 1 default |
|---|---:|
| Primary search radius | 384 blocks from provisional spawn |
| Candidate lattice | 32-block spacing, topology-canonicalized |
| Candidate rotations | four cardinal rotations |
| First-composition core | up to 96 by 96 blocks |
| Reserved expansion envelope | up to 160 by 160 blocks |
| Wider scenic survey ring | up to 256 blocks from candidate anchor |
| Wider fallback | out to 768 blocks on a 64-block lattice |
| Maximum candidate/rotation evaluations | 4,096 across both passes |

These are planner-version facts, not user-facing settings. They may be tuned
during Slices 2-3 before the first plan fingerprint is accepted. Any later
change requires an explicit tactical update and new evidence rather than a
silent constant edit.

Do not require a mathematically flat site. Candidate evaluation must preserve
and report:

- buildable area and surface-elevation span;
- slope distribution and estimated cut/fill volume;
- maximum local cut or fill depth;
- dry support and headroom at building and arrival footprints;
- existing fluid, protected-content, and structure intersections;
- ordinary-tree clearing inside the reservation;
- viable path circulation between the arrival and first buildings;
- room for named later parcels without placing them now; and
- wider-ring meadow, woodland, water, coast, and scenic-relief facts.

Hard rejection covers unsafe arrival, insufficient support, self-overlap in a
periodic topology, out-of-bounds placement, protected-content conflicts,
building-core water intersection, and earthwork beyond the accepted grading
budget. Soft scoring prefers restrained grading, balanced cut/fill, readable
arrival, mixed meadow/woodland context, nearby optional water, and relief in
the scenic ring rather than under the buildings.

Scores must be quantized before comparison. Equal candidates use a stable
seed/domain-derived tie key followed by canonical anchor and rotation. Scan or
completion order may never decide the winner.

### Fit, fallback, and rejection

Revision 1 has two composition tiers:

1. `intro-homestead-full-v1`, the preferred first composition; and
2. `intro-homestead-compact-v1`, a smaller honest fallback that retains the
   cottage, arrival, coop/farmyard identity, and named future sockets.

The primary radius tries the full tier, then the compact tier. The bounded
wider fallback uses the coarser lattice in the table only after the primary
receipt is retained. If neither tier has a safe site, world creation must
report a typed planning failure before committing an apparently successful
Homestead Start.
It must not silently create a Wild Start, flatten an arbitrary mountain, or
omit the settlement.

The arbitrary-seed corpus must measure success and fallback rates before the
showcase seed is selected. This tactical does not claim that every possible
seed supports the full tier.

### Arrival and respawn

The realized plan owns one arrival marker outside the private building
footprints and on the authored path. Its final pose must have:

- solid support and standing-player clearance;
- no body intersection with fluid or placed geometry;
- a dry route into the composition;
- a cardinal facing selected by the site plan; and
- an unobstructed first view toward the cottage/farmyard focal area.

First entry uses this pose. The realm's ordinary primary respawn path may use
the same safe shared-spawn area after death. Reopen restores an existing
player's persisted pose and must not teleport that player back to the arrival.
Changing a development preset also must not move existing players.

### First composition

The full Revision 1 composition contains only:

- a short arrival path and small common/farmyard;
- one accepted cottage variant;
- one compact barn or shed role;
- the Rosehip coop when its promoted runtime contract is ready, otherwise a
  deliberately bounded coop substitute authored through the same source path;
- one visual garden plot using currently honest materials;
- one small authored pond or spring-fed runnel independent of natural river
  availability;
- one focal authored oak and restrained planned planting;
- a small cow group and chicken group using one-time persistent markers; and
- named empty sockets for later enclosure, church, mill, stable, and
  outbuilding work.

No first slice may claim a functional animal enclosure until fence/gate
collision and navigation behavior can actually contain animals. Residents may
occupy an open farmyard in Revision 1. Crops and water are visual/world facts;
growth, harvest, irrigation, and machinery are gameplay follow-ups.

The compact tier may omit the barn and reduce the water/garden footprint. It
may not become only a cottage dropped at spawn with no arrival composition.

## Shared Ownership

- `mclone-server` owns starter-content world identity, structure statuses,
  starts/references/pieces, scheduling, materialization, authoritative
  residents, player arrival, and persistence transactions.
- `mclone-worldgen` owns profile-neutral site-survey facts, candidate scoring,
  deterministic plan construction, grading/reservation plans, and compact/full
  selection.
- `mclone-assets` and `mclone-app-runtime` own first-party content preparation
  and provenance; structure planning may refer only to stable semantic content
  ids.
- `mclone-scene` owns the shared world-create/start flow and player-facing
  readiness/failure state.
- `mclone-ui` owns the shared `Homestead Start` / `Wild Start` selection and
  typed planning-progress/failure presentation.
- native threads and browser Workers execute the same Rust planner and
  structure contracts. TypeScript, Android, winit, and OpenXR adapters transport
  inputs and results without interpreting site or structure policy.
- render crates consume ordinary chunks, actors, and UI. They do not receive a
  special farmstead draw path.

No app crate, browser script, render pass, or LOD producer may choose the site,
move the arrival, place the buildings, or respawn residents.

## Structure Lifecycle Requirement

The first production farmstead cannot use the standalone Structure Lab gallery
writer as a world-generation shortcut. Before the complete composition is
placed, this tactical must land the smallest true shared structure lifecycle:

- structure-start and structure-reference generation stages or equivalent
  status-aware scheduler ownership;
- persisted `StructureStart`, piece, bounding-box, and touched-chunk facts;
- exact per-target-chunk clipping;
- placement before ordinary decoration where the reservation requires it;
- starts owned once while every touched chunk records its reference;
- save/reopen and partial-load behavior; and
- no mutation outside the target chunk currently being materialized.

The lifecycle is first proved with a tiny original cross-chunk wayside marker
or equivalent neutral fixture. Only after its exact block diff,
starts/references, clipping, and persistence pass may the farmstead become the
larger caller. Do not implement vanilla villages, jigsaw recursion, Beardifier,
loot containers, or broad structure registries in this tactical.

## Plan and Persistence Contract

The persisted realized plan records at least:

- base descriptor and starter-content identity;
- planner and composition revisions;
- provisional spawn used as the search origin;
- selected tier, canonical anchor, rotation, and arrival pose;
- candidate score components and deterministic tie key;
- conservative plan and reservation bounds;
- exact piece ids, transforms, bounds, and touched chunks;
- grading, path, water, planting, and protected-region summaries;
- resident marker persistent ids and spawn facts;
- plan checksum and source content fingerprints; and
- explicit fallback or failure facts.

Persist the plan before any plan-owned chunk is published as complete. A
reopened world consumes the stored plan instead of rescoring. Existing full
chunks and player edits win over regeneration. Development-only rebuild tools
must be explicit and may not masquerade as ordinary reopen.

Resident markers are consumed idempotently into authoritative persistent
entities. Cache reset, chunk unload/reload, save/reopen, and partial
materialization may not duplicate cows or chickens.

## First-Party Presentation Bar

The opening view is a first-party acceptance surface. Before final visual
review:

- the documented forced-Original build/capture path must work;
- no Minecraft-reference or unknown asset may resolve;
- no numbered diagnostic texture may appear in the arrival, cottage, barn,
  coop, path, pond, oak, cow, chicken, HUD, or menu acceptance views;
- provisional fallback materials may remain outside the reviewed opening, but
  every visible fallback must be listed rather than hidden;
- debug counters and developer overlays are off in player-facing captures; and
- the ordinary scene path, lighting, vegetation, actor, and UI renderers are
  used without a farmstead-specific renderer.

This tactical does not complete the global first-party asset set. It completes
the bounded content actually visible in the accepted opening.

## Performance Budgets

Site selection must remain bounded and off the render thread.

Revision 1 targets, measured in release builds on the established hosts, are:

- at most 4,096 candidate/rotation evaluations across both search passes;
- no full chunk generation for rejected first-pass candidates;
- no more than 250 ms for the primary native scout corpus case;
- no more than 750 ms for the equivalent browser Worker case;
- no more than 64 KiB for the persisted realized-plan record before optional
  diagnostic evidence;
- exact touched-chunk scheduling derived from plan bounds rather than a global
  structure-radius increase; and
- no persistent per-frame work after materialization beyond ordinary world,
  entity, and scene costs.

If production survey facts cannot meet those targets, stop and reduce the
candidate/sample strategy before introducing an app-local cache or lowering
determinism. Record cold and warm times, candidates by rejection stage, survey
sample count, full/compact outcome, grade volume, piece count, touched chunks,
and materialization time.

## Implementation Slices

### Slice 0 — baseline and contract locks

- Capture the present Wild Start creation, Mclone Overworld fingerprints,
  original-assets provenance, save/reopen, and player-arrival behavior.
- Repair only blockers in the existing forced-Original capture path that
  prevent honest later visual validation.
- Add typed starter-content and realized-plan identities without changing any
  current world's decoded behavior.
- Pin unchanged pure `overworld` and `mclone-overworld-v1` base fingerprints.

Stop if starter identity cannot remain orthogonal to the base generation
profile and topology.

### Slice 1 — true structure lifecycle canary

- Add the minimal status/metadata/persistence path described above.
- Place one tiny original cross-chunk canary by exact per-chunk clipping.
- Prove start ownership, references, touched chunks, request-order equality,
  save/reopen, no far writes, and browser Worker parity.
- Keep the canary out of ordinary worlds unless its explicit test overlay is
  selected.

Stop before farmstead placement if any target chunk job can mutate another
already-final chunk.

### Slice 2 — neutral survey and deterministic selector

- Expose production-backed neutral survey facts through the shared worldgen
  owner.
- Compile the bounded candidate lattice, rotations, hard rejections, quantized
  score tuple, stable tie breaking, full/compact selection, and typed failure.
- Prove raster, reverse, shuffled, partitioned, native-thread, and browser
  Worker equality.
- Run a fixed arbitrary-seed corpus through Mclone Overworld plus a Flat Grass
  canary and retain success, fallback, rejection, cost, and grading metrics.

This slice ends at Human Review R1 with maps and candidate receipts. No
farmstead blocks are placed before the site and arrival direction are accepted.

### Slice 3 — showcase seed and realized plan

- Select a showcase seed from the accepted scout output rather than hand
  editing a preferred seed into the planner.
- Review aerial terrain, site bounds, grading map, arrival sightline, scenic
  ring, full/compact sockets, water fallback, and expected tree clearing.
- Persist and reopen the realized plan before materialization.
- Prove that cache reset and rescoring reconstruct the same plan checksum while
  ordinary reopen reads the persisted plan.

Human Review R1 acceptance selects the showcase seed and site; it does not
freeze arbitrary-seed support or create a release compatibility promise.

### Slice 4 — grading, reservation, and first drawable milestone

- Apply bounded foundations/terraces, path grading, authored water, and
  decoration reservation through ordinary chunk materialization.
- Suppress generic trees and large decoration inside the plan reservation
  without changing decoration outside it.
- Render and inspect the first graded pad and arrival path before adding a
  building.
- Prove maximum cut/fill depth, total earthwork, water closure, exact touched
  chunks, request-order equality, and save/reopen.

Stop at Human Review R2 if the site reads as a flattened platform, a pasted
stamp, or a drainage hazard.

### Slice 5 — first composition materialization

- Load the accepted source-first cottage/barn/coop records through their
  canonical Rust boundary.
- Construct fixed full and compact composition plans with semantic roles,
  transforms, paths, pond/runnel, focal oak, garden, planting, and future
  sockets.
- Materialize each piece through the true structure lifecycle and ordinary
  chunks.
- Inspect the cottage immediately, then the circulation/water layer, then the
  complete composition; do not defer visual review until all pieces land.

Stop at Human Review R3 if the arrival is illegible, building scale is wrong,
the farmstead lacks a coherent center, or visible content relies on diagnostic
assets.

### Slice 6 — arrival, residents, and durability

- Publish the accepted arrival as first-spawn/shared-spawn intent without
  moving returning players.
- Realize cows and chickens once from persistent markers.
- Prove first entry, death/respawn, close/reopen, chunk unload/reload, cache
  reset, partial materialization, and player block edits.
- Verify no duplicate residents, no overwritten edits, no unsafe spawn, and no
  repeated first-arrival teleport.

### Slice 7 — product flow and cross-host closeout

- Add the shared world-start selection and typed planning progress/failure to
  the existing create-world flow.
- Make the showcase preset enter the same shared path with only preset inputs.
- Run a clean 20-minute desktop session: title, create, arrive, explore,
  break/place, encounter residents, save, quit, reopen, and respawn.
- Run the equivalent headed browser creation/reopen capture and persistence
  proof.
- Build and smoke affected flat Android, synthetic stereo, desktop OpenXR, and
  Android XR boundaries. Physical Quest acceptance is required before calling
  the slice Quest-ready, but unavailable hardware does not justify a separate
  XR implementation.
- Record final first-party provenance, planning/materialization costs, pixels,
  and remaining player-facing gaps.

Human Review R4 decides whether the result is the first shareable playable
alpha and what the next product slice should be.

## Required Evidence

### Contract and determinism

- stable starter and realized-plan codecs;
- unchanged legacy world/profile decoding;
- unchanged pure reference-Overworld oracle/fingerprint corpus;
- unchanged base Mclone fingerprints outside the explicit overlay path;
- site selection equality across order, partition, threads, and Wasm;
- exact plan checksum, pieces, touched chunks, and resident identities; and
- clean full/compact/failure receipts.

### Durability

- plan-before-publication transaction ordering;
- structure starts/references and clipped pieces after SQLite reopen;
- browser IndexedDB create/reopen equivalence;
- player edits preserved across reopen;
- no resident duplication;
- returning player pose retained; and
- dead-player respawn routed to safe accepted shared spawn.

### Visual review

Write captures outside the repository. At minimum inspect:

- site/scoring map and grading map;
- unbuilt graded pad and arrival path;
- first cottage at ground and elevated views;
- complete arrival, farmyard, building, pond, and planting composition;
- day and low-light presentation;
- desktop and browser player HUD views with developer overlays disabled; and
- synthetic stereo, then physical Quest when available.

Rendered review supplements deterministic structure and persistence evidence;
it does not replace it.

### Platform boundaries

- shared workspace tests and focused server/worldgen/scene/UI suites;
- `wasm32-unknown-unknown` build and browser Worker tests;
- headed browser WebGPU create/reopen evidence after `pnpm host:check` on Linux;
- native offscreen and interactive desktop evidence;
- flat Android build plus AVD create/reopen smoke;
- synthetic stereo/OpenXR builds; and
- Android XR APK plus physical validation before a Quest-ready claim.

Use the current commands in [`../platforms.md`](../platforms.md) rather than
copying a command matrix into this tactical.

## Non-Goals

This tactical does not:

- implement the maximal farmstead, church, mill, stable, or broad settlement
  grammar;
- implement recursive jigsaw pools, villages, vanilla structures, Beardifier,
  or a general structure plugin registry;
- add crafting, hunger, combat, hostile mobs, crop growth, functional farming,
  or machinery;
- require natural rivers, drainage networks, waterfalls, or a mill-compatible
  reach;
- add worlds-within-worlds, Universe navigation, tabletop mode, or another
  terrain-representation experiment;
- add or redesign procedural-horizon LOD except to fix a defect visible in the
  accepted opening;
- create a settlement-specific renderer or distant proxy;
- finish the global first-party asset catalogue, release packaging,
  monetization, or storefront work;
- guarantee the full composition on every possible seed; or
- make all worlds include a homestead when `Wild Start` is selected.

## Stop Conditions

Stop and seek review if:

- the structure canary needs cross-chunk far writes or bypasses status
  ownership;
- the survey needs profile-name branches instead of neutral facts;
- selection depends on request or completion order;
- the first composition requires destructive terrain flattening beyond the
  declared grade budget;
- ordinary reopen rescoring can move an established homestead;
- materialization can overwrite player edits or duplicate residents;
- a platform adapter must implement starter-content policy;
- first-party acceptance views require Minecraft-reference or diagnostic
  content; or
- the slice expands into survival breadth, LOD research, or maximal-settlement
  content before the opening loop is accepted.

## Documentation and Commit Series

Implementation commits use:

```text
Topic: starter-farmstead-settlement
```

Update this tactical as an execution record after every human-review gate.
Keep [`../topics/starter-farmstead-settlement.md`](../topics/starter-farmstead-settlement.md),
[`../structures.md`](../structures.md), the world-generation compatibility
ledger, platform evidence, and the tactical index reconciled when their facts
change.

## Related

- [`../topics/starter-farmstead-settlement.md`](../topics/starter-farmstead-settlement.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../structures.md`](../structures.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../client-experience-architecture.md`](../client-experience-architecture.md)
- [`214-structure-lab-vertical-slice.md`](214-structure-lab-vertical-slice.md)
- [`190-player-health-lava-death-and-respawn.md`](190-player-health-lava-death-and-respawn.md)
- [`261-procedural-horizon-product-integration-roadmap.md`](261-procedural-horizon-product-integration-roadmap.md)
