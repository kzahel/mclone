# Tactical 284: Deer Forest-Edge Ecology and Semantic Props

Status: active; Review 1 semantic props pending 2026-08-12

Topics:

- `habitat-driven-creature-ecology`
- `figure-animation-actions`
- `semantic-figure-assets`

## Intent

Promote the approved white-tailed deer from Asset Lab into an ordinary,
persistent Mclone forest-edge population with authored idle, alert, feeding,
bedding, locomotion, and reaction sequences. The deer chapter should establish
tracking and a first honest hunting/harvest loop, make forest cover, clearings,
sight lines, disturbance, and local population history mechanically legible,
and finish with a transient playable showcase that exercises ordinary live
systems.

Use the chapter to remove two presentation shortcuts rather than adding more:

1. generalize the existing semantic figure authoring, generated-JSON,
   preparation, pack, and review machinery so checked first-party assets can be
   either actor figures or world/item/trace props; and
2. replace the renderer-local `Walk` choice with shared named-clip playback
   driven by authoritative gameplay state.

The mallard nest is the first required semantic-prop migration. Migrate the
existing mallard feather and track presentation through the same route so the
proof covers a durable world prop, a collectible item prop, and a transient
trace prop. Deer tracks, beds/sign, and shed antlers must then consume that
ordinary prop path rather than introduce deer-specific mesh code.

This is a gameplay and content-pipeline tactical, not merely a new model or a
showcase. It is complete only when the promoted assets and behaviors occur in
ordinary generated Mclone worlds, remain correct through persistence and
replication, render through shared mono/stereo/multiview paths, and are then
packaged for review by the showcase system.

## Current Baseline

- `tools/asset-lab/examples/deer/figure.ts` is an approved canonical box-only
  white-tailed buck. It has a reviewed long-legged silhouette and one authored
  `walk` locomotion clip, but no explicit multi-state behavior set and no
  runtime promotion.
- Asset Lab's schema already represents arbitrary rigid-part assemblies,
  materials, textures, named clips, clip roles, one-shot completion, and
  `nextClip`. Despite the `FigureAsset` name, its geometry compiler is not
  inherently limited to living characters.
- The checked promotion registry and Rust-facing IDs/paths are actor-centric.
  The public `/animals/` view is correctly creature-centric and should not
  silently classify nests, tracks, or antlers as animals.
- Shared Rust preparation preserves arbitrary clip names and action metadata,
  while live actor composition still reduces animation to
  `ActorAnimationClip::Walk` and the literal `walk` clip.
- The mallard itself uses checked generated semantic JSON and the prepared
  figure renderer. The mallard nest, feather, track marks, and generic egg
  presentation are still assembled by procedural functions in
  `mclone-render/src/entity.rs`.
- Mallard gameplay already proves persistent world props, collectible items,
  ephemeral traces, habitat-aware AI, life-stage state, field observations,
  and data-driven playable showcases. Those mechanics are reusable patterns,
  not permission to copy mallard-specific branches for deer.
- Passive entities do not yet have a general player-attack, health, death, and
  species-drop path. A deer implementation must not describe despawning or a
  debug deletion hook as hunting.

## Terminology and Product Boundary

Use **Asset Lab** for the source-first semantic authoring and review machinery.
An authored semantic model may represent an actor or a prop.

Use **Creature Lab** or **Creature Catalogue** for the creature-focused view of
that content. It remains useful precisely because it excludes non-creatures
from creature counts, habitat filters, and anatomy comparisons. Props may use
the same viewport, sheet, video, comparison, validation, and promotion code
without pretending to be creatures. A small developer-facing asset-kind
filter or prop review page is acceptable; renaming or rebuilding the deployed
creature catalogue is not required for this tactical.

Retain `FigureAsset` as the schema-v1 compatibility name unless implementation
shows that an additive field cannot express the selected role. Do not perform
a repository-wide type rename merely to make the word “figure” more generic.
The durable concept is a semantic rigid-part model; actor/prop use is promotion
metadata and runtime presentation policy.

## Selected Asset Architecture

### One authored semantic source

Every promoted deer or prop asset has one human/AI-authored TypeScript source.
Generated semantic JSON remains the checked runtime handoff and is never
edited directly:

