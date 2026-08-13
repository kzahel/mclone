# Tactical 292: Rabbit Burrow Ecology

Status: complete 2026-08-13

Topics:

- `rabbit-burrow-ecology`
- `habitat-driven-creature-ecology`
- `functional-kitchen-gardens`
- `semantic-figure-assets`
- `playable-showcases`
- `first-party-sound-effects`

## Instruction Synthesis

Continue the terrain, creature, and garden workstream with rabbits. Build the
recommended hybrid burrow end to end: a rabbit visibly excavates a genuine
shallow terrain threshold, while the inaccessible warren beyond that mouth is
compact persistent simulation state rather than an early tunnel engine.
Rabbits must emerge, forage, raid real carrots, respect real fences and gates,
retreat, breed from player-provided carrots, persist as families, and teach the
loop through ordinary field observations. Author and inspect the creature and
burrow assets, instantiate the ecology in normal generated worlds, package the
same systems in a bounded showcase, commit logical slices, and finish with an
exact deployed desktop/phone review URL and matching inspected pixels.

## Objective

Make rabbits the first creature chapter that turns a player-built garden into
contested habitat and makes a creature-created home part of terrain:

```text
dry sheltered bank + nearby forage
  -> a founder visibly digs one shallow entrance cell
  -> a durable burrow mouth owns compact warren state
  -> persistent rabbits emerge at active periods and forage
  -> carrots attract, feed, and breed them
  -> reachable mature carrot crops invite bounded raids
  -> closed fences exclude rabbits; open gates become opportunities
  -> danger sends family members to a real home entrance
  -> kits and field observations make stewardship legible
```

This is not a rabbit skin, a decorative hole, or a showcase scenario. It is a
shared ecology/gameplay chapter whose ordinary producers happen to be composed
into a tiny review save after they work in the generated game.

## Reference and Deliberate Extension

Minecraft Java 1.17.1 `Rabbit` is the behavioral reference for hop cadence,
carrot and dandelion temptation, avoidance distances and speeds, breeding,
biome-influenced variants, persistence, and `RaidGardenGoal`. In particular,
vanilla raids only mature carrots, removes an age-zero crop or decrements an
older age by one, and imposes a bounded raid cooldown.

Persistent warrens and terrain excavation are original Mclone mechanics. They
are deliberately layered around rather than hidden inside the vanilla-shaped
rabbit behavior. The extension should preserve obvious future parity work:
the species module keeps hop, temptation, flight, breeding, and crop-raid
decisions recognizable, while the separate burrow/home contract can be
disabled or replaced by a profile that wants strict reference behavior.

## Selected Burrow Model

### A real threshold, not a tunnel engine

A completed dig changes exactly one validated terrain cell from dirt or grass
to air. That cell must have:

- solid dry support below;
- a solid dirt/grass rear and roof, forming a bank rather than a pit;
- traversable air in front and enough local clearance for the rabbit;
- no water in or immediately flooding the entrance;
- nearby forage or cover; and
- no competing burrow mouth within the local family radius.

The durable `RabbitBurrow` entity is then grounded inside the excavated cell.
Its authored semantic figure supplies the dark inset, irregular soil lip,
small roots, and recessed visual threshold. The missing block supplies honest
depth, occlusion, and terrain integration. Neither the figure nor a fake dark
quad pretends that a full navigable tunnel exists.

Players cannot crawl into the warren in this chapter. The entrance remains a
shallow one-block recess and the dark inset closes its back. No general cave
carving, tunnel graph, underground camera, rabbit-sized player pose, or voxel
collapse simulation belongs in this tactical.

### Compact persistent warren state

The mouth owns stable persistence identity, orientation, capacity, disturbance
or safety state, and the stable identities of resident rabbits. Rabbits persist
their home identity, parents, life stage/growth, current behavior epoch,
underground state, health, love/breeding cooldown, raid cooldown, and other
state whose reset would duplicate or erase a durable result.

