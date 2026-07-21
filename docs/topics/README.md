# Topics

Focused, living records of continuing concerns live here.

Prefer the smallest coherent topic whose status, decisions, evidence, and next
work benefit from continuity across sessions or commits. A topic can cover a
contract, recurring problem, product decision, implementation campaign, status
question, or investigation; it does not need to represent an entire subsystem.
Split topics when their decisions or next work can evolve independently.

Adopt this convention incrementally. Existing architecture, reference, and
status docs do not need to move here solely for consistency. Create or update a
topic when current status is hard to answer, work spans multiple tacticals or
commits, important invariants or decisions need to survive the current session,
new evidence changes the direction, or the user explicitly asks for one. Do not
create a topic for every small standalone change.

Documentation roles:

- Architecture and reference docs own durable system shape and external facts.
- Topic docs own current truth, decisions, evidence, gaps, and direction for a
  focused continuing concern.
- Tactical docs under `docs/tactical/` own bounded implementation slices and
  execution records.

New topics should normally start with a crisp scope, a `Topic: <slug>` line,
and an honest status. Add only the sections the concern needs, such as
motivation, current state, contracts and invariants, code/documentation map,
evidence and validation, known gaps, or recommended next work. When a commit
series implements the same concern, normally reuse the document slug in its
`Topic:` trailers.

## Current Topics

- [`beta-world-generation.md`](beta-world-generation.md): active `beta-v1`
  implementation contract for a standalone Beta 1.7.3 Overworld with staged
  core parity, deterministic flavor-close population, product selection, and
  visual acceptance.
- [`beta-1.7.3-reference.md`](beta-1.7.3-reference.md): reproducible Beta
  1.7.3 decompilation, traced old-Beta terrain/biome/cave/population
  architecture, and measured Alpha comparison supporting the implementation.
- [`alpha-era-reference.md`](alpha-era-reference.md): preserved early-worldgen
  study ladder, reproducible Alpha v1.1.2_01 decompilation and terrain oracle,
  detailed generator anatomy, and the Alpha v1.2.6 biome-era comparison that
  informed the implemented `alpha-v1` profile.
- [`alpha-world-generation.md`](alpha-world-generation.md): active
  implementation contract for the internal `alpha-v1` profile—Alpha
  v1.1.2_01-shaped terrain, explicit winter state, semantic oracle mapping,
  deterministic cross-chunk decoration, persistence, and visual acceptance.
- [`jjthunder-to-the-max-reference.md`](jjthunder-to-the-max-reference.md):
  extracted current community-datapack study covering its 2,096-block physical
  height, routed landform families, finite-difference pseudo-erosion,
  terrain-participating rivers, relative-depth cave hierarchy, and
  mountain-conditioned Underlands, with bounded lessons for the original
  mclone Overworld.
- [`multiplayer-networking.md`](multiplayer-networking.md): wire protocol,
  transports, session lifecycle, server tick/publication cadence — current
  state, structural gaps (lockstep request/response wire, tick-per-command
  dedicated server, no session layer), and the phased plan toward a
  vanilla-shaped push protocol with configurable tick rates.
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md): accepted unified
  `RealmServer` topology for integrated, Web Worker, dedicated, and test hosts;
  realm-scoped players/statistics, open-ended concurrent dimensions,
  dimension-local persistence/interest, ordinary local sessions, observer
  previews, warm transfer as client presentation, and typed statistics proven
  across transfer and restart. Tactical
  [`185`](../tactical/185-realm-dimension-and-observer-runtime.md) is complete.
- [`world-height-and-volumetric-streaming.md`](world-height-and-volumetric-streaming.md):
  current 256-block column/16-section shape, modern Java's 384-block Overworld
  and custom-height envelope, accepted per-dimension finite-range contract,
  practical cost tiers, near-term height cleanup, and the separate later path
  toward section-addressed or cubic residency.
- [`bounded-world-topology.md`](bounded-world-topology.md): accepted design for
  exact locally Euclidean finite and looping dimensions, including bounded
  planes and cylinders, flat tori, observer-local lifts, topology-aware
  generation, presentation-only visual bending, a fog-capped cube atlas, and
  deliberate deferral of true spherical regional rasterization. Tactical
  [`195`](../tactical/195-periodic-cylinder-topology-proof.md) owns the exact
  baseline, finite-bound canary, and first real Flat Grass cylinder.