```text
Asset Lab TypeScript source
  -> execute DSL
  -> serialize canonical schema-v1 semantic JSON
  -> reparse and validate the JSON
  -> semantic Three.js review
  -> checked first-party promotion registry
  -> generated runtime semantic JSON
  -> mclone-assets preparation
  -> shared prepared figure renderer
```

The same source must drive previews, sheets, videos, catalogue metadata,
first-party drift checks, the runtime pack, and engine comparison captures.
There is no parallel “game model,” hand-edited runtime JSON, or showcase-only
mesh.

### Explicit promotion use

Generalize the checked first-party promotion record to carry an explicit use:

- actor figure;
- durable world prop;
- item prop; or
- transient trace prop.

Keep geometry preparation common. Use determines catalogue admission,
anchoring/presentation validation, and which runtime owner may instantiate the
asset; it does not fork the DSL or compiler. Where practical, make the checked
promotion manifest feed Rust asset inventory and lookup so each new prop does
not require matching hand-maintained lists in TypeScript, asset packing, and
multiple Rust modules.

Existing actor-only API names may remain compatibility aliases during this
slice. New code should operate on a resource-location-like semantic asset ID
and a prepared semantic model rather than grow parallel `DeerModel`,
`NestMesh`, or `AntlerMesh` registries.

### Prop anchors and gameplay truth

Promotion metadata must declare the presentation anchor needed by the first
proofs: ground, item-center, or surface trace. Anchor validation belongs to the
asset pipeline and render-session composition. Collision, interaction bounds,
ownership, incubation, pickup, expiry, drops, and persistence remain explicit
gameplay facts. Asset bounds must never silently become authoritative physics.

The prepared renderer must accept stable presentation identity for both
durable and transient props. A moving feather item may bob or rotate through
ordinary item presentation; a track may expire; neither case justifies
recompiling topology or rebuilding all actors.

### Required mallard migrations

Author and promote at least:

| Asset | Use | Required equivalence |
|---|---|---|
| `mallard_nest` | durable world prop | visible nest and egg, correct ground anchor, ordinary incubation entity unchanged |
| `mallard_feather` | item prop | collectible item identity, pickup, cooldown, and inventory behavior unchanged |
| `mallard_tracks` | transient trace prop | bounded local cap, expiry, orientation, and observation behavior unchanged |

Remove the corresponding procedural geometry only after prepared native and
Web pixels are accepted. Do not retain a hidden second production rendering
path “just in case.” A clearly reported missing-asset/debug fallback may
remain only if it is shared and cannot make first-party validation pass while
the required prop is absent.

The nest should read as a low woven/reed bowl with a visible egg instead of a
solid block mound. It remains one semantic assembly for this slice; incubation
logic does not move into animation or asset metadata.

## Deer Asset and Animation Contract

Refine the current deer source rather than create a second unreviewed deer.
Its single-segment legs are unlikely to fold convincingly, so add only the
articulation required for grounded feeding, bedding, standing, and running.
Preserve the accepted narrow body, large ears, white tail flag, and sparse
forked-antler silhouette.

Author and inspect this minimum clip vocabulary:

| Clip | Role and phase | Gameplay use |
|---|---|---|
| `idle` | looping elapsed-time idle | relaxed standing and subtle ear/head motion |
| `walk` | distance-driven locomotion | browsing travel and herd repositioning |
| `flee` | faster distance-driven locomotion | alarm escape with raised tail flag |
| `alert` | looping elapsed-time idle | head high, ears scanning, body still |
| `graze` | looping elapsed-time idle/action | browse grass or low vegetation at a valid feeding site |
| `lie_down` | non-looping action to `bedded_idle` | enter a validated bedding site |
| `bedded_idle` | looping elapsed-time idle | remain grounded without foot sliding or hovering |
| `stand_up` | non-looping action to `idle` | leave bedding before travel or flight |
| `hit` | bounded non-looping action | readable accepted damage without displacing authority |
| `fall` | non-looping terminal action | completed death presentation before drops/removal policy finishes |

Exact names may change once authored, but the roles and transitions may not be
collapsed into one procedural walk or renderer-side joint rotations. Review
every clip independently and review each transition boundary. `lie_down` and
`stand_up` must be authored as compatible inverse transitions; the final
`lie_down` pose must equal the start of `bedded_idle`, and the final
`stand_up` pose must equal standing `idle` within a small transform tolerance.