An underground resident remains the same persistent rabbit. It is omitted from
visible snapshots while sheltered, not despawned and later rerolled. A loaded
home advances only bounded ordinary state. This tactical does not invent an
unloaded whole-ecosystem simulation or ghost population summary.

Destroying or invalidating the entrance releases loaded residents and clears
their home link; it must not silently delete a family. Full burrow repair,
relocation, trapping, collapse, and player-dug artificial warrens remain later
mechanics.

## Rabbit State and Behavior

Add `Rabbit` and `RabbitBurrow` shared protocol/runtime kinds. A rabbit uses
one authoritative behavior vocabulary:

| Behavior | Meaning and required outcome |
|---|---|
| `Idle` | alert pauses and ear/head scans without rapid yaw jitter |
| `Dig` | faces one validated bank cell, visibly works, then requests one authoritative excavation |
| `Emerge` | travels from the mouth into open terrain with a reviewed one-shot transition |
| `Forage` | hops among real grass/flowers or toward a tempting carried carrot |
| `Raid` | reaches a path-accessible mature carrot and performs one real bounded crop-age mutation |
| `Flee` | moves rapidly away from a nearby player/threat and toward its home when safe routing permits |
| `EnterBurrow` | reaches and lowers through its own mouth before becoming underground |
| `Underground` | remains persistent and non-rendered until an eligible emergence window |
| `Courtship` | stays near another fed adult before one durable kit birth |

Ordinary activity is biased toward dawn and dusk. Daylight and midnight may
still contain short local foraging excursions, especially under hunger or
player temptation, but must not turn every rabbit into a continuously roaming
mob. World time, not a per-entity fake clock, governs emergence opportunity.

Ground movement uses the shared collision/path queries. A closed oak fence or
gate is an exclusion boundary; an open gate is traversable. Do not special-case
the kitchen-garden recipe or teleport through blocked geometry. Failed paths
must select a new destination or retreat instead of rotating at one point.

## Garden and Breeding Loop

Carrots have three intentionally related uses:

1. a held carrot tempts a visible rabbit at a bounded range;
2. using one carried carrot on an adult consumes exactly one and sets a
   persistent love window; and
3. two compatible fed adults sharing a safe burrow may create one durable kit
   with parent identities and a breeding cooldown.

Birth is server-authoritative, capacity-bounded, and persists immediately. It
is not feed-to-clone: both adults, a compatible home, room in that warren, and
a completed courtship are required. Kits grow through persisted loaded-world
age and use a visibly smaller presentation scale.

A raid targets only real mature carrot crops that are path-accessible within a
bounded search. The rabbit removes one growth stage per completed feed, using
the reusable crop decision path and emitting the ordinary block update. At age
zero the plant disappears. Raid cooldown and hunger prevent instant field
erasure. Closed fences must protect the bed; opening the gate can expose it.
Wheat remains forage context, not interchangeable breeding food.

## Habitat and Live Instantiation

Add `RabbitHabitatSample` on top of the shared habitat-fitness vocabulary. It
reads live blocks and measures:

- dry diggable grass/dirt banks with roof/rear/support continuity;
- open ground and low slope for hopping;
- vegetation and natural forage;
- cover and distance from deep water; and
- local burrow/population occupancy.

Natural Mclone instantiation creates a small number of durable founder rabbits
only in a high-fitness, currently loaded site. A founder without a home must
select and visibly dig a valid threshold before family behavior can develop.
Generated terrain is therefore both input and eventual record: suitable banks
permit burrows, and successful rabbits alter one cell and leave a persistent
mouth.

The Java 1.17.1 reference-locked Overworld profile keeps its vanilla-targeted
spawn/output contract. The original Mclone profile owns the burrow extension.

## Assets, Animation, and Sound

Promote the existing canonical Asset Lab rabbit rather than create a separate
runtime model. Refine it with at least:

