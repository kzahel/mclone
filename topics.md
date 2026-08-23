# Commit Topics

Registry of topic strings used in `Topic:` commit trailers. This is not an
index of `docs/topics/`; the two normally share a slug when they represent the
same continuing concern.

Append a topic when its first commit is created. Do not reconstruct historical
topics unless doing so is useful. Keep each string exact across its commit
series so `git log --grep "Topic: ..."` finds the whole chain.

- `unified-native-scene-host` — tactical 168 shared host/driver convergence
- `asset-pack-profiles` — tactical 169 runtime selection and provenance
- `multiplayer-networking` — vanilla-shaped push protocol, session layer,
  tick/publication cadence, mixed-reliability transport, and native-host
  Share to Browser launch plan
- `client-prediction` — permissive client movement authority, server pose
  sanitization, and local semantic input recording
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
- `tabletop-overview-mode` — shared active-world scale-model presentation
  across flat and XR, including inverse-mapped interaction, capability-gated
  passthrough, survival/multiplayer authority, and same-slot renderer reuse
- `performance` — high-priority known performance issues, measured pickup
  queue, baselines, and cross-platform performance follow-ups
- `actor-rendering-performance` — prepared/fallback actor render baselines,
  instancing and memory tradeoffs, Quest acceptance, sparse upload, GPU pose,
  and actor LOD follow-ups
- `xr-render-path-switching` — live dual-eye, array per-eye, and array
  multiview selection across desktop and Android OpenXR; transactional
  frame-boundary target replacement with one steady-state swapchain family
- `dynamic-point-lights` — presentation-side finite-radius point lights,
  many-light admission, voxel-DDA and entity-shadow experiments, shadow
  technique comparison, and shared mono/XR validation
- `water-reflections` — optional environment and screen-space water
  reflections, surface eligibility, shared render architecture, quality tiers,
  XR constraints, and measurement-led experiments
- `lush-grass-rendering` — dense biome-tinted procedural grass, patch
  instancing, distance LOD, wind, entity interaction, and cross-view rendering
  research/direction
- `bushy-leaf-rendering` — derived active-pack leaf sprites, deterministic
  canopy-surface geometry, independent Blocky/Bushy graphics policy, and
  shared mono/stereo/multiview validation
- `compiled-figure-rendering` — compiled figure artifact, static local-space
  GPU geometry, presentation-rate rigid-part animation, instancing, and
  generated figure LOD direction; Tactical 181 static-box proof
- `figure-animation-actions` — authored default clips, locomotion/idle/action
  roles, one-shot completion, catalogue action controls, and the path beyond
  runtime's hard-coded walk selection
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
- `generated-chunk-cache-policy` — authority-owned per-world choice to store or
  regenerate deterministic unedited terrain cache, with durable provenance and
  future reclamation boundaries
- `chunk-lighting-admission-and-backpressure` — bounded current-interest Player
  promotion, keyed Light demand, shared immutable Light inputs, cancellation,
  ticket conservation, and long-travel memory/throughput gates
- `platform-host-boundary` — one shared Rust input/context/action path across
  desktop, browser, flat Android, and XR interactive hosts; autonomous
  platform initialization and mechanics stay local; browser TS input
  semantics, web-Rust string-action dispatch, and production smoke mirrors
  are the gaps to close
- `client-entry-lifecycle` — menu-first shared entry intent, repeatable host
  lifecycle, bounded idle work, and managed development process ownership
- `figure-surface-stability` — same-facing coplanar and near-coplanar figure
  surfaces, sampled-pose exact-overlap and animated head-socket margin gates,
  reasoned face-pair exceptions, ratcheted warning inventory, and corrected
  catalog evidence
- `figure-geometry-analysis` — rest-pose canonical figure connectivity,
  oriented-box component analysis, reasoned exact-component exceptions, and
  later sampled animation checks
- `figure-ground-penetration` — sampled land-figure ground-plane analysis,
  declared-contact exclusion, exact reasoned exceptions, ratcheted warning
  inventory, and corrected Chameleon tail evidence
- `figure-alpha-cutout` — completed binary transparent ASCII palette texels
  across the semantic preview and native prepared/actor paths
- `figure-transparency-materials` — explicit figure alpha modes, compiler pass
  buckets, whole-actor opacity, and paired dithered/smooth Ghost proofs without
  prematurely adopting general OIT
- `figure-card-primitives` — fixed planar figure surfaces, explicit one- or
  two-sided rendering, box-and-card canonical authoring, and card-aware asset
  analysis across semantic preview and native prepared rendering
- `animal-catalogue` — shared Asset Lab semantic viewer, generated canonical
  figure catalogue, and production `/animals/` deployment
- `platform-boundary-convergence` — parent ledger for the shared/platform
  code-split campaign: both-language scoreboard, pass history, and the
  standalone-audit closure protocol above the child boundary topics
- `starter-farmstead-settlement` — reusable authored building vocabulary and
  the staged path from standalone templates to an adaptive demo farmstead;
  Tactical 208 begins implementation with the cottage-and-barn lab