Ground/contact analysis must sample all clips, including folded legs, muzzle
at graze, antlers, and the body at full bedding. Exceptions require the exact
part set and a reason; parent links do not prove contact. Generate clean
multi-angle sheets and deterministic multi-cycle or one-shot videos for human
inspection before runtime promotion.

The first population may use the current buck appearance, but species state
must not hard-code “every deer is a permanent antlered adult.” Before the
chapter is called a breeding foundation, it needs an explicit sex/life-stage
and antler-visibility/variant design. Breeding itself is outside this tactical
unless the implementation can add it without weakening the hunting, habitat,
or persistence acceptance gates.

## Shared Runtime Animation Contract

Replace renderer-local `ActorAnimationClip::Walk` with a renderer-neutral
named-clip request that supports two time sources:

- distance-driven phase for locomotion; and
- elapsed presentation time from an authoritative start tick/epoch for idle
  and one-shot actions.

Authoritative gameplay chooses deer behavior and owns when transitions start.
The client retains replicated behavior plus a monotonic state epoch/start tick.
Render-session maps that presentation state to a named clip and phase source.
The renderer resolves and evaluates a prepared clip; it does not infer fear,
feeding, sleep, damage, or death from velocity.

Rejoining, chunk hydration, a delayed update, or a rendering pause must sample
the correct current action instead of restarting it locally. Distance-driven
clips retain the existing continuous presentation-distance phase. A missing
optional clip may fall back through a documented semantic role, but promoted
first-party deer acceptance fails if any required clip or transition is
missing.

No crossfade system is required initially. Endpoint-compatible authored clips
and authoritative transition timing are the bounded answer. Do not add a
deer-only animation enum to `mclone-render`, and do not put animation policy in
desktop, browser, Android, or XR apps.

## Forest-Edge Habitat Contract

Introduce a shared habitat sample/fitness result above raw biome membership.
The first deer sample should expose at least:

- local canopy or woody-cover density;
- distance/direction to an open clearing or meadow edge;
- grass or low browse at the candidate site;
- slope and collision-safe standing area;
- nearby water access without requiring a wetland;
- connected escape cover and useful sight-line distance; and
- recent player disturbance.

Measure current Mclone terrain before changing generation. Then make
forest-edge/clearing intent an explicit Mclone generation fact rather than an
accidental threshold discovered independently by the spawner. Where the
existing vegetation distribution does not produce enough connected edge, use
that same fact to preserve sparse clearings or low-tree-density shoulders.
Heightfield shape should not be churned merely to place deer.

Natural spawning combines the generated semantic fact with live block checks
so player clearing, fencing, flooding, or obstruction matters. Deer spawn in
bounded small groups, count against the ordinary creature budget, respect
player/spawn distance and entity-load readiness, and become durable as soon as
materialized. Java 1.17.1 biome tables and the reference-locked `overworld`
profile remain unchanged; this original ecology belongs to the Mclone profile.

Deterministic atlas/fixture tests must show habitat cores, edge shoulders,
rejections in dense interior forest/open empty plain/steep or obstructed land,
and useful rarity over a multi-region sample. Do not tune against one showcase
seed.

## Deer Behavior and Sign

The authoritative deer loop should make habitat readable without requiring a
debug overlay:

- browse or graze at valid edge/clearing forage;
- drink at safe banks when a valid route exists;
- choose and retain covered bedding sites, lie down, remain bedded, and stand
  before movement;
- form loose bounded groups with cohesion and separation rather than a tight
  synchronized blob;
- become alert from nearby visible or noisy player movement, orient toward the
  disturbance, then flee toward connected cover if pressure continues; and
- settle only after sufficient distance/time without immediately oscillating
  between alert and calm.

Use actual displacement for body orientation and locomotion phase. Preserve
the mallard lesson: blocked or stationary entities must not rapidly rewrite yaw
while searching for a destination.

Deer leave bounded evidence even when not visible:

- oriented track pairs after qualifying movement on receptive ground;
- a bed/sign at a repeatedly used covered rest site; and
- a rare shed antler associated with an adult antler-bearing deer and a durable
  cooldown or seasonal placeholder.

Use semantic props for all three. Track lifetime may remain presentation-only;
beds/sign and antlers need explicit persistence/collection decisions. Do not
mutate arbitrary terrain blocks into paths in this first chapter. Persistent
terrain wear is a later stewardship/world-history mechanic after compact sign
and population state are proven.

## Hunting, Harvest, and Population Consequence

