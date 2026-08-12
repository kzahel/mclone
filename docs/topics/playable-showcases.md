# Playable Showcases

Topic: `playable-showcases`

Status: implemented with two publicly verified showcases 2026-08-12.
`mallard-ecology` proves a wetland motion/hatching/discovery loop;
`deer-forest-edge` proves a contrasting multi-chunk habitat, authoritative
named actions, durable sign, field notes, and an ordinary hunting tool. Both
compile checked data recipes into transient tiny saves through the shared
native and Web game path.

## Purpose

Playable showcases pair visual evidence with a link that a reviewer can open
on another device. They are small, resettable save games: authored enough to
make one feature easy to see, but otherwise governed by the real integrated
server, client replica, interaction, persistence-record, UI, audio, and render
systems.

They are review artifacts, not a second content runtime. A showcase may make a
rare or time-dependent result immediately visible. It may not be the only way
that result can occur in an ordinary live world.

The first public link is:

```text
https://mclone.kzahel.com/app.html?showcase=mallard-ecology
https://mclone.kzahel.com/app.html?showcase=deer-forest-edge
```

Opening it compiles a fresh in-memory world in the browser process. No world is
read from or written to IndexedDB, and refresh starts again from the recipe.

## Contract

The shared compiler lives in `mclone-server`. A checked-in JSON recipe may
describe only bounded initial saved state:

- one allowlisted authored base terrain;
- seed, frozen time, and a player entry pose;
- small allowlisted block patches;
- persisted entities, stable symbolic relationships, and species state; and
- bounded player inventory and durable observations.

The compiler validates the recipe, derives stable persistent entity identities,
and writes the same ordinary world, dimension, chunk, entity-chunk, and player
records that normal persistence hydrates. Native capture and Web startup both
consume that compiler. Platform code may parse `showcase=<id>` and select a
recipe, but it does not author the scene.

The schema deliberately has no updates, scripts, triggers, timers, behavior
trees, dialogue, commands, spawn rules, or rendering instructions. Behavior
after hydration must already belong to the live game.

A showcase has two independent acceptance dimensions. A captured first frame
proves composition, rendering, and entry-camera identity. A time window proves
that creatures and mechanics remain useful after that frame. When the subject
is behavioral, both are required; pixel parity alone is not acceptance.

## Live-Instantiation Evidence

Every gameplay-bearing recipe fact must declare `liveInstantiation`. Validation
rejects a missing, unknown, or subject-incompatible evidence ID. The typed
registry in `mclone-server::playable_showcase` records:

- a stable evidence ID;
- the ordinary live-game producer;
- a test or production-contract review anchor; and
- the exact kind of fact that evidence may justify.

For `mallard-ecology`, the mapping is:

| Showcased fact | Ordinary live-game source |
|---|---|
| lily pad | Overworld waterlily random-patch placement |
| adult mallard | habitat-qualified natural wetland spawning |
| duckling | attended nest hatching |
| nest | using an egg on a valid covered wetland shore |
| carried egg | adult wetland egg laying and pickup |
| carried feather | periodic shedding and pickup |
| field-note observations | proximity, call, trace, pickup, nest, and hatch observation paths |

This is enforced at the recipe boundary, rather than being only a checklist.
The registry cannot mathematically prove that a producer remains well-designed
or balanced; its ordinary-world contract test and code-review anchor remain the
maintenance proof. Removing a live producer requires removing or replacing its
showcase evidence in the same change.

Terrain is a narrow exception only for the allowlisted base fixture itself.
Gameplay-significant terrain patches still require evidence. A large raw block
palette must not be used to disguise missing world generation, structures, or
features.

## Boundedness and Anti-Sprawl Rules

- Keep recipes data-only and readable. Current hard limits are 256 block
  patches, 64 entities, a 64-byte symbolic entity ID, one bounded authored
  base fixture, and narrow item/entity/observation allowlists.
- Do not add showcase checks to gameplay ticks, entity AI, interaction,
  protocol, UI, audio, or render code. Once loaded, the world must be ordinary.
- Do not introduce an item, entity, observation, terrain feature, or behavior
  solely for a showcase. Land its shared live-world producer and tests first or
  in the same slice.
- Prefer composition from existing saved facts. A requested dynamic sequence
  belongs in real gameplay, or in a separately designed scenario framework;
  it does not justify turning this format into a scripting language.
- Add schema capability only when multiple likely showcases need it and the
  representation is still an ordinary persistence fact. A one-off field is a
  warning that the review scene is leaking into product code.
