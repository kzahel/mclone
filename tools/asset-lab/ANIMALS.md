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
| **S** | Swim (body/fin/tail undulation) | ✅ `swim` exists with configurable lateral or vertical tail motion |
| **SL** | Slither (serpentine, legless) | ✅ `slither` emits a phased lateral segment wave and locomotion metadata |
| **H** | Hop (rabbit/frog, synchronized hind legs) | ⚠️ approximate with `contactSwing`; a `hop` macro would be cleaner |
| **C** | Crawl, many legs (insects/arachnids) | ⚠️ hand-author or generalize `quadrupedWalk` to N legs |
| **ST** | Static / minimal motion (idle sway only) | ✅ plain `walkCycle`/`clip` |

### Tooling gaps to schedule alongside content

These unlock whole families, so they should be prioritized as their own slices:

- [x] **`swim` macro** → shipped with Batch 3: body counter-sway, primary/delayed tail motion, mirrored fins, vertical drift, and configurable lateral or vertical tail axes.
- [x] **`slither` macro** → shipped with Batch 6: phased lateral motion across an ordered segment chain, optional body drift and custom tracks, and ordinary locomotion metadata; unlocks snakes, eels, worms.
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
| Completed Wave 2 | Dog, Fox, Wolf | Re-authored as 51 boxes with visibly distinct domestic, low fox, and tall wolf proportions |
| Completed Wave 3 | Piglet, Sheep, Horse | Re-authored as 51 boxes spanning compact, wool-mass, and long-legged farm silhouettes |
| Completed Wave 4 | Bear, Lion, Bearfolk, Lionfolk | Re-authored as 75 boxes across heavy quadruped and player-derived biped silhouettes |

Humanoid figures are outside this animal conversion queue. `player` and
`upright_bear` are already box-only.

---

## Recommended next build order

The original farmyard and iconic-wild-animal goals are now represented in the
29-figure canonical box-only roster. Continue by maximizing reuse of those
reviewed rigs and then filling macro gaps.

1. **Low-cost rig variants (P1–P2, Q).** Polar bear can reuse the bear;
   leopard and cheetah can reuse the feline rigs; donkey and mule can reuse
   the horse. Deer, zebra, and panda shipped in the first post-migration batch.
2. **Birds (P1, W).** Owl, parrot, and eagle shipped in the second
   post-migration batch and established the reusable flying-bird vocabulary.
   Duck, turkey, penguin, and smaller birds remain open.
3. **First swim wave (P1, S).** The `swim` macro plus fish, dolphin, and shark
   shipped in the third post-migration batch. Whale, turtle, and other aquatic
   figures can now reuse the reviewed lateral/vertical tail vocabulary.
4. **Everything else** by recognizability and by which tooling gap it shares.

---

