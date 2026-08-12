# Tactical 285: Bee Pollination Ecology

Status: **active 2026-08-12.**

Topics:

- `habitat-driven-creature-ecology`
- `semantic-figure-assets`
- `playable-showcases`
- `first-party-sound-effects`

## Instruction Synthesis

Continue the terrain/creature/gameplay workstream with bees. Deliver the
chapter end to end, using tactical documentation, reviewable commits, ordinary
live-world instantiation, a useful player mechanic, durable persistence,
inspected rendered evidence, and a deployed phone/desktop playable showcase.
The result must extend terrain and gameplay vocabulary rather than merely add
another randomly spawning Creature Lab model.

## Objective

Make bees the third complete original ecology chapter:

```text
generated flowering habitat
  -> a durable colony site and individually durable bees
  -> visible flower-forage-return flights
  -> stored colony work and bounded pollinated flower spread
  -> player-provided nesting habitat and a collectible colony resource
  -> field observations that teach the loop
  -> a larger flowering landscape that can support later creatures and crops
```

This chapter should force a useful shared habitat abstraction, true
non-grounded creature motion, terrain change caused by animals, entity use,
and managed habitat placement. It must remain honest about the systems that do
not exist yet: pollinating real flowers is in scope; promising improved crop
yield before authoritative crop growth exists is not.

## Baseline and Decisions

- Creature Lab already contains an approved box-authored worker bee. Its one
  clip conflates hovering and travel, so it needs explicit hover, flight, and
  forage presentation before runtime promotion.
- Mclone generation already places dandelions and poppies, but the distribution
  reads as sparse decoration rather than a pollinator habitat. This slice may
  alter the internal, unshipped Mclone profile; it must not alter the
  Java-1.17.1 reference-locked Overworld output.
- Wetland and forest-edge ecology each own a block-derived habitat record. The
  third chapter will introduce one shared fitness vocabulary rather than add a
  third incomparable score.
- Visible bees are authoritative entities and become durable immediately upon
  materialization. The colony site is durable too. There is no unload-time
  ambient swarm pretending to be the same individuals.
- A colony site is an entity-backed semantic world prop, not a fake block or a
  showcase-only ornament. Natural colonies and player-provided bee hotels use
  the same authoritative occupancy and forage loop while retaining their
  distinct origin.
- Pollination changes real world state only after a successful
  flower-to-colony trip. Spread is deterministic, local, cooldown-bounded, and
  restricted to valid replaceable air above compatible ground.
- The first resource is beeswax. It is collected from a sufficiently worked
  colony through an authoritative entity-use command. Honey, smoke, defensive
  stings, breeding/genetics, recipes, crop yield, and seasonal colony health
  remain later mechanics.

## Habitat Contract

Add a reusable `HabitatFitness` record above raw block and biome identifiers.
It should expose named dimensions useful to multiple species: food/forage,
shelter, substrate, open movement space, terrain continuity, and an overall
bounded score. Wetland and forest-edge sampling should adopt the common record
where those dimensions are truthful; species-specific samples retain their
facts and thresholds.

`FloweringHabitatSample` will read currently published block state and fail
closed on missing terrain. It measures:

- actual flower count and local clustering;
- grass or other supported planting substrate;
- woody or sheltered colony-site opportunity;
- navigable air volume and vertical clearance;
- bounded surface variation; and
- the shared fitness dimensions and total.

Ordinary edits therefore matter automatically: removing flowers reduces
forage, planting or bee-driven flower spread improves it, and blocking the air
or removing support can invalidate a site.

Mclone decoration should create occasional coherent wildflower pockets through
its existing production vegetation fields and deterministic domains. The
habitat sampler must consume the generated blocks, not a hidden bee-only noise
field. Reference Overworld configured features remain untouched.

## Entity and Behavior Contract

Add protocol/runtime kinds for `Bee` and `BeeNest`. A bee persists:

- stable colony-site identity;
- current behavior and behavior time;
- whether it carries pollen;
- health and presentation state needed across hydration; and
- any cooldown whose reset would duplicate a durable result.

A colony persists its wild/managed origin, stored work, resource readiness,
flower-spread cooldown, and stable identity. It does not save a second list of
ghost residents: active bees remain the ordinary individual entity records.

The first behavior machine is:

```text
hover at or near colony
  -> select a real nearby flower
  -> fly through free 3D space
  -> visibly forage at that flower
  -> carry pollen home
  -> deposit at the same durable colony
  -> resume
```

Movement must be collision-aware, bounded in all three axes, and orient from
realized horizontal displacement. Arrival may not cause rapid stationary yaw
changes. The server replicates semantic behavior and pollen state; clients
select reviewed named clips without inventing simulation state.

Natural spawning creates a small colony only in a high-fitness flowering
site, under the shared passive-creature cap and existing persistence/load
gates. It must reject an already occupied neighborhood. An empty managed hotel
in suitable loaded habitat can be colonized by the same live spawning path.

## Player Loop

Add a semantic bee-hotel item/world prop and a semantic beeswax item prop.
New-player starter inventory may provide one bee hotel until crafting has an
authoritative owner. Using it on valid ground places one durable managed colony
site and consumes the item. Placement must enforce support, clearance,
distance from another colony, and flowering habitat rather than act as an
unconditional creature dispenser.

Using a worked colony through a general authoritative entity-interaction
command yields one collectible beeswax item and resets only the collected
resource threshold. Reach, line of sight, identity, and cadence are validated
server-side. An immature colony remains unchanged.

