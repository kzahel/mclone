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

- [`persistent-actor-identity.md`](persistent-actor-identity.md): open,
  vanilla-classified actor save/load contract — runtime IDs and generic tick
  counters may reset, UUID-equivalent identity and per-kind gameplay state must
  persist, and the authored-destination lifecycle fixture needs corresponding
  correction and cross-backend proof.
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
  transports, session lifecycle, server tick/publication cadence, accepted
  carrier-neutral reliable/ephemeral direction, dependency-free native UDP
  first slice, future browser-capable carriers, and native-host **Share to
  Browser** links through a versioned public HTTPS launcher.
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
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md): active
  original-profile breadth ledger measured against grouped Java 1.17.1
  families, with explicit mechanism/live/reviewed states, regional recipe
  gaps, and a first-class 3D geology campaign for boulders, tors, arches,
  hoodoos, cliff shelves, overhangs, and other volumetric rock formations.
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md): accepted
  terrain-adaptive blueprint direction for a maximal but incrementally built
  demo farmstead, including a cross-profile starter-content overlay, vanilla
  structure vocabulary, hydrology and authored-water fallbacks, site grading,
  exact touched-chunk scheduling, authored-tree reservation, persistent
  residents, an `FS-*` dependency/proof ledger, tactical completion gates, and
  the deliberately deferred Far LOD proxy.
- [`structure-lab.md`](structure-lab.md): accepted source-first Structure Lab
  direction—agent-authored TypeScript DSL, mandatory generated-JSON drift
  gates, Rust build-time baked meshes, a polished read-only
  React/Zustand/Three.js catalogue, checked runtime promotion, and indefinite
  deferral of in-browser editing.
- [`structure-catalogue-product.md`](structure-catalogue-product.md):
  brainstorm-stage product/growth/economy vision for a "better GrabCraft" that
  is also a zero-install acquisition funnel into Mclone—generator-not-library
  framing, walk-through and honestly-scoped build-along, deterministic guides
  and bills of materials, programmatic-SEO distribution, and a prompt-iteration
  credit economy that meters bespoke AI authoring while keeping discovery and
  play free. Does not reopen the read-only first proof.
- [`client-prediction.md`](client-prediction.md): player movement authority —
  accepted permissive client authority, finite-value/bounds safety, no planned
  server movement replay, and explicit teleport continuity.
- [`input-observation-timeline.md`](input-observation-timeline.md): accepted
  cross-platform input timeline — preserve ordered transitions where a
  platform exposes them, degrade honestly to snapshots, normalize monotonic
  timing, and feed shared semantic 60 Hz player commands.
- [`remote-player-presentation.md`](remote-player-presentation.md): accepted
  remote embodiment work stream — independently configurable body/tracked-pose
  report and replication cadences, buffered interpolation, separate XR
  body/head/hands, transport-neutral loss-tolerant pose semantics,
  dependency-free native TCP-plus-UDP first, and TCP/WebSocket compatibility
  with future WebTransport/WebRTC carriers.
- [`browser-hosted-peer-sessions.md`](browser-hosted-peer-sessions.md):
  accepted browser-authoritative room topology — the existing worker
  `RealmServer`, room-key signaling, WebRTC reliable/ephemeral channels,
  direct ICE with conditional TURN, and explicit browser lifecycle limits.
- [`platform-parity.md`](platform-parity.md): cross-platform parity tracker —
  per-platform-class target state, the feature × platform matrix, the
  shared-contract × consumer reuse matrix, and the cross-cutting blockers that
  keep new features from re-forking across the client lanes and offscreen
  validation hosts.
- [`steam-deck-test-bed.md`](steam-deck-test-bed.md): active physical Steam
  Deck provisioning and validation lane — official Devkit Client deployment,
  native Linux staging and asset-root contracts, Gaming Mode acceptance, and
  reproducible handheld performance evidence.
- [`graphics-video-settings.md`](graphics-video-settings.md): active
  player-facing graphics/video contract — current live controls and SteamOS
  profile behavior, settings-persistence investigation, output/UI/world
  resolution policy, and the remaining production settings backlog.
- [`release-distribution-and-updates.md`](release-distribution-and-updates.md):
  accepted first-party/store distribution architecture — a stable Tauri
  launcher for managed direct desktop installs, store-owned updates for
  Steam/Quest builds, signed static release metadata, transactional version
  slots, whole-artifact-first updates, and measurement-gated delta work.