- Keep each showcase focused on one coherent vertical slice. If a recipe stops
  being visually inspectable or its evidence table stops being understandable,
  split or retire it rather than raising limits casually.
- A tactical cannot call a showcase complete while any showcased gameplay fact
  lacks a compatible registered live-instantiation path.

## Adding a Showcase

1. Land or identify every ordinary live-world producer and its focused test.
2. Add stable typed evidence entries for the exact fact kinds being composed.
3. Add a deny-unknown-fields recipe under `assets/mclone/showcases/`, keeping
   it within the current schema and hard limits.
4. Register the ID and embedded recipe in `PlayableShowcaseId`; do not add a
   platform-local content catalogue.
5. Compile it through the shared server and add deterministic state assertions.
6. Add or generalize native and browser capture assertions without teaching
   the renderer or gameplay code about the showcase ID.
7. Inspect the first native and Web pixels. Push, let the established deploy
   route publish the exact revision, then run the deployed smoke and inspect
   its screenshot before sharing the clean URL.
8. Update this topic, the governing gameplay topic, and the tactical execution
   record with the evidence IDs, commands, receipt, and remaining gaps.

For behavior-focused showcases, insert a time-window gate between steps 6 and
7. Observe ordinary replicated state without issuing commands to the subject.
Require domain outcomes such as displacement, habitat transition, life-cycle
change, or discovery; do not substitute elapsed wall time or a second static
image for those outcomes. When presentation depends on replicated domain state,
assert that state too: Tactical 283 added nonzero water occupancy to the
mallard gate after the first deployed probe exposed a dropped incremental
`in_water` update.

