# Creature Corner Animal Behavior Case Study

Topic: `creature-corner-animal-behavior-case-study`

Status: **research captured 2026-07-24 from the public project pages and the
2026-03-18 `-1.0 Release` source at commit
`6d85c9932fe1d838dcfbd8d21354593f3bac9ae0`. Creature Corner is a useful
case study in making a small animal roster feel alive through homes,
reproduction, hunger, flocking, predation, protection, contextual animation,
and sound. This study is not an implementation approval, parity target, or
license to copy the mod's assets.**

## Scope And Short Answer

This topic records Creature Corner as an external animal-behavior design
reference and compares its roster with the current Asset Lab catalogue. It
owns:

- the researched behavior and interaction inventory;
- the distinction between already-authored figures and behavior-complete
  creatures;
- reusable design lessons for original Mclone animal systems; and
- source-quality and licensing cautions that should survive future planning.

The mod's current source release contains five creatures:

1. Pigeon
2. Coyote
3. Crested caracara
4. Endove
5. Gallian

Asset Lab already has a Pigeon, Coyote, and Bald Eagle. It does not have an
exact Crested Caracara, Endove, or Gallian. Creature Corner itself does not
contain an eagle. The existing eagle is therefore not a missing port from this
mod, and it is not a substitute for the caracara's long-legged,
ground-foraging body plan.

The main lesson is more important than the roster comparison: Creature Corner
gets disproportionate value from relationships among a few creatures. Pigeons
flock, breed through physical nests, sleep in lofts, and flee predators.
Hunger causes coyotes and caracaras to scavenge and hunt. Gallians counter
predators and gather young animals. Animation and sound expose those state
changes to the player.

This study does not change the Minecraft Java 1.17.1 vanilla Overworld target.
Any non-vanilla creature adoption would need an explicit content/profile
decision and the normal shared-first engine ownership.

## Specimen And Provenance

The public project surfaces disagreed when inspected on 2026-07-24:

