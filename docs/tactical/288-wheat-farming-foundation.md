# Tactical 288: Wheat Farming Foundation

Status: **active 2026-08-13.**

Topics:

- `wheat-farming`
- `playable-showcases`
- `starter-farmstead-settlement`

## Instruction Synthesis

Continue the creature, world-generation, and gameplay workstream with the
first real crop system. Explain field creation as player-made farmland rather
than generated decorative fields, then implement wheat farming end to end:
record the plan, land ordinary live-world mechanics and persistence, provide a
phone-usable data-only showcase, inspect rendered evidence, commit logical
slices, deploy the exact revision, and share the matching interactive URL.

## Objective

Establish the first complete renewable plant-resource loop:

```text
grass or dirt + hoe + open space
  -> dry farmland
  -> nearby water hydrates it
  -> wheat seeds become an age-0 crop
  -> active loaded-world random ticks advance visible crop ages
  -> mature harvest yields wheat plus enough seeds to continue
  -> the edited field, crop ages, and inventory survive save/reopen
```

This is the first meaning of **field creation**: a player chooses a suitable
piece of ordinary terrain and makes a working field through tools, water,
planting, time, and harvest. Naturally generated village farms, terrain-led
field placement, crop breadth, and settlement integration become later
consumers of the same mechanics rather than substitutes for them.

## Reference Contract And Deliberate Scope

Minecraft Java 1.17.1 `HoeItem`, `FarmBlock`, `CropBlock`, `Blocks.WHEAT`, and
the wheat loot table are the parity references. Preserve their useful semantic
shape:

- using a hoe on grass or dirt with air above creates moisture-0 farmland;
- farmland owns moisture ages `0..=7`, hydrates from water within four blocks,
  dries one step at a time without water, and becomes dirt when fully dry and
  no crop protects it;
- wheat owns growth ages `0..=7`, may be planted only above farmland, and uses
  the vanilla 3x3 farmland growth-speed calculation including hydrated-soil
  and dense-row penalties;
- randomly ticking blocks advance only in loaded block-ticking chunks;
- immature wheat drops one seed, while mature wheat drops one wheat plus the
  vanilla no-Fortune binomial extra-seed result; and
- farmland and every wheat age use their actual extracted 1.17.1 blockstate,
  model, texture, outline, collision, light, and cutout facts.

The first wooden hoe is intentionally reusable. The current item-stack schema
has kind and count but no damage component; silently storing durability in a
showcase or unrelated player field would create the wrong foundation. A later
item-component slice should add durable tool damage for every applicable item,
then make the hoe consume one durability per successful till as the reference
does. Rain hydration, farmland trampling, bone meal, Fortune, bread/crafting,
villager farming, and generated village fields are also explicit follow-ups.

## Shared Ownership

- `mclone-worldgen` and `mclone-assets` name the compact runtime block states;
  this slice adds eight farmland states followed by eight wheat states without
  changing reference Overworld generation output.
- `mclone-blocks` owns physical and selection facts. Farmland is a 15/16-high
  solid support; wheat has age-relative outline height and no collision.
- `mclone-server` owns tilling, planting, random ticking, hydration, growth,
  harvest loot, inventory mutation, persistence-visible block edits, and the
  showcase compiler. Keep the pure crop decisions in a focused module rather
  than growing the integrated-session file into the farming model.
- `mclone-protocol` carries only neutral item kinds and existing block deltas;
  no crop-specific client command or showcase command is permitted.
- `mclone-ui`, scene, meshing, and rendering consume existing shared item,
  terrain, and interaction contracts across desktop, Web, Android, and XR.
- App code may parse the showcase ID and drive generic capture controls. It
  may not own hoe, planting, growth, or harvest semantics.

## Interaction Contract

Add ordinary item kinds for `WoodenHoe`, `WheatSeeds`, and `Wheat`.
Fresh-player inventory supplies one hoe and a bounded starter seed stack until
crafting and grass-drop acquisition are implemented. Both desktop and touch
hotbars expose the same stacks and selected-item name.

Using the hoe must pass the existing authoritative reach, topology, behavior,
and line-of-use validation. The clicked face may not be down, the block must be
grass or dirt, and the block above must be air. A successful use changes only
that block to dry farmland.