The current first-showcase commands are:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server playable_showcase
pnpm native:mallard-ecology:capture
pnpm native:web:showcase-smoke
pnpm native:web:showcase-deployed-smoke
```

The deer commands are:

```bash
pnpm native:deer-forest-edge:capture
pnpm native:web:deer-showcase-smoke
pnpm native:web:deer-showcase-deployed-smoke
```

Captures belong under `/tmp`. The deployed smoke must confirm the recipe ID and
revision, seed, entry camera, entity composition, gameplay facts, credible
pixels, and zero records in every browser persistent-world store.

## First Verified Evidence

The 2026-08-12 acceptance used recipe revision 1, seed `17502`, entry eye
`8.5,67.62,7.5`, entry target `8,65,1.5`, four entities, three mallards, one
nest, and all six field notes. Native flat, synthetic stereo, local Web, and
deployed Web captures were inspected.

The local and deployed Web screenshots had the identical SHA-256 digest
`ebe04116774cdf19cfa8c9fb092f4e1a0a432bee918a111ab6c5d3f6157836ea`.
The deployed browser probe reported zero records in `dimensionChunks`,
`dimensionEntityChunks`, `dimensions`, `players`, `savedData`, `worldMetadata`,
`managedWorlds`, and `worlds`.

That revision remains a useful historical static-parity proof, but its human
interactive review was rejected: the mallards mostly stayed at their initial
water cells and rapidly changed yaw. Tactical
[`282`](../tactical/282-mallard-behavioral-showcase.md) records the correction.

Revision 2 uses seed `17503`, entry eye `8.5,66.62,18.5`, and entry target
`8,64.9,7.5`. It begins with three mallards, one attended nest at 2320/2400
incubation, one of six persisted observations, and no pre-awarded egg or
feather items. The bounded authored wetland fills all 49 chunks of the existing
seven-by-seven fixture envelope; its shallow pond and grassy banks cross chunk
boundaries.

Local and public headed Web acceptance each observed 80 authoritative ticks.
The three initial mallards displaced `3.33`, `2.21`, and `3.59` blocks, the
ordinary nest system hatched a fourth mallard, and field notes advanced from
two to five observations through live proximity/call/track/hatch paths. All
browser world stores remained empty. Exact pushed revision
`25ea0d2efb44513aacb922fd5cbb9049a61207ff` passed the deployed probe; its
inspected 1600x900 canvas digest was
`c9e0ee0e64f507ab21b844650bbc099306a772090f61136e4761f33c12c040dc`.

Tactical [`283`](../tactical/283-mallard-waterline-presentation.md) followed
human review of those pixels: the ducks moved correctly but stood on the water
because incremental mallard metadata was dropped by the ordinary client
replica. Exact deployed revision `acee6205` repairs that contract and applies a
shared swim-only visual waterline. Its stricter public gate reports three
swimming mallards at both ends of the 80-tick window, and its inspected canvas
digest is
`d6967d161f532f532ec7c374432a25a6bb3eaca228a0ed4493fc563681e0201a`.

Tactical
[`284`](../tactical/284-deer-forest-edge-ecology-and-semantic-props.md) adds
recipe revision 1, seed `17504`, entry eye `8.5,66.62,29.5`, and entry target
`4,65.8,10`. The recipe contains three deer, one durable bed sign, one starter
hunting spear, and two initial deer observations across the existing
seven-by-seven authored-fixture envelope.

Its typed evidence registry binds those facts to ordinary natural forest-edge
spawning, repeated-rest sign production, starter spear inventory, and player
field-note producers. Post-hydration behavior, named clips, sound, damage,
drops, and field-note advancement are shared live mechanics; the recipe has no
script or showcase-specific update.

Local and public headed Web acceptance each observed 80 authoritative ticks.
One deer displaced `11.2` blocks, the herd presented `alert` and `flee` states
and clips, field notes reached 3/6, four semantic actors drew, and all eight
browser world stores remained empty. Exact pushed revision
`8482f69ce606437286f655f324d93da58084f525` passed Worker version
`dcad575a-972a-4adb-bece-7acfd79674d6`; the inspected 1600x900 canvas digest is
`b2ce39d582463e76f717ea1a2be3c2ca3c3d6ee3c51668143aec8f9852156048`.

A post-review locomotion correction strengthened the behavior gate beyond
authoritative movement. The Web report now exposes the last presentation
distance per stable deer entity ID, and acceptance requires at least one
rendered distance phase to advance by more than `0.5` during the same window.
The repaired local run observed `11.1524` units while its subject moved `11.2`
blocks. This prevents a moving actor with a visually restarted or frozen leg
loop from passing on displacement alone.

Phone review then exposed a second acceptance gap: touch-capable clients use
larger hotbar hit targets, but those slots showed only `1` through `9` because
their overlay had not received the canonical inventory. The shared HUD now
projects the ordinary hotbar contents into the touch layout. The reusable Web
runner accepts `--mobile-showcase`, verifies visible touch controls at a
390x844 CSS-pixel viewport, and writes distinct mobile showcase captures.
The shared HUD also writes the selected item's full name above either hotbar,
so a compact label such as `SP` does not require prior knowledge.
`pnpm native:web:deer-showcase-mobile-smoke` is required when the reviewed
mechanic depends on touch-only controls or inventory selection.

The first public probe encountered the prior cached `app.html` and rejected it
because that binary knew only `mallard-ecology`. Verification resumed only
after the HTML version, JavaScript, and Wasm payload matched the new publish.
This is expected evidence that a successful deploy process is not itself the
acceptance gate.

## Code and Documentation Map

- `assets/mclone/showcases/`: readable showcase recipes
- `native/crates/mclone-server/src/playable_showcase.rs`: schema, evidence
  registry, validation, deterministic compilation, and unit tests
- `native/crates/mclone-server/src/bin/mallard_ecology_fixture.rs`: native
  tiny-save materialization receipt
- `native/crates/mclone-server/src/bin/deer_forest_edge_fixture.rs`: deer
  tiny-save materialization receipt
- `native/apps/mclone-web-client/src/web_canvas.rs`: URL selection and strict
  transient/conflict policy
- `native/apps/mclone-web-client/src/web_server_worker.rs`: worker-side shared
  compilation into an in-memory store
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`: local and deployed
  browser state, persistence, and pixel proof
- `scripts/mallard-ecology-capture.mjs`: native flat/stereo receipt and capture
- `scripts/deer-forest-edge-capture.mjs`: deer native flat/stereo receipt and
  capture
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md):
  ordinary mallard habitat and gameplay contract
- [`../tactical/280-playable-showcase-links.md`](../tactical/280-playable-showcase-links.md):
  first implementation execution record
- [`../tactical/282-mallard-behavioral-showcase.md`](../tactical/282-mallard-behavioral-showcase.md):
  rejected baseline, shared AI correction, wetland revision, and timed proof
- [`../tactical/283-mallard-waterline-presentation.md`](../tactical/283-mallard-waterline-presentation.md):
  replicated swim-state repair and shared waterline presentation proof

## Recommended Next Work

- Promote a contrasting creature whose live mechanic requires a new terrain
  capability, then decide whether it merits a second focused showcase.
- Generalize repeated capture assertions only where the two existing
  showcases now demonstrate a stable common shape. Keep species-specific
  behavioral outcomes explicit rather than flattening them into elapsed-time
  or actor-count checks.
- Keep deployed URLs as review links, not a permanent menu or public catalogue,
  until there is a product reason and an explicit lifecycle policy for one.