- `structure-lab` — source-first authored structure DSL, generated-JSON drift
  gates, Rust-baked review artifacts, and a read-only public catalogue
- `controller-input` — all-target ordinary gamepad collection, shared semantic
  action/context resolution, controller-accessible UI, tracked XR convergence,
  and the later native Steam Input path
- `structure-catalogue-product` — brainstorm-stage cross-ecosystem structure
  publishing, creator, growth, and economy vision: rights-clean semantic builds
  can feed web guides, Java/Bedrock artifacts, optional Mclone play, and
  deterministic construction-reveal media; co-branded storefronts and licensed
  human examples support distribution and an AI quality ladder of skill,
  retrieval, evals, and eventual training; does not reopen the read-only first
  proof
- `procedural-structure-starts` — reusable vanilla-shaped placement, start,
  bounding-box, reference, and clipped-piece machinery, first exercised by
  bounded terrain-affecting Mclone streams
- `local-couch-multiplayer` — 1-4 local participants, ordinary realm player
  endpoints, shared multi-presentation residency, split/auxiliary views,
  helper-builder roles, and mixed XR-plus-flat couch play
- `persistent-actor-identity` — vanilla actor save/load semantics across
  authored-world relaunch: ephemeral runtime IDs and generic tick counters,
  durable UUID-equivalent identity and per-kind gameplay state, corrected
  lifecycle evidence, and shared cross-backend proof
- `release-distribution-and-updates` — first-party and store release ownership,
  a stable direct-desktop launcher, signed manifests, transactional version
  slots, whole-artifact-first updates, and measured later delta optimization
- `distribution-go-to-market` — Steam-preferred PC distribution with a real
  no-Steam edition, first-class no-install web play, paid/demo/full-free
  commercial options, Quest and Deck positioning, competitive landscape,
  launch sequencing, and success measures
- `input-observation-timeline` — loss-aware cross-platform physical input
  observations, shared semantic reduction, and sequenced 60 Hz player commands
  independent of presentation cadence
- `touchscreen-input` — direct menu touch, shared in-world touch controls,
  capability-led visibility and preferences, desktop/Deck collection, and
  physical touchscreen acceptance
- `remote-player-presentation` — remote body/head/hand embodiment, independent
  report/replication/presentation cadences, buffered interpolation, and
  transport-neutral ephemeral pose semantics
- `browser-hosted-peer-sessions` — browser-authoritative rooms over WebRTC
  reliable/ephemeral data channels, room-key signaling, direct ICE with
  conditional TURN, and worker-owned Rust authority
- `mclone-overworld-breadth` — grouped vanilla breadth reference versus live
  original-profile regional recipes, surfaces, vegetation, water, ecology,
  landmarks, and a dedicated volumetric rock-formation campaign
- `lod-native-vegetation` — one deterministic tree identity across exact
  Mclone generation, Terrain Lab summaries/proxies, and later in-game Far LOD
- `steam-deck-test-bed` — official Devkit Client deployment, native Linux
  staging and asset-root contracts, Gaming Mode playtesting, and reproducible
  physical handheld performance evidence
- `steam-deck-rd10-plus-performance` — physical Steam Deck proof campaign for
  constant-time render bookkeeping, static and moving visibility, incremental
  ticket levels, fluid/remesh churn, worker capacity, GPU attribution, and
  reduced RD10+ draw submission
- `graphics-video-settings` — player-facing graphics/video controls,
  platform-profile and stored-preference precedence, output/UI/world
  resolution policy, and the remaining production settings backlog
- `fog-atmosphere` — shared open-air linear, exponential, and height-aware
  distance atmosphere, interactive graphics controls, exact/procedural
  coverage concealment, weather response, and conservative far culling
- `game-title-and-brand-identity` — public-title research and decision record:
  repo-derived naming brief, Wilderfold recommendation, retained fallbacks,
  preliminary availability evidence, and clearance/adoption gates
- `worldgen-debug-lens` — production-derived biome, landform, surface, and
  hydrology diagnostics projected through a cached terrain overlay and
  crosshair inspector without entering chunk persistence or generation state
- `gpu-procedural-terrain` — GPU-reconstructible first-party terrain,
  coverage-first progressive refinement, authoritative chunk handoff, optional
  asynchronous canonical generation, and later volumetric residency research
- `vanilla-terrain-lod` — direct Java 1.17.1 density-column terrain previews,
  a global Terrain Lab profile switch, CPU-worker tile generation, bounded
  macro fidelity, and explicit first-pass surface/content exclusions
- `scripting-and-mod-platform` — one cross-platform package ecosystem spanning
  declarative content, portable capability-sandboxed gameplay, trusted
  server/desktop extensions, source forks, reproducible profiles, an open
  registry protocol, and a first-party in-game mod browser
- `texture-material-profiles` — curated and provisional first-party textures,
  real local Minecraft comparison, opt-in numbered diagnostics, named shared
  visual profiles, derived detail representations, and one canonical Texture
  Lab promotion lifecycle
- `retire-chunk-far-lod` — aggressive removal of the experimental
  chunk-granular in-game Far LOD vertical feature while retaining only worker,
  budget, upload, and exact-render machinery with active non-LOD consumers