## Farm & livestock

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Pig | P0 | ✅ | Q | piglet, adult pink, spotted, boar (tusks) | `examples/piglet` — canonical 14-box piglet with vanilla-style square head, projecting snout, floppy ears, and raised tail; rounded A/B retained at `legacy-examples/piglet_rounded` |
| Sheep | P0 | ✅ | Q | white, black, brown, shorn (no wool), lamb, dyed (MC nod) | `examples/sheep` — canonical 15-box sheep with one oversized texture-edged wool mass, forelock, dark face, and short tail; rounded A/B retained at `legacy-examples/sheep_rounded` |
| Cow | P0 | ✅ | Q | Holstein (black/white), brown (Jersey), calf, bull (horns) | `examples/cow` — canonical 22-box Holstein with stepped horns, udder, texture-painted hide and socks; rounded A/B retained at `legacy-examples/cow_rounded` |
| Chicken | P0 | ✅ | W | hen, rooster (comb/wattle/long tail), chick | `examples/chicken` — canonical 14-box hen; `legacy-examples/chicken_rounded` retains the rounded A/B; both preserve `bipedWalk` wing/head motion |
| Horse | P0 | ✅ | Q | brown, black, white, palomino, foal; pony | `examples/horse` — canonical 22-box bay horse with long barrel/legs, angled neck, blaze, animated mane ridge, and hanging tail; rounded A/B retained at `legacy-examples/horse_rounded` |
| Goat | P1 | ✅ | Q | white, brown, kid, billy (horns + beard) | `examples/goat` — canonical 24-box billy with stepped swept horns, beard, cloven-hoof texture, and upturned tail; rounded A/B retained at `legacy-examples/goat_rounded` |
| Rabbit | P1 | ✅ | H | brown, white, gray, black, lop-ear, kit | `examples/rabbit` — canonical 18-box rabbit; `legacy-examples/rabbit_rounded` retains the rounded A/B and matching synchronized hop |
| Donkey / Mule | P2 | ✅ | Q | donkey, mule | `examples/donkey` — approved 23-box adult with compact gray body, large four-box ears, upright mane, dorsal stripe, pale muzzle, and tasseled tail |
| Duck | P2 | ✅ | W/S | mallard drake, hen, duckling | `examples/mallard_duck` — approved 14-box drake with low gray body, green head, white neck ring, blue wing speculum, yellow bill, and webbed-foot waddle |
| Turkey | P2 | ✅ | W | tom (fanned tail), hen | `examples/wild_turkey` — approved 21-box strutting tom with bronze wings, bare blue-red neck, snood, wattle, and seven-feather display fan |
| Llama / Alpaca | P2 | ✅ | Q | white, brown, gray | `examples/llama` — approved 27-box woolly llama with a deep fleece body, upright two-stage neck, long alert ears, slim two-stage legs, and curled tail |

## Domestic pets

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Dog | P0 | ✅ | Q | breeds: shepherd, lab, husky, pug, dachshund, corgi; puppy | `examples/dog` — canonical 17-box broad dog with floppy ears, collar, wide paws, and raised tail; rounded A/B retained at `legacy-examples/dog_rounded` |
| Cat | P0 | ✅ | Q | tabby, black, white, calico, orange, siamese; kitten | `examples/cat` — canonical 16-box tabby with pixel face/stripes and an attached two-piece cuboid tail; rounded A/B retained at `legacy-examples/cat_rounded` |
| Hamster / Guinea pig | P3 | ☐ | Q | — | tiny, rounded |
| Parrot (pet) | P2 | ✅ | W | see Birds | `examples/parrot` — approved 15-box scarlet macaw; see Birds |

## Canids (wild)

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Wolf | P0 | ✅ | Q | gray, black, arctic (white), pup | `examples/wolf` — canonical 16-box tall wolf with a vanilla-derived shoulder mass, upright ears, long legs, and heavy tail; rounded A/B retained at `legacy-examples/wolf_rounded` |
| Fox | P1 | ✅ | Q | red, arctic (white), fennec (huge ears), kit | `examples/fox` — canonical 18-box low fox with oversized ears, black-stocking texture, and attached white-tipped tail; rounded A/B retained at `legacy-examples/fox_rounded` |
| Coyote | P3 | ☐ | Q | — | between wolf and fox |
| Hyena | P3 | ✅ | Q | spotted, striped | `examples/spotted_hyena` — approved 21-box spotted hyena with tall heavy shoulders, lower rear, blunt dark muzzle, raised mane, and short brush tail |

## Felids (big cats)

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Lion | P1 | ✅ | Q | male (mane), lioness, cub | `examples/lion` — canonical 23-box male with chest/head mane frame, light feline barrel, textured paws, and attached tufted tail; rounded A/B retained at `legacy-examples/lion_rounded` |
| Tiger | P1 | ✅ | Q | orange, white, cub | `examples/tiger` — canonical 20-box tiger with pixel-textured stripes; `legacy-examples/tiger_rounded` retains the rounded A/B |
| Leopard / Jaguar | P2 | ✅ | Q | spotted, melanistic (black panther) | `examples/jaguar` — approved 23-box adult jaguar with heavy shoulder/hip masses, broad cheeks, short legs, and rosette-patterned coat faces |
| Cheetah | P2 | ✅ | Q | — | `examples/cheetah` — approved 20-box narrow-waisted runner with tall legs, tear-marked face, spotted coat, and paired-leg sprint |
| Lynx / Bobcat | P3 | ☐ | Q | ear tufts | scaled-up cat rig |

