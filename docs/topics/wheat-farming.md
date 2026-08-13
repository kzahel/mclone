# Wheat Farming

Topic: `wheat-farming`

Status: **live foundation deployed under Tactical
[`288`](../tactical/288-wheat-farming-foundation.md); its manual-interaction
acceptance was rejected and corrective Tactical
[`289`](../tactical/289-wheat-farming-feedback-and-harvest.md) is active
2026-08-13.**

## Purpose

This topic owns the first shared crop loop: player-created farmland, hydration,
random-tick growth, wheat harvest and renewal, durable field state, and the
ordinary-world sources used by any focused farming showcase. It should become
the reusable substrate for more crops, crop-aware pollination, farmstead
gardens, generated working fields, animal feed, and later food/crafting loops.

The governing direction is mechanics first. A field is not merely a patch of
crop textures placed by a fixture or generator; it is terrain the player can
transform and maintain through tools, water, seed, elapsed loaded-world time,
and harvest.

## Live Contract

The live shared implementation follows the useful Minecraft Java 1.17.1
semantic shape: eight moisture states, eight visible wheat ages, four-block
irrigation reach, vanilla-shaped crop growth speed, mature and immature loot,
and loaded-chunk random ticking. The resulting block states and inventory are
ordinary persisted records.

The player creates a field by using a wooden hoe on grass or dirt with clear
air above, placing water within four horizontal blocks, and planting wheat
seeds on the new farmland. Loaded block-ticking chunks sample three random
positions per non-empty section. Water hydrates farmland toward moisture 7;
unwatered soil dries toward 0 and eventually returns to dirt if no crop
protects it. Wheat grows from age 0 to 7 when light and the surrounding
farmland/crop layout pass the reference-shaped growth calculation.

Breaking immature wheat returns one seed. Breaking mature wheat adds one wheat
and the no-Fortune reference seed roll atomically to the real inventory; a full
inventory leaves the crop intact. Removing farmland support cleans up the crop.
Tilling, planting, growth results, moisture, harvest output, remaining seeds,
and the selected hotbar slot all use ordinary authoritative state and save
records.

The first-party Texture Lab source owns dry and hydrated farmland plus eight
visually distinct green-to-gold crop stages. Farmland has its 15/16-height
support shape; wheat is cutout, selectable by age-relative height, and has no
collision. `WoodenHoe`, `WheatSeeds`, and `Wheat` travel through the shared
protocol, persistence, inventory, item-entity, HUD, and item-label paths.

The compact runtime terrain-state lane is still an interim identity map backed
by `u8` raw IDs. This slice can represent the sixteen required states, but it
leaves little remaining ID space. Do not compress future crop state or alias
distinct mechanics to evade that limit; widen or replace the interim lane when
the next content family needs it.

## Deliberate Gaps

- tool durability and general item components;
- rain hydration and farmland trampling;
- bone meal and enchantment-aware loot;
- grass/fern seed drops, crafting, bread, hunger, and cooking;
- bee-to-crop pollination effects;
- additional crops, crop genetics, seasons, pests, and soil fertility;
- villager farming and generated village/farmstead crop parcels; and
- unloaded-time catch-up.

These must consume the ordinary mechanics recorded here. They may not be
implemented only inside a showcase or authored settlement.

## Current Evidence

The `wheat-farming` revision-1 showcase is a data-only tiny save with seed
`17506`, entry eye `8.5,65.62,14.5`, and target `8,64.4,6`. It composes a water
trench, dry/wet soil, every visible growth phase, a mature harvest target, one
hoe, and eight seeds. Typed evidence binds each fact to the ordinary till,
plant, random-tick growth, starter inventory, and harvest producers.

Local headed WebGPU desktop and 390x844 phone gates both:

- observed an automatic moisture or crop-state transition;
- tilled ordinary grass through the authoritative item-use path;
- planted age-0 wheat and recorded seeds changing exactly `8 -> 7`;
- harvested mature wheat and recorded wheat `0 -> 1` and seeds `7 -> 9`;
- used real keyboard or rendered touch hotbar/action controls respectively;
  and
- retained zero records in all eight browser world stores.

The inspected desktop and phone capture SHA-256 digests are
`e4093132d9ab1d0b63c4bf3869533c80a9cb29b5e1af6adccfa3b9a2c6f17e8d`
and
`53b55792adccd5b8f2761674bad379d09121d0d31f59ddaca6355cc717058d44`.
The native capture digest is
`a6deb1d4d576784d75f8dc81120da1e7a08c07cb237def27797f75bcf8068504`;
the inspected Texture Lab contact sheet digest is
`22e36db683b32203f3c5212dae9391beb556bb9bf3b40a304ccbe04db09904ef`.

Focused authoritative tests cover tilling, planting, protected-world denial,
hydration, drying, growth, support cleanup, mature/immature renewal, bounded
order-independent random candidates, atomic inventory capacity, and SQLite
restart. The full server suite passes 669 tests. The full worldgen suite passes
414 tests with one intentional ignore and pins the already-live post-bee
wildflower output; wheat adds no generated crop fields or Overworld mutation.

Exact pushed implementation revision
`250edd201dc16b069d4d7033823d3625b69c0a7f` deployed as Cloudflare Worker
version `0041ab1e-5dd7-492d-8dbc-d899be35451d`. Public desktop and phone gates
reproduced the same seed, revision, camera, automatic transition, interaction
counts, inventory changes, and zero-store result. Their inspected images are
byte-identical to the local captures at the digests above.

The resettable public field is:

```text
https://mclone.kzahel.com/app.html?showcase=wheat-farming
```

Human review then found that adjacent-water hydration was too delayed to read,
the age-0 sprout was nearly invisible, and the initial camera did not make a
mature harvest target discoverable. The automated gate's smoke-only framing
proved authoritative mechanics but masked those usability failures. Tactical
289 owns the correction; until it closes, the revision-1 public URL remains a
mechanics fixture rather than accepted manual farming evidence.