- `idle`: elapsed ear/head scan loop;
- `hop`: distance-driven grounded locomotion with a readable airborne phase;
- `flee`: faster distance-driven hop;
- `forage`: lowered head and chewing/forepaw motion;
- `dig`: repeated forepaw scrape and body pitch;
- `enter_burrow`: one-shot body-lowering transition;
- `emerge`: compatible inverse one-shot transition; and
- `courtship`: brief social loop.

Author `rabbit_burrow` as a ground-anchored semantic world prop through the
same checked TypeScript-to-JSON pipeline. Inspect a source render before live
promotion. The prop must read as an entrance at player scale from several
angles and avoid the rejected raised-track look. A future terrain-conforming
decal system may add subtle loose-earth blending, but it is not a prerequisite
and this tactical must not implement a rabbit-only decal renderer.

Use provenance-locked first-party sound families for a quiet thump/rustle and
digging scrape, preferably by assigning already bundled Kenney CC0 variants to
new semantic families rather than duplicating audio files. Producers are real
hop/dig/retreat events with bounded cadence and spatial range, not an ambient
biome loop.

## Discovery

Persist six ordinary rabbit field observations:

1. see a rabbit;
2. find a burrow mouth;
3. witness successful digging;
4. witness emergence or retreat through the entrance;
5. witness a real carrot crop raid; and
6. feed a breeding pair or witness a kit birth.

The concise field-note HUD remains optional/supportive feedback. Silhouette,
animation, terrain mutation, crop-stage change, item consumption, and the kit
must communicate the mechanics without requiring crosshair prose.

## Showcase Contract

Add a `rabbit-burrow` bounded data recipe using the existing tiny-save schema.
It may establish one shallow excavated threshold with a persistent burrow and
family, a second suitable founder/dig site, a fenced garden with a closed gate,
real mature carrots, and a carried carrot. It may preserve ordinary saved
behavior epochs, but it may not script digging, opening the gate, crop damage,
flight, emergence, feeding, courtship, or birth.

Every non-base gameplay fact declares compatible typed `liveInstantiation`
evidence whose producer is ordinary habitat spawning, excavation, garden
placement/farming, interaction, breeding, or observation. Unknown and
incompatible evidence fail validation.

Acceptance requires:

- native first-frame pixels in which the recessed mouth, rabbits, garden,
  fence, gate, and carrot bed read without labels;
- a bounded ordinary simulation window proving meaningful rabbit
  displacement, emergence/retreat or completed digging, and a path-dependent
  carrot raid without commanding the rabbits;
- an interactive keyboard and phone/touch pass covering carrot feeding and
  gate-mediated exclusion/exposure;
- exact local and deployed desktop/phone screenshots from the same recipe,
  seed, camera, and pushed revision; and
- a public fresh-world URL at
  `https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`, with every
  IndexedDB world-record store verified empty.

## Implementation Slices

1. **Record the contract.** Add this tactical, the living rabbit topic, topic
   log entry, and links from garden/ecology direction.
2. **Review assets early.** Refine the rabbit clips, author the burrow prop,
   regenerate checked semantic JSON, capture a multi-angle review, and inspect
   it before runtime promotion.
3. **Land shared types and persistence.** Add entity/species/warren records,
   replication, hydration, client state, named presentation, field notes, and
   save compatibility.
4. **Implement the ecology.** Add habitat sampling, founder instantiation,
   validated one-cell excavation, time-aware emergence, collision-aware
   movement, retreat, and family/home maintenance.
5. **Close the garden loop.** Add carrot temptation/use, pair courtship, kit
   birth/growth, crop raid mutation, fence/gate path tests, drops/feedback, and
   spatial sound cues.
6. **Prove ordinary instantiation.** Exercise natural founder-to-burrow state,
   persistence/unload/reload, garden protection, breeding, and observation in
   focused shared tests.
7. **Package review.** Add typed evidence and the data-only showcase, capture
   and inspect native pixels at the first drawable milestone, then close local
   desktop/phone behavioral and persistence gates.
8. **Deploy exactly.** Update living docs and this execution record, run the
   affected validation matrix, commit, push the tested revision, wait for the
   established deployment, verify public pixels/behavior/zero persistence,
   and share the clean URL and screenshot.

