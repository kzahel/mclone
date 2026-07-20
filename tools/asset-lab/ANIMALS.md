# Asset Lab — Animal Figure Roadmap & Checklist

Planning / tracking / prioritization doc for figures authored in the
[Asset Lab](README.md). Scope is **recognizable real-world animals** — the ones
a player would name on sight — plus sensible variants (breed, color morph, age,
sex). A short fantasy/anthro stretch section at the end continues the existing
`-folk` line.

> **Scope:** real animals first, organized by family/type, then fantasy/anthro
> `-folk` figures and a **Monsters & baddies** cast (skeletons, zombies, the
> humanoid pig/pigman, etc.) since this is a Minecraft-style game. Everything
> below is a first pass meant to be edited — say the word and any slice gets
> re-cut or re-prioritized.

---

## How to use this doc

- **Status** is the checklist. Flip the emoji as work moves.
  - `☐` not started · `🔨` in progress · `✅` shipped (has `examples/<name>/figure.ts`) · `🔁` exists but needs a revisit
- **Pri** is build priority: `P0` flagship/first-wave · `P1` core · `P2` worth doing · `P3` stretch/exotic.
- **Body plan** flags which gait macro the figure needs. This is the single most
  useful planning signal: some animals are "free" (a macro already exists),
  others are blocked on **new tooling** (a gait macro that doesn't exist yet).

### Body-plan / gait legend

| Tag | Meaning | Macro status |
|---|---|---|
| **Q** | Quadruped walk (4 legs) | ✅ `quadrupedWalk` exists |
| **B** | Biped walk (2 legs, 2 arms) | ✅ `bipedWalk` exists |
| **W** | Winged flight | ✅ `wingFlap` exists |
| **S** | Swim (body/fin/tail undulation) | ❌ **needs a `swim` macro** |
| **SL** | Slither (serpentine, legless) | ❌ **needs a `slither` macro** |
| **H** | Hop (rabbit/frog, synchronized hind legs) | ⚠️ approximate with `contactSwing`; a `hop` macro would be cleaner |
| **C** | Crawl, many legs (insects/arachnids) | ⚠️ hand-author or generalize `quadrupedWalk` to N legs |
| **ST** | Static / minimal motion (idle sway only) | ✅ plain `walkCycle`/`clip` |

### Tooling gaps to schedule alongside content

These unlock whole families, so they should be prioritized as their own slices:

- [ ] **`swim` macro** → unlocks all marine life (fish, shark, dolphin, whale, octopus…). High leverage.
- [ ] **`slither` macro** → unlocks snakes, eels, worms.
- [ ] **`hop` macro** → rabbit, frog, kangaroo, grasshopper (cleaner than faking with contactSwing). *Rabbit currently ships a hand-authored approximation (`examples/rabbit`: synchronized `contactSwing` legs + phased body `bob`); this macro would replace it.*
- [ ] **N-leg crawl** (generalize `quadrupedWalk`) → spiders (8), insects (6), crabs.
- [ ] **roll-up / curl helper** (a pose, not a gait) → roly-poly, armadillo, pangolin.

---

## Figure style direction

Use a **box-only Minecraft-style vocabulary** for every canonical and promoted
figure. Start with a sparse cuboid rig and pixel face textures. Spheres,
capsules, and cylinders are deprecated authoring inputs; do not spend curved
geometry on whiskers or other tiny surface details that a pixel texture can
express—or that can simply be omitted.

Canonical sources live under `examples/` and use `figure()`. Retained rounded
A/B sources live under `legacy-examples/` and use the explicitly deprecated
`legacyFigure()` compatibility API. Schema-v1 still parses those historical
primitive kinds, but the first-party drift gate rejects them from promotion.

### Blocky conversion queue

The 2026-07-20 inventory ranked existing animal sources by non-box share, then
used body-plan coverage and likely rig reuse to choose each wave.

| Wave | Animals | Why |
|---|---|---|
| Canonical now | Elephant, Tiger, Rabbit, Butterfly, Chicken | Approved box-only sources promoted to their ordinary names; rounded comparisons archived explicitly |
| Completed Wave 1 | Cat, Cow, Goat | Re-authored as 62 boxes total; pixel markings replace whiskers, hide patches, socks, nostrils, and cloven-toe geometry |
| Wave 2 | Dog, Fox, Wolf | Shared canid anatomy with distinct proportions and textures |
| Wave 3 | Piglet, Sheep, Horse | Farm silhouettes spanning compact, woolly, and long-legged rigs |
| Wave 4 | Bear, Lion, Bearfolk, Lionfolk | Heavy quadrupeds plus the two player-derived anthropomorphic rigs |

Humanoid figures are outside this animal conversion queue. `player` and
`upright_bear` are already box-only.

---

## Recommended build order

Ordered to maximize reuse of the macros that already exist and to fill obvious
gaps in the current eight figures.

1. **Finish the farmyard (P0, all Q/B — no new tooling).** Cow, chicken, horse,
   goat, rabbit. We already have pig(let), sheep, dog, cat — this rounds out the
   single most recognizable animal set in the game.
2. **Iconic wild quadrupeds (P0–P1, Q).** Wolf, fox, bear, lion, tiger,
   elephant, deer. All ride `quadrupedWalk`.
3. **Birds (P1, W).** Chicken (also farm), owl, parrot, eagle — exercise and
   harden `wingFlap`.
4. **First swim wave (P1, S) — build the `swim` macro, then** fish, dolphin,
   shark. One tooling investment, large payoff.
5. **Everything else** by recognizability and by which tooling gap it shares.

---

## Farm & livestock

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Pig | P0 | 🔨 | Q | piglet, adult pink, spotted, boar (tusks) | Rounded source retained at `legacy-examples/piglet_rounded`; canonical box-only re-authoring is Wave 3 |
| Sheep | P0 | 🔨 | Q | white, black, brown, shorn (no wool), lamb, dyed (MC nod) | Rounded source retained at `legacy-examples/sheep_rounded`; canonical box-only re-authoring is Wave 3 |
| Cow | P0 | ✅ | Q | Holstein (black/white), brown (Jersey), calf, bull (horns) | `examples/cow` — canonical 22-box Holstein with stepped horns, udder, texture-painted hide and socks; rounded A/B retained at `legacy-examples/cow_rounded` |
| Chicken | P0 | ✅ | W | hen, rooster (comb/wattle/long tail), chick | `examples/chicken` — canonical 14-box hen; `legacy-examples/chicken_rounded` retains the rounded A/B; both preserve `bipedWalk` wing/head motion |
| Horse | P0 | 🔨 | Q | brown, black, white, palomino, foal; pony | Rounded source retained at `legacy-examples/horse_rounded`; canonical box-only re-authoring is Wave 3 |
| Goat | P1 | ✅ | Q | white, brown, kid, billy (horns + beard) | `examples/goat` — canonical 24-box billy with stepped swept horns, beard, cloven-hoof texture, and upturned tail; rounded A/B retained at `legacy-examples/goat_rounded` |
| Rabbit | P1 | ✅ | H | brown, white, gray, black, lop-ear, kit | `examples/rabbit` — canonical 18-box rabbit; `legacy-examples/rabbit_rounded` retains the rounded A/B and matching synchronized hop |
| Donkey / Mule | P2 | ☐ | Q | donkey, mule | horse variant; big ears |
| Duck | P2 | ☐ | W/S | mallard drake, hen, duckling | walks + paddles; pairs with swim work |
| Turkey | P2 | ☐ | W | tom (fanned tail), hen | |
| Llama / Alpaca | P2 | ☐ | Q | white, brown, gray | tall neck |

## Domestic pets

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Dog | P0 | 🔨 | Q | breeds: shepherd, lab, husky, pug, dachshund, corgi; puppy | Rounded source retained at `legacy-examples/dog_rounded`; canonical box-only re-authoring is Wave 2 |
| Cat | P0 | ✅ | Q | tabby, black, white, calico, orange, siamese; kitten | `examples/cat` — canonical 16-box tabby with pixel face/stripes and an attached two-piece cuboid tail; rounded A/B retained at `legacy-examples/cat_rounded` |
| Hamster / Guinea pig | P3 | ☐ | Q | — | tiny, rounded |
| Parrot (pet) | P2 | ☐ | W | see Birds | |

## Canids (wild)

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Wolf | P0 | 🔨 | Q | gray, black, arctic (white), pup | Rounded source retained at `legacy-examples/wolf_rounded`; canonical box-only re-authoring is Wave 2 |
| Fox | P1 | 🔨 | Q | red, arctic (white), fennec (huge ears), kit | Rounded source retained at `legacy-examples/fox_rounded`; canonical box-only re-authoring is Wave 2 |
| Coyote | P3 | ☐ | Q | — | between wolf and fox |
| Hyena | P3 | ☐ | Q | spotted, striped | not a canid, but a dog-like rig fits |

## Felids (big cats)

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Lion | P1 | 🔨 | Q | male (mane), lioness, cub | Rounded source retained at `legacy-examples/lion_rounded`; canonical box-only re-authoring is Wave 4 |
| Tiger | P1 | ✅ | Q | orange, white, cub | `examples/tiger` — canonical 20-box tiger with pixel-textured stripes; `legacy-examples/tiger_rounded` retains the rounded A/B |
| Leopard / Jaguar | P2 | ☐ | Q | spotted, melanistic (black panther) | spot rosette texture |
| Cheetah | P2 | ☐ | Q | — | slender; tear-mark face |
| Lynx / Bobcat | P3 | ☐ | Q | ear tufts | scaled-up cat rig |

## Bears

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Bear | P1 | 🔨 | Q | brown/grizzly, black, polar (Arctic), cub | Rounded source retained at `legacy-examples/bear_rounded`; canonical box-only re-authoring is Wave 4 |
| Panda | P2 | ☐ | Q | adult, cub | bear rig + iconic black/white texture |
| Polar bear | P2 | ☐ | Q | adult, cub | also lives in Polar/Arctic set |

## Hoofed & large herbivores

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Deer | P1 | ☐ | Q | doe, buck (antlers), fawn (spots) | antlers = signature variant |
| Moose / Elk | P2 | ☐ | Q | moose, elk | huge antlers |
| Elephant | P1 | ✅ | Q | African (big ears), Asian, calf; tusks | `examples/elephant` — canonical box-only African elephant; `legacy-examples/elephant_rounded` retains the rounded A/B and matching articulated trunk walk |
| Giraffe | P2 | ☐ | Q | adult, calf | extreme neck proportions |
| Zebra | P2 | ☐ | Q | adult, foal | horse rig + stripes |
| Rhino | P2 | ☐ | Q | one-horn, two-horn | |
| Hippo | P2 | ☐ | Q | adult, calf | also semi-aquatic |
| Camel | P2 | ☐ | Q | one hump (dromedary), two hump (bactrian) | |
| Bison / Buffalo | P3 | ☐ | Q | — | hump + shaggy head |
| Antelope / Gazelle / Impala | P2 | ☐ | Q | horns; springbok, oryx | deer/horse-class rig |
| Wildebeest / Gnu | P3 | ☐ | Q | — | safari staple |
| Tapir | P3 | ☐ | Q | — | short trunk; baby is striped |
| Okapi | P3 | ☐ | Q | — | giraffe cousin, zebra legs |
| Warthog | P3 | ☐ | Q | — | tusks; pig-class rig |

## Primates

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Monkey | P2 | ☐ | B/Q | generic, with baby | long tail; knuckle/upright ambiguity |
| Gorilla | P2 | ☐ | B/Q | silverback, female, juvenile | knuckle-walk |
| Chimpanzee | P3 | ☐ | B/Q | — | |
| Orangutan | P3 | ☐ | B/Q | — | long arms; orange shag |
| Lemur | P3 | ☐ | Q/B | ring-tailed | banded tail signature |

## Small & exotic mammals

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Mouse / Rat | P2 | ☐ | Q | — | tiny; long tail |
| Squirrel | P2 | ☐ | Q/H | gray, red; flying squirrel | huge tail |
| Rabbit | P1 | ✅ | H | (see Farm) | `examples/rabbit`; rounded A/B in `legacy-examples/rabbit_rounded` |
| Hedgehog | P3 | ☐ | Q | — | spine texture |
| Porcupine | P3 | ☐ | Q | — | quill texture; hedgehog cousin |
| Raccoon | P2 | ☐ | Q | — | mask + ringed tail |
| Beaver | P3 | ☐ | Q | — | flat tail |
| Bat | P2 | ☐ | W | — | flying mammal; reuses `wingFlap` |
| Meerkat | P2 | ☐ | Q | — | upright sentry idle pose is the signature |
| Otter | P2 | ☐ | Q/S | river, sea | semi-aquatic; pairs with `swim` |
| Red panda | P2 | ☐ | Q | — | ringed tail; zoo favorite |
| Sloth | P3 | ☐ | Q | two-toe, three-toe | very slow / hanging; mostly ST |
| Capybara | P3 | ☐ | Q | — | giant rodent; semi-aquatic |
| Anteater | P3 | ☐ | Q | giant, tamandua | long snout + tongue |
| Armadillo | P3 | ☐ | Q | — | **roll-up** ball pose (see Snail/roly-poly note) |
| Pangolin | P3 | ☐ | Q | — | scale texture; **roll-up** ball pose |
| Mongoose | P3 | ☐ | Q | — | meerkat-class rig |

## Marsupials & monotremes

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Kangaroo | P2 | ☐ | H | adult, joey-in-pouch | wants `hop`; big tail for balance |
| Koala | P3 | ☐ | Q | adult, joey | clinging/climb idle; mostly ST |
| Wallaby | P3 | ☐ | H | — | smaller kangaroo rig |
| Wombat | P3 | ☐ | Q | — | stocky; raccoon-class rig |
| Opossum | P3 | ☐ | Q | — | prehensile tail; hangs |
| Tasmanian devil | P3 | ☐ | Q | — | |
| Platypus | P3 | ☐ | Q/S | — | bill + webbed feet; semi-aquatic |
| Echidna | P3 | ☐ | Q | — | spiny anteater; quill texture |

## Birds

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Chicken | P0 | ✅ | W | (see Farm) | `examples/chicken`; rounded A/B in `legacy-examples/chicken_rounded` |
| Toucan | P3 | ☐ | W | — | oversized colorful bill |
| Owl | P1 | ☐ | W | brown, snowy (white) | |
| Parrot | P1 | ☐ | W | red, green, blue, yellow morphs | color morphs are cheap variants |
| Eagle / Hawk | P2 | ☐ | W | bald eagle (white head), hawk | |
| Penguin | P2 | ☐ | B | emperor, chick | waddle, not flight — biped |
| Duck | P2 | ☐ | W/S | (see Farm) | |
| Pigeon / Dove | P3 | ☐ | W | — | |
| Crow / Raven | P3 | ☐ | W | — | |
| Flamingo | P3 | ☐ | W | — | one-leg idle pose |
| Songbird (robin/sparrow) | P3 | ☐ | W | color morphs | generic small-bird base |
| Peacock | P3 | ☐ | W | — | tail fan showpiece |
| Ostrich / Emu | P3 | ☐ | B | — | flightless runner |

## Marine & aquatic — *blocked on `swim` macro*

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Fish (generic) | P1 | ☐ | S | clownfish, tropical morphs, cod, salmon, pufferfish | base rig for the whole family |
| Shark | P1 | ☐ | S | great white, hammerhead | |
| Dolphin | P1 | ☐ | S | — | |
| Whale | P2 | ☐ | S | orca, humpback, blue | scale challenge |
| Octopus | P2 | ☐ | S | — | 8 tentacles (capsule chains) |
| Crab | P2 | ☐ | C | — | sideways multi-leg |
| Sea turtle | P2 | ☐ | S | adult, hatchling | also Reptiles |
| Seahorse | P3 | ☐ | S | — | |
| Jellyfish | P3 | ☐ | S | — | pulse animation |
| Starfish | P3 | ☐ | ST | — | nearly static |
| Lobster / Shrimp | P3 | ☐ | C/S | — | |
| Seal / Sea lion | P3 | ☐ | S/Q | — | also Polar |
| Manatee / Dugong | P3 | ☐ | S | — | slow "sea cow"; zoo/aquarium staple |

## Reptiles & amphibians

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Snake | P2 | ☐ | SL | green, brown, cobra (hood), rattlesnake | blocked on `slither` |
| Lizard / Gecko | P2 | ☐ | Q | gecko, iguana, chameleon | small sprawled quadruped |
| Turtle / Tortoise | P2 | ☐ | Q/S | land tortoise, sea turtle, hatchling | |
| Crocodile / Alligator | P2 | ☐ | Q | croc, gator | sprawled walk + swim |
| Komodo dragon | P3 | ☐ | Q | — | giant monitor lizard; zoo headliner |
| Frog | P2 | ☐ | H | green, tree-frog morphs, toad; tadpole | wants `hop`; tadpole is S |
| Salamander / Newt | P3 | ☐ | Q | — | |

## Insects & arthropods

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Butterfly | P1 | ✅ | W | color morphs, moth | `examples/butterfly` — canonical nine-box figure with pixel-patterned wing slabs; rounded A/B in `legacy-examples/butterfly_rounded` |
| Bee | P1 | ☐ | W | — | iconic; reuses `wingFlap` |
| Ladybug | P2 | ☐ | C/W | — | |
| Dragonfly | P2 | ☐ | W | — | four wings |
| Spider | P2 | ☐ | C | small, large, color morphs | 8 legs → needs N-leg crawl |
| Ant | P3 | ☐ | C | worker, soldier | 6 legs |
| Beetle | P3 | ☐ | C | rhino/stag beetle | |
| Grasshopper / Cricket | P3 | ☐ | H | — | |
| Scorpion | P3 | ☐ | C | — | claws + tail |
| Roly-poly / Pill bug | P2 | ☐ | C | — | isopod; **roll-up** ball pose is the whole gag |
| Earwig (pincher bug) | P2 | ☐ | C | — | rear pincers (cerci) |
| Centipede / Millipede | P3 | ☐ | C/SL | — | many legs; segment chain |
| Snail | P3 | ☐ | SL | — | shell; very slow |

> **Shared "roll-up" pose:** roly-poly, armadillo, and pangolin all curl into a
> ball. Worth authoring one curl/uncurl helper and reusing it across all three.

## Polar / Arctic set (cross-listed)

A themed bundle if we ever want a biome pack: Polar bear, Arctic fox, Penguin,
Seal/Walrus, Snowy owl, Arctic hare, Reindeer (deer variant). Mostly Q/W/S —
no new tooling beyond `swim` for the marine members.

## Zoo / safari set (cross-listed)

The classic "day at the zoo" roster, pulled from the families above for a themed
pack: Lion, Tiger, Elephant, Giraffe, Zebra, Hippo, Rhino, Gorilla, Orangutan,
Chimp, Monkey, Lemur, Kangaroo, Koala, Penguin, Flamingo, Peacock, Toucan,
Meerkat, Red panda, Sloth, Otter, Komodo dragon, Crocodile, Snake, Camel,
Bear/Polar bear, Hyena, Wildebeest, Antelope. Almost all **Q/B/W** (existing
macros) — the only blocked members are the aquarium wing (manatee, seal — `swim`)
and the hoppers (kangaroo — `hop`).

---

## Stretch: fantasy & anthro (the `-folk` line)

The lab already has anthropomorphic figures (`bearfolk`, `lionfolk`) and the
humanoid `player`. If we lean into an original cast, these are the natural
extensions — all **B** (`bipedWalk`), so no new tooling.

| Figure | Pri | Status | Notes |
|---|---|---|---|
| Player (humanoid base) | — | ✅ | `examples/player`; not an animal, the rig reference |
| Bearfolk | — | 🔨 | Rounded source at `legacy-examples/bearfolk_rounded`; canonical box-only re-authoring is Wave 4 |
| Lionfolk | — | 🔨 | Rounded source at `legacy-examples/lionfolk_rounded`; canonical box-only re-authoring is Wave 4 |
| Wolffolk / Foxfolk | P3 | ☐ | obvious next anthro canids |
| Dragon | P3 | ☐ | flagship mythic; W + Q hybrid, likely new tooling |
| Unicorn / Pegasus | P3 | ☐ | horse rig + horn / wings |
| Griffin | P3 | ☐ | eagle + lion |
| Slime / blob | P3 | ☐ | ST + squash-stretch; trivial rig, fun motion (also a baddie — see below) |

---

## Monsters & baddies

The hostile/enemy cast. **Most are humanoid bipeds** → they reuse the `player` /
`-folk` rig plus `bipedWalk`, so the whole top table is "free" tooling-wise and
should be a high-value early wave for a Minecraft-style mob set.

> **Humanoid-animal overlap:** the friendly `-folk` line and the enemy "beast-man"
> mobs are the *same* biped rig — a humanoid pig (pigman/orc), werewolf, or
> minotaur is a `pigfolk`/`wolffolk`/`bullfolk` body retextured and given a
> weapon. Build the rig once; fork friendly vs. hostile by material + held item +
> idle pose. The humanoid pig you asked for is the poster child for this.

### Humanoid enemies (B — reuse player/`-folk` rig)

| Figure | Pri | Status | Variants to consider | Notes |
|---|---|---|---|---|
| Skeleton | P1 | ☐ | plain, dark/wither, frost/stray, armored | bone texture on the humanoid rig; carries bow |
| Zombie | P1 | ☐ | plain, husk (desert), drowned (water) | shambling walk variant |
| **Humanoid pig (pigman / orc)** | P1 | ☐ | piglin-style, zombified, brute, armored, tusked | **the one you asked for** — pig snout/ears on the folk biped + weapon |
| Goblin | P2 | ☐ | scout, shaman | small biped |
| Orc / Ogre / Troll | P2 | ☐ | orc (medium), ogre/troll (oversized) | big-biped scale of the rig |
| Witch / Warlock | P2 | ☐ | witch, hooded cultist, necromancer | caster pose; staff/potion |
| Werewolf | P2 | ☐ | — | hostile `wolffolk`; ties to Canids |
| Minotaur | P3 | ☐ | — | hostile `bullfolk`; horns + axe |
| Vampire | P3 | ☐ | — | cape; can pair with Bat |
| Mummy | P3 | ☐ | — | wrapped reskin of zombie rig |
| Imp / lesser demon | P3 | ☐ | — | small biped, optional wings/tail |
| Cyclops / Giant | P3 | ☐ | — | oversized biped (mini-boss) |
| Skeleton/zombie animals | P3 | ☐ | undead horse, wolf, etc. | retexture of existing quadrupeds |

### Non-humanoid creature enemies

| Figure | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Slime / blob | P2 | ☐ | ST | small, medium, large; magma/acid | squash-stretch hop; trivial rig |
| Creeper-like crawler | P2 | ☐ | Q | — | legless/4-stub silent stalker |
| Giant spider | P2 | ☐ | C | normal, cave (small) | hostile build of Spider; needs N-leg crawl |
| Ghost / wraith | P2 | ☐ | ST | — | floats (no gait); semi-transparent |
| Golem | P2 | ☐ | B | stone, iron, clay | heavy biped; can be friendly too |
| Gargoyle | P3 | ☐ | W/B | — | winged biped; perch + glide |
| Bat swarm | P3 | ☐ | W | — | hostile build of Bat (`wingFlap`) |
| Mimic (chest monster) | P3 | ☐ | ST | — | block that sprouts teeth; fun gag |
| Will-o-wisp / floating eye | P3 | ☐ | ST | — | tiny floater |

### Bosses (large effort, stretch)

| Figure | Pri | Status | Body | Notes |
|---|---|---|---|---|
| Dragon | P3 | ☐ | W+Q | flagship boss; shares the fantasy Dragon entry above |
| Hydra | P3 | ☐ | SL+Q | multi-head serpent; new tooling |
| Kraken | P3 | ☐ | S | giant octopus; aquarium-scale `swim` |
| Lich / demon lord | P3 | ☐ | B | upgraded humanoid caster |

---

## Coverage snapshot

- **Canonical box-only figures (7):** butterfly, chicken, elephant, player,
  rabbit, tiger, and upright bear.
- **Rounded-to-box migration in progress (13):** piglet, sheep, dog, cat,
  bearfolk, lionfolk, cow, horse, goat, wolf, fox, bear, and lion.
- **Retained rounded A/B archive:** all 18 former mixed-primitive sources live
  under `legacy-examples/` and remain schema-round-trip tested.
- **Macros ready:** `quadrupedWalk` (Q), `bipedWalk` (B), `wingFlap` (W).
- **Macros to build:** `swim` (S), `slither` (SL), `hop` (H — rabbit currently approximates it), N-leg crawl (C).
- **Biggest single unlock:** the `swim` macro — gates the entire marine family.
- **Lowest-effort wins next:** the farmyard is done; iconic wild quadrupeds (Wolf, Fox, Bear, Deer) are the cheapest next wave — all ride `quadrupedWalk`.
- **Cheapest enemy wave:** humanoid baddies (skeleton, zombie, humanoid pig) — all reuse the `player`/`-folk` biped rig + `bipedWalk`, no new tooling.