| Surface | Observed state |
|---|---|
| [CurseForge project](https://www.curseforge.com/minecraft/mc-mods/creature-corner) | Description names Pigeon, Coyote, Crested Caracara, and Endove |
| [Modrinth project](https://modrinth.com/mod/creaturecorner) | Description also includes Gallian |
| [Public source](https://github.com/ChickenDesigner/ChickensAnimalsMod) | `multiloader` release branch registers all five |

The source specimen used for this study is:

| Field | Receipt |
|---|---|
| Repository | [`ChickenDesigner/ChickensAnimalsMod`](https://github.com/ChickenDesigner/ChickensAnimalsMod) |
| Branch | `multiloader` |
| Commit | [`6d85c9932fe1d838dcfbd8d21354593f3bac9ae0`](https://github.com/ChickenDesigner/ChickensAnimalsMod/commit/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0) |
| Commit date | 2026-03-18 |
| Commit subject | `-1.0 Release` |
| Listed game target | Minecraft 1.21.1, Fabric and NeoForge |

The source was inspected from a temporary clone under `/tmp`; no third-party
code, models, textures, or sounds are vendored in this repository.

The exact release commit is used below instead of mutable branch links. This
matters because the repository's default branch describes an older
four-creature state and has materially different licensing text.

## Asset Lab Comparison

This is a catalogue comparison, not a live-runtime comparison. The deployed
catalogue topic explicitly distinguishes Asset Lab-only figures from promoted
runtime figures; see [`animal-catalogue.md`](animal-catalogue.md).

| Creature | Asset Lab state | Important difference |
|---|---|---|
| Pigeon | Present in [`examples/pigeon`](../../tools/asset-lab/examples/pigeon/figure.ts) | Authored figure and flight clip exist; the mod adds flock, nest, egg, loft, variant, and predator-response systems |
| Coyote | Present in [`examples/coyote`](../../tools/asset-lab/examples/coyote/figure.ts) | Authored figure and trot clip exist; the mod adds hunger, scavenging, hunting, avoidance, anger, variants, and idles |
| Crested caracara | Missing as an exact species | The eagle is useful raptor anatomy reference, but lacks the caracara's leggy ground gait and scavenger identity |
| Endove | Missing | It is structurally a fantasy derivative of Pigeon and could share most rig and behavior contracts |
| Gallian | Missing | Asset Lab has adjacent large-bird and fantasy-bird anatomy, but no exact giant guardian bird |
| Eagle | Present in [`examples/eagle`](../../tools/asset-lab/examples/eagle/figure.ts) | A 15-box Bald Eagle with one flight clip; it is not in Creature Corner and is not yet behavior-complete |

The broader Asset Lab inventory is tracked in
[`ANIMALS.md`](../../tools/asset-lab/ANIMALS.md). Presence in that inventory
means a reusable visual source exists. It does not imply ecology, persistence,
AI, sound, animation breadth, reproduction, or runtime promotion.

## Why The Small Roster Feels Dynamic

Creature Corner composes ordinary goal-oriented entity AI into several
overlapping loops:

```text
seeds -> pigeon breeding -> pregnancy -> nest -> eggs -> hatchlings
                              |
night or rain -> pigeon -> loft -> sleep/snore -> daylight release

dropped meat -> hungry coyote/caracara -> pick up -> visibly eat -> satiation
                                     `-> acquire prey -> chase/dive -> feed

cats / wolves / ocelots / coyotes -> frighten pigeons
hungry coyotes and caracaras      -> hunt pigeons and farm animals
Gallian                           -> attacks coyotes / wolves / foxes
Gallian leader                    -> gathers nearby baby animals
```

No individual edge is unusually complex. The effect comes from shared world
state:

- hunger decides whether a predator is dangerous;
- weather and time decide whether a loft is occupied;
- breeding produces a physical object and location rather than an immediate
  child;
- prey can perceive and react to predators;
- a guardian changes the safety of the same farm ecosystem; and
- state-specific motion and sound make the simulation legible.

This is a useful contrast with a catalogue-only creature wave. Five deeply
connected creatures can create more incidents and player stories than a much
larger set of isolated wander-and-flee actors.

## Pigeon: Ambient-Life Foundation

The
[`Pigeon`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Pigeon.java)
is the densest general-purpose animal design in the release.

### Movement And Flocking

- It switches between ground and flying navigation rather than staying in one
  locomotion mode.
- Flying pigeons can become flock leaders; nearby adults can follow, with a
  local group capped at roughly ten.
- Followers synchronize their flying state with the leader.
- Flocking is conditioned by time and reproductive state rather than being a
  permanent visual formation.
- The animal slow-falls when airborne but not actively flying, avoiding a
  harsh drop between navigation states.
- It avoids wolves, ocelots, cats, and coyotes, and it panics when harmed.

The dedicated
[`PigeonFlockFollowLeader`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/PigeonFlockFollowLeader.java)
and
[`PigeonFlyGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/PigeonFlyGoal.java)
separate social following from locomotion selection.

### Place In The World

- Mountain spawning creates groups rather than isolated birds.
- A separate village spawner periodically admits pigeons while enforcing a
  local population cap.
- Three ordinary coats are represented: gray, white, and red.
- Offspring normally inherit a parent coat; same-gray parents can produce a
  white or red mutation.
- Naming an adult `Cannoli` activates a special texture.

The village spawner is notable because it gives the species a recognizable
human-settlement niche without making it a player-owned pet.

### Reproduction Through Nests

Breeding with seeds does not immediately create a child. The female becomes
pregnant and runs a sequence of nest goals:

1. locate a suitable existing nest;
2. travel to it;
3. construct a nest when none is available;
4. lay one or two eggs; and
5. allow the eggs to hatch into the inherited variants.

The reusable source pieces are
[`EggLayerBreedGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/nesting/EggLayerBreedGoal.java),
[`LocateNestGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/nesting/LocateNestGoal.java),
[`GoToNestGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/nesting/GoToNestGoal.java),
and
[`BuildNestGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/nesting/BuildNestGoal.java).

Players can remove eggs from nests and place them back. Thrown pigeon eggs also
have a vanilla-like chance to hatch. Reproduction is therefore spatial,
visible, interruptible, and connected to blocks and items.

### Loft Home Loop

Pigeons search for an empty loft during rain or at night. Entering serializes
the occupant into the
[`PigeonLoftBlockEntity`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/blockentity/custom/PigeonLoftBlockEntity.java).
The loft retains variant and age, renders its occupant, releases it in dry
daylight after a minimum stay, and supports an emergency exit when fire is
nearby.

An occupied loft occasionally emits one of four sleep sounds. This creates a
small but complete home loop:

```text
free animal -> perceive weather/time -> seek owned place -> disappear inside
            -> audible/visible occupancy -> environmental release
```

The persistence is part of the behavior, not just an animation trick.

## Crested Caracara: Grounded Aerial Predator

The
[`Caracara`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Caracara.java)
is the most valuable missing real-animal concept for Asset Lab.

- It inhabits badlands, savannas, and desert-village environments.
- It has a bounded food level that decays.
- Hunger admits prey selection; satiation suppresses unnecessary predation.
- It seeks dropped meat, carries it visibly, and eats it.
- Potential prey include pigeons, chickens, rabbits, pigs, and sheep, with
  narrower rules for young caracaras.
- An aerial hunt first moves above the target, then transitions into a swoop
  and attack.
- Rendering exposes walking, running, flying, and diving, including pitch and
  roll during aerial movement.
- Breeding uses raw rabbit and produces one to three nest eggs.
- It prefers to construct nests atop cactus and is immune to cactus damage.

The cactus relationship is particularly effective species design: one small
habitat-specific rule simultaneously communicates anatomy, niche, silhouette,
and a memorable place to search for eggs.

The caracara should not be treated as an eagle skin. Its long legs, frequent
ground locomotion, scavenging, and crackling vocal identity are central. The
existing Asset Lab eagle can contribute generic raptor wing and beak
experience, but a caracara needs its own proportions and gait.

## Coyote: Shy Hunger-Driven Scavenger

The
[`CoyoteEntity`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/CoyoteEntity.java)
avoids being merely a smaller wolf.

- A wild, unharmed coyote avoids nearby players.
- Harm changes it from cowardly avoidance to temporary neutral anger and
  retaliation.
- Untamed coyotes have a decaying food level.
- Hunger drives searches for dropped meat and attacks on pigeons, chickens,
  rabbits, and baby sheep.
- Kills restore different amounts of food according to prey size.
- Orange, rusty, jackal, and cold-biome white coats provide regional and
  inherited variation.
- A rare ear-scratch idle stops locomotion for the authored action duration.
- Tamed presentation assets include sitting and a tongue-out idle.

This creates three readable modes from a compact state space:

| State | Player-visible behavior |
|---|---|
| Wild and undisturbed | Keeps its distance |
| Hungry | Searches, scavenges, or hunts |
| Harmed | Turns and retaliates |

The source contains tamable-owner scaffolding, but the release has no working
player interaction that changes a wild coyote into a tamed one. A howl state
also exists without a live model call or howl sound. Taming and howling should
therefore be recorded as unfinished intent, not shipped behavior.

## Endove: Economical Fantasy Derivative

The
[`Endove`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Endove.java)
inherits Pigeon and reuses its movement, flocking, breeding, nesting, egg, and
loft machinery.

Its differentiation is compact:

- it spawns in End biomes in larger flocks;
- it emits portal particles;
- its nest and eggs are dimension-themed;
- its eggs hatch in the End;
- it is sensitive to water and clean-water potions;
- projectiles and other damage trigger teleport attempts;
- teleport destinations reject water; and
- it uses distinct calls plus the familiar Enderman teleport sound.

This is a strong example of behavior reuse. A deep ordinary animal becomes a
recognizable fantasy species through a different habitat, material treatment,
particles, vulnerability, and one signature reaction. It argues for shared
species-family contracts instead of duplicating whole actors.

## Gallian: Discovery, Imprinting, And Guardianship

The large fantasy
[`GallianEntity`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/GallianEntity.java)
has the strongest authored player story in the mod.

### Acquisition And Imprinting

The creature does not use ordinary natural spawning. Its placed egg is wired
into jungle-temple and shipwreck treasure loot, cracks through visible stages,
and eventually hatches a distinct chick. During an early age window, an
unowned chick searches for a nearby player and imprints on the first suitable
one.

This sequence makes ownership an event in the world:

```text
explore -> discover egg -> carry/place -> observe cracking -> attend hatch
        -> chick imprints -> raise guardian
```

It is substantially more memorable than applying a repeated taming item to an
adult random spawn.

### Adult Social Role

- A hungry adult pecks short grass or converts a grass block to dirt.
- It attacks foxes, coyotes, and wolves.
- The
  [`DefendFarmAnimalsGoal`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/DefendFarmAnimalsGoal.java)
  searches for nearby hurt animals and targets their attacker.
- Wild Gallians squint at non-creative players; owned Gallians retain that
  suspicion toward non-owners.
- One local adult can become a leader and attract nearby baby animals, except
  ordered-to-sit pets.
- Adult animation states include peck, blink, squint, attack, walk, and run;
  chicks add pecking and tail-shaking.

The combination of predator defense and gathering young animals gives the
Gallian a farm role rather than only a combat statistic. The squint is a cheap
facial behavior with a strong emotional reading.

The release behavior is incomplete in several places. `isFood` returns false,
no breeding goal is installed, and the owned actor lacks ordinary follow and
sit goals. Ownership mainly affects attribution and suspicion. The concept is
more complete than the implementation.

## Sound And Personality

The release
[`sounds.json`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/resources/assets/creaturecorner/sounds.json)
defines small but deliberately varied palettes:

| Creature | Authored palette | Behavioral use |
|---|---|---|
| Pigeon | 3 idle coos, 2 hurt, 1 death, 6 flaps, 4 sleep sounds | Makes flight and loft occupancy audible |
| Endove | 3 idle calls, 2 hurt, 1 death, plus inherited/shared effects | Preserves pigeon readability while teleport sound marks its signature reaction |
| Caracara | 3 crackling idles, 3 hurt, 1 death | Separates the raptor from generic eagle cries |
| Coyote | 3 short idle barks, 2 hurt, 1 death | Supports alert, shy canid character without a functioning howl |
| Gallian | 3 soft idles, 2 hurt, 1 death, egg sounds | Contrasts the animal's size with quieter social calls |

The Gallian content also includes a roughly four-minute “Gallian Grotto” music
track intended for a discoverable disc.

The important pattern is contextual placement, not file count. Wing samples
prove takeoff and flight. Snores prove that a loft is occupied. Eating sounds
close the hunger loop. Egg crack and hatch sounds turn slow block-state changes
into anticipated events. The sound is evidence of behavior.

## Transferable Design Lessons

### Prefer A Behavior Stack Over A Roster Entry

A useful creature is not just geometry plus wander and flee. Creature Corner's
best actors combine:

1. habitat and spawning niche;
2. locomotion modes;
3. needs such as hunger, safety, or reproduction;
4. relationships with other actors;
5. a home or environmental affordance;
6. visible action clips;
7. contextual sound; and
8. persistent consequences such as eggs, offspring, variants, or ownership.

Not every creature needs every layer, but at least one distinctive loop should
survive after the novelty of the model wears off.

### Make Relationships Bidirectional

Predator AI becomes more convincing when prey recognizes it. Farm defense
becomes meaningful when predators already participate in that ecology.
Flocking matters when danger can scatter it. Creature definitions should
therefore identify both outgoing actions and incoming perceptions.

### Give Animals Places, Not Only Coordinates

The pigeon loft and nests are successful because they bind an animal to a
legible world object. Time, weather, reproduction, persistence, sound, and
player interaction all meet at that place.

A Mclone animal-home contract could eventually support nests, dens, roosts,
burrows, hives, and shelters without making each one a wholly separate actor
architecture. That is a future design implication, not an implementation
decision in this topic.

### Let Needs Gate Disruption

Coyotes and caracaras do not hunt continuously. Hunger makes their aggression
episodic. The world alternates between calm observation and short incidents,
and dropped food can satisfy the same need without a kill.

This is preferable to always-on targeting for ambient animals. Needs should
change behavior selection and then be visibly resolved.

### Derive Fantasy Species From Deep Ordinary Species

Endove demonstrates a productive content hierarchy:

```text
deep shared pigeon family
  -> ordinary pigeon: village / mountain / nest / loft / variants
  `-> End derivative: portal material / teleport / water weakness / End eggs
```

The fantasy form is inexpensive only because the ordinary form already has a
strong behavioral foundation.

### Use Small Idles As Character Multipliers

The coyote ear scratch, Gallian squint, chick tail shake, visible eating, and
loft snoring are low-frequency actions with high recognition value. They
interrupt mechanical locomotion and imply an internal state.

An idle should be tied to context or temperament where possible rather than
chosen as an arbitrary animation lottery.

### Build Ownership Around An Event

Gallian imprinting is more distinctive than conventional feed-to-tame loops.
Discovery, incubation, attendance, and early care create a relationship before
the adult utility arrives. Similar sequences could support rescued,
hand-raised, bonded, or habituated animals without making every species a
collared pet.

## Recommended Mclone Direction

If this research later becomes authorized work, the highest-value order is:

1. **Pigeon behavior foundation.** Extend the existing figure with flocking,
   predator response, nests and eggs, village ecology, loft sleep, variants,
   and contextual sound. This creates reusable bird-family and animal-home
   contracts.
2. **Crested caracara.** Add the most distinctive missing real animal with a
   ground gait, scavenging, hunger, circling/swooping pursuit, cactus nesting,
   and crackling calls. Do not present it as an eagle variant.
3. **Original guardian-bird concept.** Use the Gallian's
   egg-to-imprint-to-farm-role arc as inspiration while designing independent
   Mclone anatomy, identity, assets, and exact mechanics.
4. **Coyote personality pass.** Apply shyness, hunger, scavenging, coat
   variation, ear scratching, and a deliberately completed social/howl
   contract to the existing coyote.
5. **Fantasy pigeon derivative.** Consider an independently designed
   dimension-themed bird only after the shared pigeon behavior is mature.
6. **Eagle behavior pass.** The figure already exists; later work should add
   soaring, perching, nesting, prey acquisition, and a species-appropriate
   talon strike instead of creating another static eagle asset.

This ordering deliberately extracts shared behavior leverage before adding
more isolated figures.

Any real implementation must route authoritative needs, reproduction,
relationships, and world effects through shared simulation ownership; input,
renderer, platform, and app adapters must not acquire animal policy. New
world-visible animation and effects must also cover ordinary and XR multiview
render paths under the repository guardrails.

## Source Quality And Licensing Cautions

Creature Corner should be treated as design evidence, not a normative behavior
specification.

Observed release roughness includes:

- CurseForge's public roster omits the source-present Gallian.
- Coyote taming infrastructure has no live taming entry interaction.
- Coyote howl state exists without a live animation or sound.
- Gallian breeding and ordinary owned follow/sit behavior are incomplete.
- Some release loot/resource identifiers use inconsistent namespaces.
- Several egg and spawn paths contain code shapes that warrant independent
  verification before treating edge behavior as intentional.

Licensing also needs clarification before any reuse:

- the inspected `multiloader` release branch contains a CC0 `LICENSE`;
- the default branch's `LICENSE.txt` says code is MIT while
  `src/main/resources/assets/creaturecorner` is All Rights Reserved; and
- the public project metadata does not resolve which text governs every file
  in the release branch.

Behavioral ideas can be independently studied and redesigned. Models,
textures, sounds, music, and source should not be copied into Mclone based on
this research. If direct reuse ever becomes desirable, obtain a clear
file-by-file license statement from the author first.

## Source Map

Primary release files:

- [`Pigeon.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Pigeon.java)
- [`Caracara.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Caracara.java)
- [`CoyoteEntity.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/CoyoteEntity.java)
- [`Endove.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/Endove.java)
- [`GallianEntity.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/GallianEntity.java)
- [`PigeonLoftBlockEntity.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/blockentity/custom/PigeonLoftBlockEntity.java)
- [`PigeonNestBlock.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/block/obj/custom/PigeonNestBlock.java)
- [`GallianEggBlock.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/block/obj/custom/GallianEggBlock.java)
- [`DefendFarmAnimalsGoal.java`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/java/chicken/creaturecorner/server/entity/obj/goal/DefendFarmAnimalsGoal.java)
- [`sounds.json`](https://github.com/ChickenDesigner/ChickensAnimalsMod/blob/6d85c9932fe1d838dcfbd8d21354593f3bac9ae0/common/src/main/resources/assets/creaturecorner/sounds.json)

Local comparison sources:

- [`tools/asset-lab/ANIMALS.md`](../../tools/asset-lab/ANIMALS.md)
- [`examples/eagle/figure.ts`](../../tools/asset-lab/examples/eagle/figure.ts)
- [`examples/pigeon/figure.ts`](../../tools/asset-lab/examples/pigeon/figure.ts)
- [`examples/coyote/figure.ts`](../../tools/asset-lab/examples/coyote/figure.ts)
- [`animal-catalogue.md`](animal-catalogue.md)