## Non-Goals

- player-walkable warrens, arbitrary tunnel graphs, cave generation, digging
  tools, collapse, traps, predators, disease, seasons, genetics, domestication,
  hunger/cooking, fence crafting, leads, or unloaded population simulation;
- a rabbit-only navigation system, terrain decal renderer, raw mesh path,
  scripted showcase behavior, or platform-local gameplay branch;
- strict vanilla rabbit output in the original Mclone profile; and
- completion claims based only on a static screenshot.

## Completion Gate

This tactical is complete only when:

- the checked rabbit and burrow sources render through the shared semantic
  asset path in mono, stereo, multiview, and Web composition;
- ordinary generated Mclone habitat can produce a durable founder, one visible
  completed excavation, and a persistent burrow/family across reload;
- rabbits visibly move, forage, retreat, enter/emerge, raid live carrots, and
  breed from consumed inventory while real fences and gates matter;
- the six field observations advance only from ordinary evidence;
- focused protocol/server/client/asset/farming/showcase tests pass along with
  workspace formatting/checks and the affected platform matrix; and
- the exact pushed/deployed revision has inspected desktop and phone pixels,
  a passing bounded behavior receipt, zero IndexedDB persistence, and a usable
  public `rabbit-burrow` URL.

## Execution Record

### Contract and reviewed semantic assets

Commit `e6921bff` recorded the selected hybrid contract and living topic.
Commit `7260474c` then promoted the canonical Asset Lab rabbit and authored the
`rabbit_burrow` world prop through the shared checked semantic-figure path. The
rabbit has named idle, hop, flee, forage, dig, entry, emergence, and courtship
clips; the burrow uses a recessed dark back, earth threshold, and roots around
the real missing terrain cell. The same slice moved carrot, fence, and gate
inventory figures into checked first-party sources.

Inspected Asset Lab sheets:

- burrow multi-angle sheet: `/tmp/mclone-rabbit-burrow-sheet.png`, SHA-256
  `96ddae394b83c471bb64f65aa3acb85fb323201c39671b5b5260ace3dca8c673`;
- hop strip: `/tmp/mclone-rabbit-hop-sheet.png`, SHA-256
  `e3db3c26ad4d272026ca3ae9ebbb55327c8b6ff142002919bcbc030b5fdafb6e`;
- dig strip: `/tmp/mclone-rabbit-dig-sheet.png`, SHA-256
  `dd4aa554ef3e112eb5a670a553295b6390a74ef965cd70a725dd3982e1bd5cbe`.

### Shared rabbit and warren ecology

Commits `e2bbbc74` and `2e506cba` added `Rabbit` and `RabbitBurrow` across the
shared protocol, server, persistence, client replica, scene, render-session,
UI, assets, and audio owners. Ordinary Mclone natural spawning now requires a
dry bank, support, clearance, vegetation, open movement space, and local
occupancy. A founder reaches a qualified cell, performs the authored dig
motion, removes exactly that live dirt/grass cell, and creates a durable mouth
whose resident identities and capacity survive entity-chunk reload.

The same persistent rabbit emerges and shelters according to world time,
hops through shared collision, flees a nearby player toward home, reconsiders
forage instead of retaining a stale destination, seeks live mature carrots,
and mutates one crop age after a completed raid. Closed fence/gate collision
protects the garden; an open gate is traversable. A held carrot tempts rabbits,
and server-validated use consumes one carrot and starts persistent adult
breeding state. Compatible fed adults sharing a warren with capacity produce
one durable smaller kit with parent identities and cooldowns.

Six ordinary observations and spatial thump/dig cues are routed from actual
proximity, burrow, threshold, raid, and family events. The sound families reuse
provenance-locked Kenney CC0 samples rather than adding an ambient rabbit
track.