## Bears

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Bear | P1 | ✅ | Q | brown/grizzly, black, polar (Arctic), cub | `examples/bear` — canonical 17-box brown bear with vanilla-derived two-mass torso, shoulder hump, broad feet, and tiny tail; rounded A/B retained at `legacy-examples/bear_rounded` |
| Panda | P2 | ✅ | Q | adult, cub | `examples/panda` — approved 17-box giant panda; bear-derived mass with black shoulder band, limbs, ears, and eye patches |
| Polar bear | P2 | ✅ | Q | adult, cub | `examples/polar_bear` — approved 18-box long-bodied adult with narrow head, small ears, oversized snow paws, and slow planted walk |

## Hoofed & large herbivores

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Deer | P1 | ✅ | Q | doe, buck (antlers), fawn (spots) | `examples/deer` — approved 25-box white-tailed buck with a connected sparse antler rig, large ears, pale belly, and white tail flag |
| Moose / Elk | P2 | ✅ | Q | moose, elk | `examples/moose` — approved 30-box bull with tall legs, long muzzle, throat bell, high shoulders, and broad palmate antler rack |
| Elephant | P1 | ✅ | Q | African (big ears), Asian, calf; tusks | `examples/elephant` — canonical box-only African elephant; `legacy-examples/elephant_rounded` retains the rounded A/B and matching articulated trunk walk |
| Giraffe | P2 | ✅ | Q | adult, calf | `examples/giraffe` — approved 22-box adult with extreme spotted neck proportions, mane, large ears, and stepped ossicones |
| Zebra | P2 | ✅ | Q | adult, foal | `examples/zebra` — approved 22-box plains zebra; horse-derived proportions with stripes carried by pixel face textures |
| Rhino | P2 | ✅ | Q | one-horn, two-horn | `examples/rhinoceros` — approved 25-box two-horn adult with plated shoulders, low head, and broad feet |
| Hippo | P2 | ✅ | Q | adult, calf | `examples/hippopotamus` — approved 31-box adult with barrel body, high-set eyes, broad feet, and oversized muzzle; also semi-aquatic |
| Camel | P2 | ✅ | Q | one hump (dromedary), two hump (bactrian) | `examples/camel` — approved 26-box dromedary with stepped hump, long articulated neck, and broad desert feet |
| Bison / Buffalo | P3 | ✅ | Q | — | `examples/american_bison` — approved 27-box bull with layered hump, compact rear, low shaggy head, short horns, and beard |
| Antelope / Gazelle / Impala | P2 | ✅ | Q | horns; springbok, oryx | `examples/gemsbok_oryx` — approved 24-box gemsbok with black-white mask, flank stripe, leg stockings, and twin two-stage spear horns |
| Wildebeest / Gnu | P3 | ✅ | Q | — | `examples/wildebeest` — approved 28-box blue wildebeest with massive dark shoulders, lowered long face, beard, sweeping three-stage horns, and black tail |
| Tapir | P3 | ✅ | Q | Malayan, lowland; striped baby | `examples/malayan_tapir` — approved 23-box adult with a massive pale saddle, black fore and rear masses, white-rimmed ears, broad feet, and short three-stage trunk |
| Okapi | P3 | ✅ | Q | — | `examples/okapi` — approved 24-box adult with a deep chestnut body, shorter giraffe-like neck, huge ears, small ossicones, and white-barred rump and legs |
| Warthog | P3 | ✅ | Q | — | `examples/warthog` — approved 30-box adult with a wide low head, cheek bosses, paired two-stage tusks, stiff dorsal mane, and raised tufted tail |

