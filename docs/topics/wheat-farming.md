# Wheat Farming

Topic: `wheat-farming`

Status: **live foundation plus world-drop and object-level visual correction
deployed and accepted under Tactical
[`290`](../tactical/290-wheat-world-drops-and-visual-language.md) on
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
seeds on the new farmland. A successful till already in that water range
immediately creates moisture-7 farmland so field creation is readable; dry
tilling creates moisture 0. This is a narrow responsiveness divergence from
Java 1.17.1's delayed first hydration. Loaded block-ticking chunks sample three
random positions per non-empty section. Water hydrates farmland toward
moisture 7;
unwatered soil dries toward 0 and eventually returns to dirt if no crop
protects it. Wheat grows from age 0 to 7 when light and the surrounding
farmland/crop layout pass the reference-shaped growth calculation.

Breaking immature wheat removes the crop and spawns one seed as an ordinary
item entity. Breaking mature wheat removes the crop and spawns one wheat plus
the no-Fortune reference seed roll. The shared item lifecycle owns motion,
pickup delay, merging, tracking, persistence, capacity-aware collection, and
inventory publication. A full inventory therefore leaves loot in the world
instead of preventing harvest. Removing farmland support cleans up the crop.
Tilling, planting, growth results, moisture, harvest output, remaining seeds,
and the selected hotbar slot all use ordinary authoritative state and save
records.

The first-party Texture Lab source owns dry and hydrated farmland plus eight
visually distinct green-to-gold crop stages. Farmland has its 15/16-height
support shape; wheat is cutout, selectable by age-relative height, and has no
collision. Age zero also reuses the active pack's green crop pixels on a small
top-readable leaf surface, so planting reads from the normal steep view without
changing selection or collision. `WoodenHoe`, `WheatSeeds`, and `Wheat` travel
through the shared protocol, persistence, inventory, item-entity, HUD, and
item-label paths. World wheat and seed entities use dedicated checked Asset Lab
props rather than the egg compatibility mesh. Crop-state target labels remain
available behind debug diagnostics but are absent from the ordinary HUD;
object geometry and world loot are the primary language. Confirmed hoe and
seed mutations use the normal interaction sound path.

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
289 owns the correction and supersedes revision 1 as the accepted manual
farming evidence.

Corrective commits `0d1b23b1` and `8e6cab39` provide immediate wet tilling,
confirmed hoe/seed sound, a brighter proprietary-free age-zero rosette, and
replicated crop target labels. Revision 2 keeps seed `17506` and entry eye
`8.5,65.62,14.5`, but targets a reachable mature crop at
`9.5,64.5,10.5`. Local desktop and phone gates harvested state 236 at
`9,64,10` from the untouched entry camera before using the smoke-only frame
helper for the separate till/plant proof. Both then tilled the water-adjacent
cell directly to moisture 7, planted wheat through real controls, observed an
automatic transition, and retained zero browser records.

The inspected local initial desktop and phone digests are
`fd8510a51ef69a8b522e31f1a1d5caa5c562c1b0088fbca53000a51f46a2eb07`
and
`c09833fac7bb6ca09496383cd1ac1cc58dcbc08158d3e7aa21033011f357ef74`;
the just-planted captures are
`50a1b89e780251b95de52d701a7d16e3178e14dae2a13a37386a3bd3f9b0a017`
and
`be303f33a85d30dde3a4c5ae8a770132100461491abb27b85b6b0b76705688eb`.
Full server validation now passes 670 tests. Exact pushed revision
`924edf10a706b43a1965413e359965292eac91a6` deployed as Worker version
`a917fa06-dbc2-4ddc-ac55-04963368e209`. Public desktop and phone gates repeated
the untouched-camera harvest, immediate hydrated till, planted-sprout
feedback, automatic transition, inventory, and zero-storage receipts. Their
initial frames are byte-identical to the local digests above; the public
desktop planted frame is likewise byte-identical. The inspected public phone
planted frame is
`175d4502904c2be544afb2800790bf56d240236b287b3c892872994382f9b47c`.

Human review then rejected revision 2's harvest and planting language. Harvest
appeared not to remove the crop and offered no collectible world loot like a
hunted deer's drops. The target text was useful as optional diagnostics but
had become required to understand the crop, while the newly planted pixels
still did not visibly contain green from the player's placement view.
Tactical 290 withdraws that usability acceptance.

Commits `040b2ffa` and `a6729567` implement the local correction. Revision 3
targets isolated mature crop `11,64,10`. Harvest removes it to air and spawns a
dedicated wheat-sheaf prop plus any seed-pouch drops through generic item
entities; the player's inventory changes only after walking into them. A full
inventory leaves those drops in the world. Age-zero wheat gains a clearly
green top-readable leaf surface, and crop target labels are disabled in the
ordinary HUD.

The correction also fixes a general browser live-edit defect. Client section
deltas now advance the resident snapshot content revision, causing the Web
render-worker mirror to upsert changed columns before compiling. This is why
both the removed Y=64 crop and newly added Y=64 sprout now produce current
pixels rather than stale section-boundary geometry.

Local desktop and phone gates exercise real attack, movement, pickup, till,
and plant controls and retain zero browser world records. Inspected harvested
frames are
`b43edcee22b7aa9e7083955f748e0d60ff9d960fdeab012fac48b89365b76984`
and
`d7e62749744f345cc1935918d77d611155cac45be1a5901e9bd73df916e65ae3`;
inspected planted frames are
`01c66226e3b29d96985b2b6b378f22ad09781c71cd43393093d2c912c1c78a66`
and
`c17abedc91702af4fcea639bf7dbf5c200fa48b7ac5db657c2aee533c87e488f`.
Full affected shared suites pass. Exact pushed revision
`0f746896545ee3bc9ce446257d656b5ab28fbffe` deployed as asset version
`0f746896545e-20260813081257` and Worker version
`a2306ede-be7a-4056-9b7c-73a2a1b9a89c`.

Public desktop and phone gates reproduced the same revision, seed, target,
world-drop counts, pre-pickup inventory, `4.14`-block movement pickup, till,
plant, debug-label, and zero-storage receipts. Their initial frames are
byte-identical to local. Inspected public harvested frames are
`a93ea616dee3086edffd2c6cdf20e3cef860068f989021f903e447017f609934`
and
`1899ec9b17b88877354311bcb4875d6117263d9c71f81e754633caf955274518`;
planted frames are
`9637b7ec8be7f7daee10b67d0644a2cad3bef3efd626460d8ebc695582b2aacf`
and
`af7a0cd01f76ee8eb0a05abff7f7f4b40dae0ae30c863e1a63a7602f58a63172`.
The existing clean public field URL remains fresh and non-persistent.