- [`faithful-world-embeddings.md`](faithful-world-embeddings.md): complementary
  design exploration for dimensions whose exact slab, hinge, square-tube, or
  cuboid embedding is visible to the player, including patch-local authority,
  face-relative gravity, bounded cuboid shells, deterministic face-priority
  ownership, and a later bounded connection to worlds inside blocks.
- [`world-generation-profiles.md`](world-generation-profiles.md): accepted
  generator-profile direction and authoritative compatibility safety ledger;
  current reference-locked Overworld versus internal-mutable flat-grass,
  seeded-island, and authored-only proofs, plus the separate original-mclone
  profile identity.
  Tactical
  [`187`](../tactical/187-generator-profile-flat-grass-and-seeded-island.md)
  owns the bounded refactor and first two generators.
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md): accepted
  terrain-first direction for the original continuous mclone Overworld,
  including structured macro fields, module/reuse boundaries, explicit
  review/refactor stages, and the long-term relief, river, cave, geology, and
  landmark sequence. Tactical
  [`188`](../tactical/188-mclone-overworld-v1-terrain-foundation.md) owns the
  first bounded terrain foundation.
- [`client-prediction.md`](client-prediction.md): player movement authority —
  vanilla-shaped client-authoritative interim, planned server validation
  checks, the preserved sequenced-input-replay path, and remote-actor
  interpolation divergence.
- [`platform-parity.md`](platform-parity.md): cross-platform parity tracker —
  per-platform-class target state, the feature × platform matrix, the
  shared-contract × consumer reuse matrix, and the cross-cutting blockers that
  keep new features from re-forking across the client lanes and offscreen
  validation hosts.
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md): completed browser
  adoption of the shared `McloneSceneHost`, including landed service and
  typed-async boundaries, preserved worker/compiler topology, production
  cutover evidence, the subsequently proven browser Far LOD producer, and the
  exact remaining four-row browser feature ledger. Macro sequencing now lives
  in Tactical 171 — Convergence And Parity Closeout.
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md): accepted
  isolated-Rust-actor and domain-blind-TypeScript direction for browser
  Workers, including the landed server-job actor and main-side Rust render
  coordinator, current 5,275-line TypeScript inventory, external-SAB-mailbox
  versus shared-Wasm-heap distinction, copy/lifecycle tradeoffs, and explicit
  shared-linear-memory revisit gates. Tactical
  [`197`](../tactical/197-domain-blind-web-worker-broker.md) owns the approved
  autonomous high-value continuation and final long-tail review stop.
- [`unified-persistence-interface.md`](unified-persistence-interface.md):
  implemented typed completion-based Rust persistence port, shared
  coordinator, interchangeable SQLite, IndexedDB, and memory/null record
  executors, and world-scoped writer admission. Tactical
  [`199`](../tactical/199-unified-persistence-interface.md) records the
  implementation.
- [`world-dimension-storage-layout.md`](world-dimension-storage-layout.md):
  accepted physical-layout direction for a realm-global native database plus
  dimension SQLite shards, retained logical dimension keys and world-level
  ownership, and deliberately consolidated browser IndexedDB.
- [`cross-platform-operation-execution.md`](cross-platform-operation-execution.md):
  accepted shared Rust actor/mailbox direction across native threads and
  browser Workers while preserving direct native execution and domain-blind
  TypeScript. Completed Tactical 201 removed the accidental managed-lobby
  installer; completed Tactical 202 found no replacement coarse-operation
  actor and cleaned the surviving seams through their existing Rust owners.
- [`platform-host-boundary.md`](platform-host-boundary.md): accepted clean
  interactive host direction and code-grounded current/desired audit — one
  shared Rust input/context/action path across native, browser, Android, and XR
  where applicable; autonomous platform initialization and mechanics remain
  local; production TypeScript input semantics, the browser-Rust string-action
  dispatch surface, and smoke-state mirrors are the primary remaining gaps.