Six persisted observations should teach the system without prose screens:

1. see a bee;
2. find a colony site;
3. witness flower foraging;
4. witness a pollen-bearing return;
5. witness a new pollinated flower; and
6. collect beeswax.

The shared field-note HUD continues to show concise progress. These facts are
ordinary player records, not showcase awards.

## Assets, Animation, and Sound

Refine the checked bee source into reviewed named clips:

- `hover`: elapsed-time loop with rapid wing motion and small body drift;
- `fly`: distance-driven locomotion loop with stronger pitch/bob; and
- `forage`: action loop or bounded action with a flower-facing dip and wing
  modulation.

Author `bee_nest`, `bee_hotel`, and `beeswax` through the generalized Asset Lab
semantic-prop pipeline. A natural site should read as a sheltered wild cavity
or comb-bearing log; the managed hotel must read as deliberate player-made
habitat. Promote all four assets to `live_gameplay` only with ordinary
instantiation and required runtime loading in the same series.

Add a small first-party buzz family through the provenance-locked audio bank.
Buzz events originate from real bees, use bounded cadence and local-colony
suppression, and carry spatial position/range. They are evidence of present
creatures, not an ambient biome soundtrack.

## Showcase Contract

Add a `bee-pollination` data recipe within the existing tiny-save schema. It
may compose a flowering fixture, a natural colony, three bees in different
parts of the loop, one carried bee hotel, and a bounded initial observation
set. It may not script movement, pollination, colony filling, harvesting, or
field-note advancement.

Every gameplay fact requires typed live-instantiation evidence for ordinary
world generation, spawning, placement, behavior, interaction, or discovery.
After hydration the world is the same integrated simulation as any other
world, remains entirely in memory in the browser, and leaves every persistent
browser store empty.

Acceptance requires:

- a native first frame with readable bee, flowers, colony, and entry pose;
- a timed authoritative window in which multiple bees move, at least one
  reaches forage/return state, presentation motion advances, and pollination
  or colony work changes real state;
- local headed-WebGPU desktop and mobile proofs, including a usable touch
  hotbar and `USE` path for the bee hotel/colony interaction;
- exact deployed-revision proof at
  `https://mclone.kzahel.com/app.html?showcase=bee-pollination`; and
- inspected screenshots whose scene and seed match the shared URL.

## Implementation Slices

1. **Record the design.** Add this tactical and index it.
2. **Review assets early.** Refine the bee clips, author the three semantic
   props, regenerate canonical JSON, render still/sheet evidence, and inspect
   the pixels before runtime promotion.
3. **Add shared types and presentation.** Extend protocol, asset loading,
   actor composition, clip selection, and flat/stereo/multiview paths.
4. **Generalize habitat and generation.** Introduce shared fitness, adapt the
   existing habitats, and add bounded Mclone wildflower pockets.
5. **Implement colonies and flight.** Add natural/managed colony state,
   persistent bee AI, collision-aware 3D travel, authoritative cues, and
   bounded flower spread.
6. **Close the player loop.** Add entity use, hotel placement, beeswax pickup,
   field observations, and spatial buzz cues.
7. **Prove ordinary instantiation.** Cover natural colony creation, managed
   colonization, persistence/hydration, habitat edits, pollination, collection,
   and player progress with deterministic tests.
8. **Build the showcase.** Add typed evidence and the data recipe, then capture
   native and local Web desktop/mobile behavior.
9. **Close and deploy.** Update the ecology, showcase, asset/audio, and
   platform docs; push, deploy the exact revision, rerun the public gates, and
   share the inspected screenshot and transient play URL.

## Human Review Points

- **Review A — visual language:** bee hover/fly/forage sheets and the natural
  nest, managed hotel, and wax props before they become runtime requirements.
- **Review B — behavioral legibility:** an early real-world capture/video
  showing useful three-dimensional movement, flower approach, and return. A
  bee that only jitters, spins, or hovers in one cell is a failed gate.
- **Review C — final interactive scene:** hosted desktop and phone review with
  visible flowers, colony state change, the ordinary item/control surface, and
  the same seed/camera as the shared screenshot.

The currently authorized autonomous run may inspect and pass these gates
without pausing. It must stop and report rather than paper over a substantive
failure.

## Validation

At minimum:

- Asset Lab generation, semantic validation, render sheets, and required
  runtime resource tests;
- protocol codec and client incremental-replica coverage;
- habitat/generation determinism and Java-reference non-regression;
- server spawn, flight, colony, persistence, interaction, pollination, sound,
  and observation tests;
- shared render-session flat, stereo, and multiview tests;
- relevant workspace tests and `cargo fmt --all --check`;
- native flat/stereo capture inspected at the first drawable milestone;
- local headed-WebGPU desktop and mobile timed showcase gates;
- pushed/deployed headed-WebGPU desktop and mobile gates with zero browser
  persistence records; and
- updated living topics and this execution record with commands, receipts,
  image hashes, deployed revision, and remaining gaps.

## Explicit Deferrals

- crop growth and pollination yield;
- queen genetics, breeding, swarming, seasons, disease, and unloaded colony
  population summaries;
- honey, smoking/protective equipment, defensive aggro, stings, and player
  damage;
- recipes or a full crafting source for bee hotels and wax;
- transparent-wing material upgrades beyond the current semantic figure
  material contract; and
- a public showcase catalogue or permanent hosted save.

These are future chapters, not hidden requirements for calling the first
flowering-colony loop complete.