Using wheat seeds on farmland places age-0 wheat into clear air above and
consumes exactly one seed. Breaking wheat follows the existing authoritative
block-action path: an immature crop restores one seed, and a mature crop adds
one wheat plus the bounded reference seed roll. Drops enter the real player
inventory so this first loop does not depend on authoring three new item-entity
figures; inventory publication and persistence remain ordinary.

## Random Tick And Persistence Contract

Introduce one reusable random-block-tick phase driven by the dimension seed,
game time, chunk/section coordinates, and the vanilla default speed of three
candidates per non-empty section. It runs only for the scheduler's current
block-ticking chunks and selects candidates independently of showcase identity.
The phase initially dispatches farmland and wheat; later random-ticking flora
can join the same registry-shaped function.

Random-tick queue position is not persisted in vanilla and will not be
persisted here. The durable facts are the resulting farmland moisture and
wheat age block states, which already flow through ordinary chunk edit records.
Save/reopen must preserve those exact states and item stacks. Unloaded fields
do not catch up or reroll hidden progress.

## Showcase Contract

Add a `wheat-farming` data recipe under the bounded tiny-save schema. It may
compose a readable water-fed plot containing:

- a few grass/dirt cells for tilling;
- hydrated and dry farmland states;
- wheat at several ages including a mature harvest target;
- a carried wooden hoe and wheat seeds; and
- a camera position from which soil, water, and crop stages are legible.

Every patch and item needs typed ordinary-world instantiation evidence. The
recipe may not accelerate ticks, refill seeds, script interactions, change
loot, or own any farming behavior. After hydration it is the same integrated
simulation as a normal world and remains an in-memory, refresh-reset tiny save.

Acceptance requires generic commands to prove, on both desktop and phone:

1. select the hoe and till one ordinary grass/dirt cell;
2. select seeds, plant that new farmland, and observe the seed count fall;
3. observe at least one automatic farmland hydration or wheat-age transition;
4. harvest a mature crop and observe real wheat/seed inventory gain; and
5. retain zero browser persistence records.

The inspected screenshot and public URL must use the recipe's same seed,
entry pose, exact deployed revision, and unmodified gameplay code.

## Implementation Slices

1. **Record the contract.** Add this tactical, the focused living topic, and
   series log entry.
2. **Add crop content.** Register farmland/wheat states and exact asset,
   render, light, shape, collision, and meshing facts; inspect the first crop
   pixels before proceeding.
3. **Add player items and actions.** Extend item codecs/persistence/UI, starter
   inventory, authoritative hoe/seed use, mature/immature harvest, and tests.
4. **Add loaded-world ecology.** Implement deterministic random-block sampling,
   hydration/drying, vanilla-shaped growth speed, support cleanup, publication,
   and focused determinism/bounds tests.
5. **Prove durability.** Save and reopen tilled soil, moisture, crop age,
   harvest output, remaining seeds, and selected hotbar state.
6. **Build the showcase.** Add typed evidence, a bounded data recipe, generic
   desktop/mobile interaction gates, and inspected native/Web pixels.
7. **Close and deploy.** Update farming, showcase, farmstead, and platform
   records; run proportional tests; push; deploy the exact revision; rerun the
   public gates; and share the inspected screenshot plus transient URL.

## Human Review Points

- **Review A — block readability:** dry/hydrated soil and wheat ages must be
  distinguishable in an actual game capture without labels painted onto the
  world.
- **Review B — interaction feel:** till, plant, and harvest must all be usable
  through the normal crosshair/`USE`/`ATK` controls on a phone-sized viewport.
- **Review C — final loop:** the deployed resettable field should let a reviewer
  repeat the loop and see why water placement and maturity matter.

The currently authorized autonomous run may inspect and pass these points. It
must stop rather than hide a substantive failure behind fixture composition.

## Validation

At minimum:

- exact blockstate asset validation and terrain mesh/facts/shape tests;
- protocol codec, item-stack, inventory, persistence, and UI tests;
- focused till/plant/hydrate/dry/grow/harvest/support tests;
- a save/reopen integration proof for field and inventory state;
- Mclone/Java Overworld generator non-regression evidence;
- `cargo fmt --all --check` and relevant shared-crate tests;
- an inspected native first drawable milestone;
- local headed-WebGPU desktop and mobile farming gates;
- pushed/deployed exact-revision desktop and mobile gates; and
- this execution record plus living-topic status, evidence, hashes, and known
  gaps.

## Execution Record

Implementation pending.