Add the smallest honest shared living-entity damage/death foundation needed by
deer. The player action must pass through host-neutral input, entity raycast or
target selection, an authoritative command, reach/line-of-sight/cadence
validation, durable health state, damage reaction, death, removal, and item
drops. A showcase command or test-only deletion does not satisfy this gate.

The first harvest should expose species-specific venison, hide, and an antler
only when the deer state warrants it. These items use ordinary inventory,
drop, pickup, persistence, and semantic item-prop presentation. Raw meat need
not complete cooking, recipes, hunger, equipment, or trade in this tactical;
the UI/field note must describe that limitation honestly.

Do not claim a polished hunt if the only normal tool is implausible repeated
bare-hand pursuit. Reuse an existing ordinary combat/tool path if one is ready;
otherwise include one deliberately bounded hunting implement and its authored
item/projectile presentation in this tactical, or stop the chapter at
tracking/approach and leave the hunting acceptance box unchecked. This
decision must be recorded before implementation of damage starts.

Killing a durable deer must never cause the same seed-authored identity to
return. Record local depletion in a compact population/history owner distinct
from visible entity records, then allow only explicit, slow recolonization
under habitat and cap rules. Tests must separate “this deer is dead” from “the
habitat may eventually support another deer.” This closes the existing
death/removal persistence gap rather than adding a deer exception.

## Discovery and Feedback

Generalize mallard-only observation plumbing only as far as a second species
requires. Deer observations should include at least seen, track, bed/sign,
alert/flee behavior, shed antler, and harvest. Keep them idempotent and
per-player, persist them through the ordinary player record, and render them
through the shared flat/stereo/multiview field-note surface.

Add restrained spatial deer sounds—contact, alarm/snort, and movement/impact
only where they convey real state. Source them through the first-party sound
pipeline with explicit provenance. Flock/herd suppression and finite range
remain required; an ambient loop must not imply deer where no authoritative
deer or explicit ambient population layer exists.

## Playable Showcase

After ordinary world instantiation is proven, add a `deer-forest-edge`
showcase recipe as a transient tiny save. It may author a roomy multi-chunk
forest edge, clearing, water access, several deer, sign, and review timing, but
may not implement deer behavior, drops, animation switching, habitat queries,
or prop rendering.

Register live-instantiation evidence for every showcased subject and mechanic.
The behavioral probe must observe domain outcomes over time, including:

- meaningful displacement without stationary yaw jitter;
- at least one relaxed-to-alert-to-flee sequence;
- one complete lie-down/bedded/stand-up sequence;
- named clip and state-epoch changes matching those behaviors;
- at least one real track/sign or antler outcome;
- an ordinary validated harvest if the hunting gate is completed; and
- zero records in every browser persistent-world store.

Capture and inspect the same recipe through native flat, synthetic stereo,
local headed WebGPU, and the deployed public URL. A single still image does not
prove behavior. The final handoff includes a clean playable URL and a screenshot
from that exact pushed/deployed revision.

## Implementation Order

1. **Record baselines.** Capture the current deer source, clips, generated
   habitat distribution, mallard procedural-prop pixels, runtime actor/prop
   counts, and affected performance lanes.
2. **Generalize promotion.** Add explicit actor/world/item/trace use to the
   first-party semantic promotion path, checked generated outputs, Rust asset
   inventory/lookup, and review diagnostics without changing gameplay.
3. **Prove semantic props.** Author, review, promote, and render the mallard
   nest first. Migrate feather and tracks, compare pixels/behavior, then delete
   their superseded procedural geometry.
4. **Generalize runtime animation.** Land named clip selection, distance and
   elapsed-time phase sources, authoritative epoch continuity, fallback/error
   policy, and mono/stereo/multiview tests using an existing promoted figure
   before deer depends on it.
5. **Finish the deer asset.** Refine articulation, author all required clips
   and explicit classification, run semantic/native comparison and animation
   review, then promote checked deer JSON into the first-party pack.
6. **Land forest-edge habitat.** Add generated clearing/edge semantics, live
   fitness sampling, deterministic distribution evidence, natural group spawn,
   immediate durability, and hydration.
7. **Land behavior and sign.** Add forage, drink, bed, alert/flee, herd, track,
   bed/sign, and antler state using shared AI, prop, sound, replication, and
   persistence owners.