## Primates

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Monkey | P2 | ☐ | B/Q | generic, with baby | long tail; knuckle/upright ambiguity |
| Gorilla | P2 | ✅ | B/Q | silverback, female, juvenile | `examples/gorilla` — approved 28-box silverback with gray saddle, long articulated arms, and grounded knuckle-walk |
| Chimpanzee | P3 | ✅ | B/Q | — | `examples/chimpanzee` — approved 24-box agile knuckle-walker with pale ears, muzzle, and lighter proportions than the gorilla |
| Orangutan | P3 | ✅ | B/Q | — | `examples/orangutan` — approved 27-box adult male with rust-orange shag, dark cheek flanges, and exceptionally long forelimbs |
| Lemur | P3 | ✅ | Q/B | ring-tailed | `examples/ring_tailed_lemur` — approved 22-box slender quadruped with amber-eyed mask and seven-section banded tail |

## Small & exotic mammals

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Mouse / Rat | P2 | ☐ | Q | — | tiny; long tail |
| Squirrel | P2 | ✅ | Q/H | gray, red; flying squirrel | `examples/red_squirrel` — approved 18-box Eurasian red squirrel with synchronized bound, deep haunches, and three-stage plume tail |
| Rabbit | P1 | ✅ | H | (see Farm) | `examples/rabbit`; rounded A/B in `legacy-examples/rabbit_rounded` |
| Hedgehog | P3 | ✅ | Q | — | `examples/hedgehog` — approved 19-box low quadruped with pointed cream face, tiny legs, and four-step pixel-textured spine coat |
| Porcupine | P3 | ✅ | Q | — | `examples/porcupine` — approved 21-box heavy adult with high pale-tipped quill mantle, small face, sturdy legs, and short thick tail |
| Raccoon | P2 | ✅ | Q | — | `examples/raccoon` — approved 18-box adult with broad face mask, dark paws, cautious walk, and four-stage ringed tail |
| Beaver | P3 | ✅ | Q | — | `examples/beaver` — approved 19-box adult with blunt muzzle, orange incisors, compact legs, and broad crosshatched two-stage paddle tail |
| Bat | P2 | ✅ | W | — | `examples/bat` — approved 19-box flying mammal with large stepped ears, broad three-stage membrane wings, tucked feet, and tail membrane |
| Meerkat | P2 | ✅ | Q | — | `examples/meerkat` — approved 19-box upright sentry with striped torso, dark eye patches, folded forepaws, planted feet, balancing tail, and scanning cycle |
| Otter | P2 | ✅ | Q/S | river, sea | `examples/river_otter` — approved 17-box river otter with elongated body, webbed paws, and two-stage lateral swim tail |
| Red panda | P2 | ✅ | Q | — | `examples/red_panda` — approved 21-box adult with rust coat, white mask and ruff, dark legs, and plush six-section ringed tail |
| Sloth | P3 | ✅ | Q | two-toe, three-toe | `examples/sloth` — approved 20-box low quadruped with masked face, long two-stage forelimbs, bent hind limbs, hooked contact paws, and very slow crawl |
| Capybara | P3 | ✅ | Q | — | `examples/capybara` — approved 16-box tailless barrel with high blunt head, tiny ears, short planted legs, and calm walk |
| Anteater | P3 | ✅ | Q | giant, tamandua | `examples/giant_anteater` — approved 23-box giant anteater with a long three-stage snout, bold shoulder saddle, heavy clawed forefeet, and enormous four-stage plume tail |
| Armadillo | P3 | ✅ | Q | — | `examples/armadillo` — approved 23-box nine-banded adult with stepped armor, pointed head, upright ears, clawed feet, and three-stage plated tail; roll-up remains future work |
| Pangolin | P3 | ✅ | Q | — | `examples/pangolin` — approved 23-box adult with five overlapping scale plates, earless pointed head, clawed feet, and four-stage armored tail; **roll-up** remains future work |
| Mongoose | P3 | ✅ | Q | — | `examples/mongoose` — approved 19-box low runner with a narrow speckled body, pointed muzzle, small ears, dark feet, and three-stage tail |