Focused coverage includes habitat qualification and exclusion, natural
founder creation, completed excavation, warren/rabbit identity round trips,
underground visibility, emergence, displacement, stall recovery, fence/gate
reachability, mature-carrot target selection and raid state, feeding range and
inventory consumption, courtship/capacity/birth, field observations, audio,
replication, and prepared semantic assets.

### Bounded review save and behavior proof

Commits `672b4e5b` and `4e0bda07` add `rabbit-burrow` revision 1. The data-only
recipe uses seed `17507`, entry feet `14.5,65,18.5`, target
`15,65.5,9.5`, one saved warren family, a second ordinary founder, one real
bank threshold, a closed fenced carrot garden, and twelve carried carrots.
Every gameplay fact has typed evidence for ordinary habitat spawning,
excavation, farming, enclosure placement/use, family persistence, or field
observation. No rabbit AI, update, interaction, or rendering branch knows the
showcase ID.

The desktop and phone harnesses first observe 100 protected ticks with all four
mature carrots unchanged. They then walk through ordinary controls, open the
gate, retreat beyond the flee radius, and observe at least 360 and at most 720
rabbit ticks. Acceptance requires useful multi-rabbit displacement, hop
presentation, a second live burrow, and at least one real crop raid. Finally,
the harness selects the carrot through keyboard or rendered touch controls,
approaches a moving adult, feeds it through normal entity interaction, and
checks `12 -> 11`. Commits `057d45c4` and `3a950022` retain simulation-tick
outcomes while allowing throttled phone wall time and an adaptive bounded raid
window.

The inspected native flat capture is `/tmp/mclone-rabbit-burrow.png`, SHA-256
`26e1a3e21babd1a3a238cb0a2366db38d9a261bd2db619c87b604a5f363e4056`.
It shows the rabbit family and recessed mouth on the left and the connected
fenced carrot garden on the right from the recipe entry camera. The inspected
stereo capture is `/tmp/mclone-rabbit-burrow-stereo.png`, SHA-256
`32a8947fa83b9c923f1cd88bbabe4dec82f849d50f2e31e8c2bfe0054ed4e232`.
Inspected local desktop and phone Web captures have SHA-256
`f1eacb509a9d3974719a561c875a3adada496271144f531968c2bd752dd7daf0`
and
`309bd74a8386224617d04fc5ea4a25972d6dae9af75ac74336178c416d373499`.

### Host validation

The proportional closeout matrix passes on 2026-08-13:

- full `cargo test --manifest-path native/Cargo.toml`;
- focused rabbit habitat, mob, store, farming, integrated-interaction,
  persistence, protocol, client, scene, asset, audio, and showcase suites;
- `pnpm native:thin-adapters:purity`;
- `pnpm native:desktop-offscreen:smoke`;
- `pnpm native:web:build`;
- native flat/stereo rabbit capture; and
- local headed WebGPU desktop and phone rabbit behavior gates.

### Deployment receipt

Exact pushed implementation revision
`3a950022c3440ccac9fb5f44689c46c808f69d7f` deployed as asset version
`3a950022c344-20260813145448` and Cloudflare Worker version
`c2ee65eb-2ad4-4fa9-941e-2b1e8fda174a`. The clean public review URL is:

```text
https://mclone.kzahel.com/app.html?showcase=rabbit-burrow
```

The sequential public desktop and 390×844 phone gates both kept mature states
`308,308,308,308` intact behind the closed gate, opened state 270 to 274
through ordinary controls, then observed final states `307,307,308,308`.
All four rabbits accumulated useful travel: desktop measured
`29.97,21.52,26.74,25.83` blocks and phone measured
`30.44,21.65,26.75,26.04`. Both saw hop, emerge, dig, and forage clips, one
new burrow, field notes at 5/6, and carrot feeding `12 -> 11`.

All eight IndexedDB stores remained at zero records in both public runs. The
inspected public desktop and phone capture digests are
`81133b9bc7bfcb5f1aed0314838b5fd976bd0a2ee5b04b09b599556082c89895`
and
`4803f1656053c21dd700652707d91c336efe1ed1a274e66f8f1d2abb1e9be1e9`.