8. **Land honest hunting.** Resolve the normal-tool decision, add shared
   entity attack/health/death/drop behavior, species harvest, non-resurrection,
   and slow recolonization evidence. Leave this step explicitly incomplete if
   no acceptable ordinary hunting interaction is delivered.
9. **Land discovery and showcase.** Add deer observations, field-note
   presentation, a data recipe with live evidence, timed behavior gates,
   captures, deployment, and exact-revision public verification.
10. **Close documentation.** Update the living ecology, semantic asset,
    animation, catalogue, sound, platform, and showcase records with actual
    contracts, evidence, known gaps, and the next creature/terrain capability.

Each numbered step is a logical commit boundary when it stands independently.
Do not combine an unreviewed asset rewrite, protocol revision, AI behavior, and
world-generation change into one opaque commit.

## Acceptance Gates

### Asset and toolchain

- Strict Asset Lab typecheck and discovered semantic tests pass.
- First-party write/check commands deterministically reproduce every promoted
  actor and prop; stale, missing, orphaned, wrong-use, and wrong-anchor assets
  fail clearly.
- Creature Catalogue counts and filters exclude props while Asset Lab review
  can deliberately select every promoted prop.
- Deer and prop clean sheets, action sheets, videos, semantic/prepared
  comparisons, grounding, geometry, surface, and pack-drift checks pass and
  are visually inspected.
- No authored runtime JSON, procedural replacement mesh, or app-local asset
  list remains.

### Simulation, persistence, and protocol

- Habitat/spawn tests cover edge fitness, exclusions, group bounds, caps,
  profile isolation, entity-load gating, and durable versus transient stores.
- Deterministic long-running behavior tests cover retained destinations,
  graze/drink/bed timing, alert hysteresis, useful flight, herd spacing,
  bounded turning, and stationary-yaw stability.
- Snapshot/update codecs round-trip deer species, behavior, animation epoch,
  health, sign, and collection state. The client applies incremental state;
  reconnect/hydration poses do not restart actions.
- Unload/reload and full reopen retain stable deer identities and durable
  state. Harvested deer stay dead, drops do not duplicate, and later
  recolonization creates a different identity under explicit rules.
- The mallard nest, feather, and track retain their existing gameplay and
  persistence semantics after their visual migration.

### Rendering and platform boundaries

- Named distance- and elapsed-time clips render in shared prepared geometry
  with stable identity and no whole-crowd topology rebuild.
- World, item, and trace props render through ordinary shared prepared paths;
  no species-specific mesh function is added.
- First drawable milestones are captured and inspected before later behavior
  is stacked on top.
- Native mono/offscreen, synthetic stereo, full-frame multiview contract,
  browser WebGPU, flat Android build/smoke, and affected XR build/validation
  gates follow `docs/platforms.md`. Any capability skip is recorded honestly.
- Actor/prop counts, CPU pose cost, uploads, draw batching, and memory remain
  within the pre-change envelopes or have an explicit measured acceptance.

### Product acceptance

- A fresh ordinary Mclone world naturally yields deer only in meaningful
  forest-edge habitat with debug/showcase injection disabled.
- Players can read deer presence from behavior and sign, approach/stalk them,
  see full authored bedding and flight sequences, and collect at least one
  ordinary deer-origin resource.
- Hunting is claimed only if the normal validated attack/tool, damage, death,
  drops, depletion, and persistence loop passes end to end.
- The final transient showcase proves ordinary mechanics, leaves persistent
  stores empty, and matches the deployed screenshot/URL at the pushed revision.

## Guardrails and Explicit Non-Goals

- Do not put gameplay, clip selection, asset parsing, or prop geometry into app
  crates, React, browser TypeScript, Android glue, or OpenXR glue.
- Do not make Asset Lab or Creature Lab a game runtime. They author and review
  data; the authoritative Rust server owns behavior and state.
- Do not add showcase-only AI, drops, timing shortcuts, scripted paths, or
  renderer poses. Showcase recipes may only instantiate registered ordinary
  content and starting state.
- Do not create one Rust mesh builder per prop or species. The point of the
  mallard migration is to make that pattern unnecessary.
- Do not let visual bounds define authoritative collision, reach, habitat, or
  persistence.
- Do not send platform time or renderer-local completion back as authoritative
  behavior. Server tick/epoch state remains the continuity source.
- Do not add general skeletal skinning, glTF import, arbitrary triangle meshes,
  runtime IK, crossfade graphs, or a browser asset editor.