- [`far-lod.md`](far-lod.md): synthetic far-terrain LOD status — the landed
  resident-tile producer mechanism, the settle contract, the confirmed
  coverage-defect ledger (altitude graph-cull voids, suppression/painted
  mismatch), planned settle-state validation lanes, and the tactical 172 →
  162 ordering.
- [`lighting.md`](lighting.md): native Java-shaped stored-light system,
  render handoff, solver/status/rendering gaps, and next slices.
- [`dynamic-point-lights.md`](dynamic-point-lights.md): presentation-side
  finite-radius point lights, many-light admission, voxel-DDA and entity-shadow
  options, cubemap/stencil comparisons, and shared mono/XR validation direction.
- [`performance.md`](performance.md): high-priority known performance issues,
  low-hanging pickup guidance, native baselines, the broader priority queue,
  and Java-shaped render/scheduling follow-ups.
  The broader frame/terrain/host accounting model lives in
  [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md).
- [`lush-grass-rendering.md`](lush-grass-rendering.md): pinned Grassier Grass
  artifact/reconstruction research, observed section/wind/interaction/color
  architecture, attribution and license constraints, and the accepted
  independent mclone patch-instancing, LOD, multiview, and validation direction.
- [`compiled-figure-rendering.md`](compiled-figure-rendering.md): selected
  direction for compiling Asset Lab primitives, textures, rigs, clips, and
  generated LODs into shared static GPU figures with presentation-rate
  rigid-part animation, optional measured crowd pose evaluation, instancing,
  and cross-platform validation. Tactical
  [`181`](../tactical/181-compiled-figure-static-box-proof.md) owns the first
  bounded static-box/UV artifact proof.
- [`figure-surface-stability.md`](figure-surface-stability.md): accepted
  authoring contract for avoiding same-facing coplanar figure parts, the
  corrected Batch 23 pigeon evidence, and a deferred sampled-pose surface
  linter with a report-first catalog rollout.
- [`asset-pack-profiles.md`](asset-pack-profiles.md): shared UI asset-pack
  selection, pack-time generated missing assets, runtime provenance, and the
  standalone first-party boundary.
- [`falling-tree-physics.md`](falling-tree-physics.md): Dynamic Falling Tree
  and Sable reference investigation for future tree felling, moving voxel
  assemblies, and impact effects.
- [`embedded-worlds.md`](embedded-worlds.md): design north-star for rendering a
  second world inside the current one — lobby diorama, seed-explorer console,
  network "palantir" window, warm world switching, walkable seams, and
  shrink-and-fall nesting. Records geometry-first XR composition, the
  baked→live-local→remote-spectator→joined-warm fidelity ladder, N-world budget,
  runtime/authority/render seams, retained previews, A-to-B-to-A switching, and
  the protected single-world fast path. Tactical
  [`201`](../tactical/201-lobby-content-simplification.md) records the completed
  simplification from managed installed content to a transient authored lobby
  and ordinary persistent destinations.
- [`visor-vr-reference.md`](visor-vr-reference.md): Visor VR architecture
  research, including reusable lessons for world-space UI surfaces, pointer
  focus/capture, virtual-keyboard text entry, input contexts, render staging,
  lifecycle, addons, settings, and remote XR pose replication.
- [`vanilla/networking.md`](vanilla/networking.md): Minecraft Java 1.17.1
  vanilla networking reference — connection pipeline, login/join sequence,
  50 ms tick loop, chunk/entity sync cadences, movement validation and
  teleport acks, interpolation, and a constants quick-reference table.
- [`vanilla/weather.md`](vanilla/weather.md): Minecraft Java 1.17.1 vanilla
  weather reference notes, including rain, snow, thunder, lightning/fire,
  randomness, biome-local precipitation, and non-vanilla atmosphere boundaries.

## Update Policy

- Read the relevant topic before changing the behavior it governs.
- Update it when a tactical lands or its status, contract, evidence,
  validation, gaps, or recommended direction changes.
- Keep the main text as current truth rather than an append-only diary. Git and
  motivation-preserving commit bodies retain the history.
- Keep detailed per-slice execution in `docs/tactical/` and topic docs short
  enough to scan.
- Link relevant architecture/reference docs, Java files, native modules, and
  tacticals so future work starts from the right boundaries.
- Create a sibling topic instead of turning an existing topic into a catch-all.
