# Playable Showcases

Topic: `playable-showcases`

Status: implemented and first publicly verified 2026-08-12. The first
showcase, `mallard-ecology`, compiles one checked-in data recipe into an
ordinary transient tiny save and opens at a fixed camera through the shared
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
  island, and narrow item/entity/observation allowlists.
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

The current first-showcase commands are:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server playable_showcase
pnpm native:mallard-ecology:capture
pnpm native:web:showcase-smoke
pnpm native:web:showcase-deployed-smoke
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

## Code and Documentation Map

- `assets/mclone/showcases/`: readable showcase recipes
- `native/crates/mclone-server/src/playable_showcase.rs`: schema, evidence
  registry, validation, deterministic compilation, and unit tests
- `native/crates/mclone-server/src/bin/mallard_ecology_fixture.rs`: native
  tiny-save materialization receipt
- `native/apps/mclone-web-client/src/web_canvas.rs`: URL selection and strict
  transient/conflict policy
- `native/apps/mclone-web-client/src/web_server_worker.rs`: worker-side shared
  compilation into an in-memory store
- `native/apps/mclone-web-client/scripts/browser-smoke.mjs`: local and deployed
  browser state, persistence, and pixel proof
- `scripts/mallard-ecology-capture.mjs`: native flat/stereo receipt and capture
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md):
  ordinary mallard habitat and gameplay contract
- [`../tactical/280-playable-showcase-links.md`](../tactical/280-playable-showcase-links.md):
  first implementation execution record

## Recommended Next Work

- Generalize the capture command's expected receipt from the catalogue manifest
  when a second showcase creates real demand; avoid speculative framework work.
- Promote a contrasting creature whose live mechanic requires a new terrain
  capability, then decide whether it merits a second focused showcase.
- Keep deployed URLs as review links, not a permanent menu or public catalogue,
  until there is a product reason and an explicit lifecycle policy for one.