- [`distribution-go-to-market.md`](distribution-go-to-market.md): current
  commercial-channel and launch direction — Steam as the preferred PC surface,
  a supported no-Steam direct edition, paid/demo/full-free model options,
  web as a first-class no-install client and acquisition surface, Quest as a
  differentiation wedge, competitive landscape, positioning, launch
  sequencing, metrics, and unresolved store-policy decisions.
- [`game-title-and-brand-identity.md`](game-title-and-brand-identity.md):
  current public-title research and decision record — the repo-derived
  living-landscape plus folded-world naming brief, `Wilderfold` recommendation,
  retained fallbacks, dated preliminary availability evidence, near-name
  concerns, and explicit clearance and adoption gates.
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
  active shared Rust actor/mailbox direction across native threads and browser
  Workers while preserving direct native execution and domain-blind
  TypeScript. Completed Tacticals 201-202 removed the accidental managed-lobby
  installer and its clear platform decisions; active Tactical 207 has landed
  Slices 0–2 and Gate A and is converging the remaining scene-operation pump.
- [`platform-host-boundary.md`](platform-host-boundary.md): completed
  interactive host convergence — one shared Rust input/context/action path
  across native, browser, Android, and XR where applicable; Rust-owned browser
  preferences/bootstrap/status; operational-only product reports; and an
  explicit semantic test observer while autonomous platform mechanics remain
  local.
- [`controller-input.md`](controller-input.md): accepted all-target controller
  architecture — standard gamepad snapshots, semantic action/context state,
  shared UI navigation and prompts, thin desktop/web/Android collectors,
  tracked XR extensions, and the later native Steam Input path.
- [`local-couch-multiplayer.md`](local-couch-multiplayer.md): accepted 1-4
  local-participant and multi-presentation direction — ordinary realm player
  endpoints, shared unioned residency with per-view culling, single-player
  auxiliary panes, helper/builder roles, Bedrock study plan, and mixed
  XR-plus-flat couch play.
- [`platform-boundary-convergence.md`](platform-boundary-convergence.md):
  parent record for the shared/platform code-split campaign — the
  sixteen-pass ledger, the 2026-07-21 two-sided measured audit, the
  both-language scoreboard every pass must report, and the standalone-audit
  closure protocol that keeps implementing tacticals from declaring the
  whole concern done.
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
- [`figure-alpha-cutout.md`](figure-alpha-cutout.md): implemented binary
  transparent-palette contract across semantic validation, Asset Lab preview,
  prepared atlases, mono/stereo/multiview shaders, and the Cutout Skeleton
  visual proof.
- [`figure-transparency-materials.md`](figure-transparency-materials.md):
  accepted explicit opaque/mask/blend/additive material contract, prepared
  pass bucketing, whole-figure opacity, and dithered versus smooth Ghost proofs;
  exact OIT remains deferred.
- [`figure-animation-actions.md`](figure-animation-actions.md): accepted
  extensible clip contract for authored defaults, locomotion/idle/action roles,
  one-shot completion, catalogue action controls, and later general runtime
  clip requests beyond the hard-coded walk path.
- [`figure-surface-stability.md`](figure-surface-stability.md): required
  sampled-pose coplanar-face and animated head-socket margin gates, exact
  reasoned exceptions, ratcheted legacy warning inventory, and corrected
  catalog evidence.
- [`figure-geometry-analysis.md`](figure-geometry-analysis.md): implemented
  rest-pose oriented-box connectivity gate for canonical Asset Lab figures,
  exact-component exceptions with mandatory reasons, current zero-warning
  baseline, and the separate path toward sampled animation checks.
- [`figure-ground-penetration.md`](figure-ground-penetration.md): required
  sampled-pose land-figure ground gate, declared-contact exclusion, exact-part
  reasoned exceptions, ratcheted catalog warnings, and corrected Chameleon
  evidence.
- [`animal-catalogue.md`](animal-catalogue.md): active read-only React and
  Three.js catalogue for canonical Asset Lab figures, generated from validated
  semantic JSON and deployed at `/animals/` through the existing web bundle.
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
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md): researched
  potential for presenting the active world as one shared manipulable scale
  model across flat, touch, gamepad, XR, and capability-gated passthrough;
  records edit and direct-control adventure purposes, same-slot renderer reuse,
  camera follow and leashed XR recentering, bounded keyhole cutaways, inverse
  target mapping, authority boundaries, platform-neutral input, current gaps,
  and a read-only-first implementation sequence.
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