## Marsupials & monotremes

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Kangaroo | P2 | ✅ | H | adult, joey-in-pouch | `examples/kangaroo` — approved 24-box red kangaroo with synchronized hop, long feet, and two-stage balancing tail |
| Koala | P3 | ✅ | Q | adult, joey | `examples/koala` — approved 21-box upright adult with oversized stepped ears, broad dark nose, pale belly, folded long arms, planted feet, and cling idle |
| Wallaby | P3 | ☐ | H | — | smaller kangaroo rig |
| Wombat | P3 | ✅ | Q | — | `examples/wombat` — approved 17-box common wombat with broad head, low barrel, tiny ears, short powerful legs, wide clawed paws, and slow walk |
| Opossum | P3 | ✅ | Q | — | `examples/opossum` — approved 24-box adult with white pointed face, black-pink ears, pink feet and nose, and six-stage bare prehensile tail |
| Tasmanian devil | P3 | ✅ | Q | — | `examples/tasmanian_devil` — approved 23-box black adult with red inner ears, white chest and shoulder marks, oversized jaw, short strong legs, and thick tail |
| Platypus | P3 | ✅ | Q/S | — | `examples/platypus` — approved 15-box low swimmer with broad slate bill, four webbed feet, two-stage paddle tail, and alternating paddle cycle |
| Echidna | P3 | ✅ | Q | — | `examples/echidna` — approved 18-box short-beaked adult with domed spine coat, elongated snout, broad clawed feet, and compact digging walk |

## Birds

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Chicken | P0 | ✅ | W | (see Farm) | `examples/chicken`; rounded A/B in `legacy-examples/chicken_rounded` |
| Toucan | P3 | ✅ | W | — | `examples/toucan` — approved 16-box toco toucan with enormous multicolor bill, white throat, blue eye ring, layered wings, and fast flap |
| Owl | P1 | ✅ | W | brown, snowy (white) | `examples/owl` — approved 15-box great horned owl with facial disc, layered wings, and tucked talons |
| Parrot | P1 | ✅ | W | red, green, blue, yellow morphs | `examples/parrot` — approved 15-box scarlet macaw with hooked beak, saturated wing bands, and long tail |
| Eagle / Hawk | P2 | ✅ | W | bald eagle (white head), hawk | `examples/eagle` — approved 15-box bald eagle with broad wings, white head/tail, hooked beak, and talons |
| Penguin | P2 | ✅ | B | emperor, chick | `examples/penguin` — approved 12-box emperor penguin with hanging flippers, broad webbed feet, and lateral body waddle |
| Duck | P2 | ✅ | W/S | (see Farm) | `examples/mallard_duck`; approved drake in Farm section |
| Pigeon / Dove | P3 | ☐ | W | — | |
| Crow / Raven | P3 | ✅ | W | — | `examples/raven` — approved 18-box common raven with heavy beak, throat shag, broad layered wings, wedge tail, and measured soar |
| Flamingo | P3 | ✅ | W | — | `examples/flamingo` — approved 17-box greater flamingo with angular S-neck, two-stage stilt legs, and slow planted walk |
| Songbird (robin/sparrow) | P3 | ☐ | W | color morphs | generic small-bird base |
| Peacock | P3 | ✅ | W | — | `examples/peacock` — approved 22-box Indian peacock with crest, patterned wings, and parented seven-feather tail fan |
| Ostrich / Emu | P3 | ✅ | B | — | `examples/ostrich` — approved 21-box male ostrich with tiny white-edged wings, long bare neck, two-stage legs, broad feet, and fast run |