- `procedural-horizon-clipmap` — fixed-budget toroidal natural-terrain rings,
  skirts, exact-painted chunk masking, vegetation handoff, and predictable
  native/browser/XR frame admission
- `procedural-horizon-surface-appearance` — block-atlas ground texture,
  biome/material color, approximate lighting, interpolated inland water, and
  measured fixed-budget quality improvements
- `world-view-navigation` — shared map/orbit/focus/zoom control across Terrain
  Lab, standalone World Explorer, tabletop, and game consumers, plus the
  authoritative Explorer-to-play handoff
- `mclone-macro-landscape-planning` — eagle-eye composition of terrain,
  hydrology, coasts, geology, ecology, landmarks, and negative space with
  bounded topology-aware plans, scale-aware previews, and selective
  volumetric terrain
- `deterministic-streamed-landscape-planning` — falsifiable research into a
  novel relational macro planner with exact request/path/cache/window/order
  independence, semantic boundary agreement, topology-aware bounded work,
  comparable performance evidence, and a coordinate-pure fallback
- `modern-minecraft-reference` — pinned current-stable Java comparative
  source lane, official unobfuscated jar bootstrap, focused worldgen research,
  refresh policy, and post-1.18 findings without changing the 1.17.1 target
- `universe-product-shell` — first-class product navigation across catalogs,
  detached previews, observers, live play, resume policy, and Lab workbenches
  while preserving current realm, scene, terrain-view, and platform owners
- `multiscale-terrain-representation` — direct semantic coarse-to-fine
  geography, cheap canonical surface queries, sparse implicit 3D terrain, and
  conservative distant summaries under bounded random access and periodic
  topology
- `quest-testbed` — public physical Quest ADB provider, recoverable headset
  leases, and removal of project-local wake/proximity/restore implementations
- `first-party-sound-effects` — provenance-locked distributable CC0 effects,
  shared variant and material semantics, gameplay/UI producers, and
  cross-platform audio preparation
- `habitat-driven-creature-ecology` — terrain and creature co-design,
  generated-world habitat queries, durable visible animals, explicit ambient
  summaries, and mechanics-led Creature Lab promotion
- `seasons` — regional seasonal climate, plane/cylinder latitude, derived
  snow and water response, active-boundary migration encounters, frozen
  unloaded state, and tagged wildlife/managed-habitat decisions
- `playable-showcases` — bounded data-driven tiny-save recipes, live-game
  instantiation evidence, matched capture/browser review, and temporary hosted
  play links
- `wheat-farming` — player-created irrigated farmland, loaded-world crop
  growth, renewable harvest, persistence, and later farmstead/pollinator use
- `functional-kitchen-gardens` — connected fences and gates, reusable mixed
  crops, source-first garden composition, ordinary homestead integration, and
  future animal-pressure gameplay
- `rabbit-burrow-ecology` — one genuinely excavated shallow bank threshold,
  persistent rabbit families, time-aware emergence, garden pressure, carrot
  breeding, and a bounded ordinary-system showcase
- `wildlife-ecology-state-model` — persistent individual knowledge,
  unavailable-world semantics, bounded social/habitat patterns, staggered AI
  work, and consistent lazy checkpoints across varied wildlife
- `held-item-selection-sync` — automatic shared client-to-server carried-slot
  synchronization, interaction ordering, and held-item-driven creature behavior
- `semantic-figure-assets` — one checked TypeScript-to-semantic-JSON asset path
  for actor figures plus world and item props, beginning with mallard prop
  migration and deer runtime promotion
- `terrain-surface-traces` — bounded terrain-conforming decals or surface
  patches for ecological tracks and scuffs, separate from rigid semantic
  figures and persistent terrain mutation
- `desktop-openxr-validation` — Windows VDXR and macOS/Linux WiVRn runtime
  bootstrap, headset-backed smoke evidence, Vulkan interop, and remaining
  desktop OpenXR hardware acceptance
- `continental-ecoregion-planning` — top-down continental, physiographic,
  ecoregional, and landscape-mosaic authorship with bottom-up realization,
  ecology-scale habitat geography, and review-gated production integration
- `continental-hydrography` — bounded feature-owned mountain-to-lake
  catchments, directed reaches and basin spills, analytic terrain realization,
  direct exact/LOD queries, and downstream riparian ecology
- `quest-frontier-performance` — pixel-preserving suppression, transition
  shading, and support-geometry reductions for stable Low/RD8 Quest headroom
- `v2-forest-continuity-and-performance` — honest full-view V1/V2 Quest
  measurement, fixed-budget coarse canopy continuity, and measured V2 cost
  recovery without reducing terrain or ecology content
- `isocraft-reference` — pinned Isocraft reveal implementation study,
  topology-first room/local-cave masks, geometry cutaway, and lessons for an
  optional Mclone adventure-view control profile
- `ecstatic-lod-reference` — exact Ecstatic 1.3.0 implementation study and
  source-level comparison with Mclone's geometry clipmap, Distant Horizons'
  multi-span columns, and Voxy's volumetric GPU hierarchy