- Do not implement seasons, rut, pregnancy, disease, predators, scent-fluid
  simulation, full population genetics, cooking, crafting, hunger, armor, or a
  general combat overhaul in this tactical.
- Do not change Java 1.17.1 reference-world spawning or claim parity from this
  original Mclone ecology.
- Do not alter shipped-compatibility assumptions without first updating the
  world-generation compatibility safety ledger. Current internal Mclone
  terrain may evolve intentionally, but fixtures and fingerprints must move
  with it.

## Documentation Map

- [`../topics/habitat-driven-creature-ecology.md`](../topics/habitat-driven-creature-ecology.md):
  terrain/creature mechanics, durability, population, and species admission
- [`../topics/figure-animation-actions.md`](../topics/figure-animation-actions.md):
  named clip roles, one-shot completion, and runtime playback gap
- [`../topics/compiled-figure-rendering.md`](../topics/compiled-figure-rendering.md):
  semantic JSON, prepared geometry, instancing, and render invariants
- [`../topics/animal-catalogue.md`](../topics/animal-catalogue.md): Creature
  Catalogue product/filter contract
- [`../topics/playable-showcases.md`](../topics/playable-showcases.md): bounded
  tiny-save review and live-instantiation evidence
- [`275-first-party-sound-effects.md`](275-first-party-sound-effects.md):
  distributable sound sourcing, preparation, and playback execution record
- [`../creatures.md`](../creatures.md): Java spawning reference and ordinary
  entity lifecycle background
- `tools/asset-lab/`: canonical TypeScript sources, semantic validation,
  sheets/videos/comparison, promotion, and catalogue generation
- `native/crates/mclone-assets/`: runtime semantic loading and preparation
- `native/crates/mclone-render-session/`: gameplay presentation to prepared
  actor/prop and named-clip mapping
- `native/crates/mclone-render/`: shared mono/stereo/multiview prepared drawing
- `native/crates/mclone-server/`: authoritative habitat, spawning, AI, damage,
  drops, population, sign, and persistence

## Completion Record

### 2026-08-12 — Review 1 candidate: semantic mallard props

- Generalized checked first-party promotion from actor-only records to
  explicit `actor`, `world_prop`, `item_prop`, and `trace_prop` use with exact
  `feet`, `ground`, `item_center`, and `surface_trace` anchors. Incompatible
  use/anchor pairs and unregistered `props/*/figure.ts` sources fail catalogue
  generation.
- Promotion separately records `live_gameplay` versus `review_only`
  instantiation. All three candidates are visibly and machine-readably
  `review_only`; packing them is not treated as evidence that the live game
  creates them.
- Authored static canonical `mallard_nest`, `mallard_feather`, and
  `mallard_tracks` sources. Generated runtime JSON is checked for drift and
  classified as `SemanticProp` in the first-party Rust inventory; it is not
  admitted to the live actor registry.
- Added a bounded `?view=props` mode to the existing Asset Lab deployment.
  Default `/animals/` counts and filters remain the 202-figure creature
  catalogue. The prop mode shows only three checked assets and supports static
  sources without fake animation clips.
- The semantic and native preparation gates pass: 32 Asset Lab tests, 205
  discovered canonical sources across actor and prop validation, one reasoned
  disconnected track-pair component, six exact joined-imprint surface
  exceptions, 76 `mclone-assets` unit tests after the prop preparation proof,
  and five browser catalogue/review tests. First-party authored, fallback, and
  diagnostic packs stage successfully.
- Two visual iterations were inspected. The first nest read as a rectangular
  tray and the first heel pads were oversized; the candidate uses an octagonal
  woven rim and compact heel marks. The feather retains a strong blue
  speculum for item-scale recognition.
- This is intentionally a review stop. Live mallard nest, feather, and track
  visuals still use the accepted procedural meshes. No gameplay, persistence,
  collection, observation, or creature-instantiation behavior changed. After
  human acceptance, the next slice may replace those visuals through the
  shared prepared prop path and must then delete the superseded procedural
  builders.

For later slices, continue to record commit IDs, asset review paths,
deterministic habitat/population measurements, focused and workspace tests,
native/browser/Android/XR evidence, rejected visual or interaction iterations,
the deployed revision and URL, remaining gaps, and the next terrain/creature
capability selected by this chapter.