## Marine & aquatic

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Fish (generic) | P1 | ✅ | S | clownfish, tropical morphs, cod, salmon, pufferfish | `examples/fish` — approved 10-box blue/yellow tropical fish with a lateral two-stage tail |
| Shark | P1 | ✅ | S | great white, hammerhead | `examples/shark` — approved 16-box great white with gills, dorsal/pectoral fins, and a two-lobe lateral tail |
| Dolphin | P1 | ✅ | S | — | `examples/dolphin` — approved 13-box bottlenose dolphin with vertical propulsion and horizontal flukes |
| Whale | P2 | ✅ | S | orca, humpback, blue | `examples/orca` — approved 15-box adult orca with bright eye and saddle patches, continuous white underside, tall dorsal fin, rear-swept pectorals, and horizontal flukes |
| Octopus | P2 | ☐ | S | — | 8 articulated cuboid tentacles |
| Crab | P2 | ☐ | C | — | sideways multi-leg |
| Sea turtle | P2 | ☐ | S | adult, hatchling | also Reptiles |
| Seahorse | P3 | ☐ | S | — | |
| Jellyfish | P3 | ☐ | S | — | pulse animation |
| Starfish | P3 | ☐ | ST | — | nearly static |
| Lobster / Shrimp | P3 | ☐ | C/S | — | |
| Seal / Sea lion | P3 | ✅ | S/Q | harbor seal, sea lion | `examples/harbor_seal` — approved 12-box harbor seal with a tapered spotted body, earless head, pale whiskered muzzle, short foreflippers, and paired hind flippers |
| Manatee / Dugong | P3 | ✅ | S | West Indian manatee, dugong | `examples/manatee` — approved 11-box West Indian manatee with a massive stepped body, blunt whiskered muzzle, paddle flippers, and broad horizontal spoon tail |

## Reptiles & amphibians

| Animal | Pri | Status | Body | Variants to consider | Notes |
|---|---|---|---|---|---|
| Snake | P2 | ✅ | SL | green, brown, cobra (hood), rattlesnake | `examples/king_cobra` — approved 15-box king cobra with raised patterned hood, forked tongue, and seven-stage slither chain |
| Lizard / Gecko | P2 | ☐ | Q | gecko, iguana, chameleon | small sprawled quadruped |
| Turtle / Tortoise | P2 | ✅ | Q/S | land tortoise, sea turtle, hatchling | `examples/sea_turtle` — approved 11-box green sea turtle with stepped shell and four swimming paddles |
| Crocodile / Alligator | P2 | ✅ | Q | croc, gator | `examples/crocodile` — approved 24-box Nile crocodile with armored back, long jaw, sprawled walk, and articulated tail |
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
| Bearfolk | — | ✅ | `examples/bearfolk` — canonical 15-box player-derived biped with vest, belly patch, broad head/muzzle, and paw textures; rounded A/B retained at `legacy-examples/bearfolk_rounded` |
| Lionfolk | — | ✅ | `examples/lionfolk` — canonical 20-box player-derived biped with tunic, mane frame, and attached articulated tail; rounded A/B retained at `legacy-examples/lionfolk_rounded` |
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

- **Canonical box-only figures (20):** bear, bearfolk, butterfly, cat, chicken,
  cow, dog, elephant, fox, goat, horse, lion, lionfolk, piglet, player, rabbit,
  sheep, tiger, upright bear, and wolf.
- **Rounded-to-box migration complete (13):** piglet, sheep, dog, cat,
  bearfolk, lionfolk, cow, horse, goat, wolf, fox, bear, and lion.
- **Retained rounded A/B archive:** all 18 former mixed-primitive sources live
  under `legacy-examples/` and remain schema-round-trip tested.
- **Macros ready:** `quadrupedWalk` (Q), `bipedWalk` (B), `wingFlap` (W).
- **Macros to build:** `swim` (S), `slither` (SL), `hop` (H — rabbit currently approximates it), N-leg crawl (C).
- **Biggest single unlock:** the `swim` macro — gates the entire marine family.
- **Lowest-effort wins next:** Deer and zebra can reuse the reviewed
  horse-class rig; panda and polar bear can reuse the canonical bear.
- **Cheapest enemy wave:** humanoid baddies (skeleton, zombie, humanoid pig) — all reuse the `player`/`-folk` biped rig + `bipedWalk`, no new tooling.
