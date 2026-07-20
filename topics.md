# Commit Topics

Registry of topic strings used in `Topic:` commit trailers. This is not an
index of `docs/topics/`; the two normally share a slug when they represent the
same continuing concern.

Append a topic when its first commit is created. Do not reconstruct historical
topics unless doing so is useful. Keep each string exact across its commit
series so `git log --grep "Topic: ..."` finds the whole chain.

- `unified-native-scene-host` — tactical 168 shared host/driver convergence
- `asset-pack-profiles` — tactical 169 runtime selection and provenance
- `multiplayer-networking` — vanilla-shaped push protocol, session layer, and
  tick/publication cadence plan
- `client-prediction` — movement authority, server validation checks, and the
  preserved input-replay path
- `vanilla-networking` — 1.17.1 vanilla network stack reference receipts
- `web-scene-host-adoption` — tactical 170 browser adoption of the shared
  scene host
- `convergence-and-parity-closeout` — cross-tactical resident-tile, LOD,
  browser feature-parity, asset follow-up, and documentation burn-down
- `far-lod-settle-contract` — tactical 172 settle-state harness, coverage
  correctness burn-down, and LOD detail modes
- `shared-startup-configuration` — tactical 173 canonical launch
  configuration, scene-host consumption, and thin platform-source adapters
- `embedded-worlds` — nested/second-world rendering and warm-world runtime
  foundation (lobby diorama, seed explorer, network palantir, quick switching,
  seams/portals, shrink-and-fall); geometry-first XR composition, fidelity
  ladder, authority/runtime/render seams, and protected single-world fast path.
  Initial dual-integrated-host ownership smoke added; no commit series yet
- `performance` — high-priority known performance issues, measured pickup
  queue, baselines, and cross-platform performance follow-ups
- `dynamic-point-lights` — presentation-side finite-radius point lights,
  many-light admission, voxel-DDA and entity-shadow experiments, shadow
  technique comparison, and shared mono/XR validation
- `lush-grass-rendering` — dense biome-tinted procedural grass, patch
  instancing, distance LOD, wind, entity interaction, and cross-view rendering
  research/direction
- `compiled-figure-rendering` — compiled figure artifact, static local-space
  GPU geometry, presentation-rate rigid-part animation, instancing, and
  generated figure LOD direction; Tactical 181 static-box proof
- `realm-dimension-runtime` — unified integrated/dedicated realm server,
  concurrent dimension ownership, realm-scoped players/statistics,
  dimension-local persistence and interest, observer previews, and warm
  player transfer; Tactical 185
- `world-generation-profiles` — stable versioned generator identity, shared
  dispatch, flat-grass and seeded-island proofs, and the later frozen-vanilla
  versus original-mclone worldgen fork; Tactical 187
- `mclone-overworld-generation` — original continuous Overworld terrain,
  structured macro fields, biome/surface/decoration ownership, and explicit
  reuse/refactor review stages beginning with Tactical 188
- `alpha-world-generation` — Alpha v1.1.2_01 reference archaeology and the
  deterministic, selectable `alpha-v1` shared generator; Tactical 193
- `beta-1.7.3-reference` — pinned Beta 1.7.3 decompilation and source study
  comparing its old-Beta Overworld pipeline with Alpha
- `beta-world-generation` — standalone selectable Beta 1.7.3 Overworld with
  staged core parity and deterministic flavor-close population; Tactical 194
- `bounded-world-topology` — finite and periodic canonical dimension identity,
  observer-local lifts, seam-safe simulation/rendering, and later exact patch
  atlases; Tactical 195 begins with a Flat Grass X-periodic cylinder
- `world-height-and-volumetric-streaming` — authoritative finite per-dimension
  height, taller-world cost controls, and the separate path toward partial
  vertical or cubic residency
- `web-worker-runtime-ownership` — isolated Rust worker actors,
  domain-blind TypeScript browser brokers, explicit SAB mailbox ownership, and
  measurement-gated shared-Wasm-linear-memory reconsideration; Tacticals 197
  and 198
- `unified-persistence-interface` — one typed completion-based Rust persistence
  port with shared coordination and interchangeable SQLite, IndexedDB,
  filesystem, and memory/null record executors
- `cross-platform-operation-execution` — one typed operation/completion port
  over native background threads and browser Rust actors; shared Rust owns
  semantics while platform adapters and domain-blind TypeScript own mechanics.
  Tactical 201 removes the accidental managed-lobby installer before another
  coarse-operation actor is selected
- `world-dimension-storage-layout` — realm-global native SQLite metadata plus
  dimension-local SQLite shards beneath one world persistence owner, while
  browser IndexedDB remains physically consolidated
