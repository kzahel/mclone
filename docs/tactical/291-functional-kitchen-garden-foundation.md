# Tactical 291: Functional Kitchen Garden Foundation

Status: **active 2026-08-13.**

Topics:

- `functional-kitchen-gardens`
- `wheat-farming`
- `starter-farmstead-settlement`
- `structure-lab`
- `playable-showcases`

## Instruction Synthesis

Continue the garden direction before promoting rabbits. Build the prerequisites
as real shared gameplay rather than scenery: remove the compact block-state
ceiling, add functional connected fences and gates, generalize wheat into a
reusable crop system, add carrots as the first vegetable, author a source-first
kitchen garden, and place the same mechanics in the ordinary intro homestead.
Record the plan first, implement and validate it end to end, commit logical
slices, deploy the exact revision, and provide a phone-usable interactive
review scene. Rabbits and crop raiding remain the next chapter.

## Objective

Turn the existing decorative garden idea and single-crop field foundation into
a small working place:

```text
wide exact block-state identity
  -> connected fence boundary + usable gate
  -> irrigated reusable crop beds
  -> wheat and directly planted carrots
  -> visible ordinary harvest drops and renewal
  -> source-first Structure Lab kitchen garden
  -> terrain-matched ordinary homestead garden
  -> future rabbit attraction, raids, protection, and breeding
```

The result must already be useful without rabbits. The player can understand
the enclosure, open it, enter, harvest vegetables, collect them, and replant.
The later rabbit slice should consume these facts rather than inventing a
rabbit-only fence, crop, or fixture implementation.

## Reference Contract And Deliberate Scope

Minecraft Java 1.17.1 `CrossCollisionBlock`, `FenceBlock`, `FenceGateBlock`,
`CropBlock`, `CarrotBlock`, `WalkNodeEvaluator`, and the carrot loot table are
the behavior references. Preserve their useful semantic shape:

- an oak fence owns independent north/east/south/west connections plus its
  waterlogged fact, connects to compatible fences, correctly oriented gates,
  and sturdy full faces, and refreshes both sides after neighboring edits;
- fence selection follows the visible post and arms while collision rises to
  1.5 blocks, so players and ordinary ground creatures cannot jump it;
- an oak gate owns horizontal facing, open, powered, and in-wall facts; manual
  use toggles it and may reverse its facing when opened from behind;
- a closed gate collides to 1.5 blocks and blocks land navigation, while an
  open gate has no collision and is pathfindable;
- carrots share farmland support, light, random-tick growth speed, and ages
  `0..=7` with other crops, but use their shorter reference outline heights;
- a carrot is both the planting item and harvest item; immature harvest yields
  one carrot and mature no-Fortune harvest yields one guaranteed carrot plus
  the reference binomial extra-carrot stack; and
- all mutations are normal chunk state, item entities, inventory, protocol,
  persistence, render, collision, navigation, and sound behavior.

The gate retains a persisted `powered` property so exact identities are not
aliased, but redstone signal production is not introduced by this tactical.
Waterlogged fence states are represented and rendered; general concurrent
fluid restoration and bucket interaction remain owned by the broader fluid
and item systems. Fence leads, crafting recipes, tool durability, food/hunger,
composters, bone meal, Fortune, potatoes, beetroot, irrigation channels beyond
existing water, and villager farming are explicit follow-ups.

## Block-State Capacity Contract

The generated/live raw block lane is an interim identity mapping, but it may no
longer be byte-sized. Widen `RawBlockId` and every job/worker transport that
actually carries raw blocks to `u16` little-endian elements. Keep canonical
`BlockStateId(u32)`, palette-packed snapshots, protocol block updates, and
persisted chunk records unchanged.

Acceptance must prove at least one state above 255 through:

- generated/mutable raw storage and access;
- native and browser worldgen job codecs;
- raw-to-canonical snapshot conversion;
- authoritative mutation and persistence restart; and
- textured mesh and collision lookup.

No implicit truncating cast, byte reinterpretation, or state alias is allowed.
Existing raw IDs `0..=236` stay stable; the new family begins at 237. The
interim lane remains bounded to `u16`, not a permanent replacement for a
registry-backed palette design.

## Shared Ownership

- `mclone-worldgen` owns the widened raw identity lane, fence/gate/carrot state
  constants and property helpers, canonical names, transforms, and source-first
  structure material vocabulary.
- `mclone-assets` owns exact blockstate/model/texture resolution and the
  proprietary-free first-party visual inventory. Reference-pack rendering
  uses extracted 1.17.1 assets; first-party carrot stages are Texture Lab
  sources, while fence and gate geometry reuse ordinary model baking with the
  active oak-plank texture.
- `mclone-blocks` owns exact multi-box outline and collision facts. It must
  support the five-box connected fence shape rather than collapsing it to a
  full cube.
- `mclone-server` owns placement orientation, neighbor connection refresh,
  gate use, generic crop decisions, carrot loot, item entities, navigation
  invalidation, persistence-visible block edits, homestead materialization,
  and the bounded showcase compiler.
- `mclone-protocol`, `mclone-ui`, `mclone-audio`, `mclone-scene`, and shared
  rendering own neutral carrot/fence/gate items, labels, confirmed feedback,
  existing wood-creak/wood-placement sound families, and all-host presentation.
- Structure Lab owns the TypeScript source recipe and generated canonical JSON.
  The ordinary homestead consumes that promoted content through its shared
  structure/landscape path; it may not keep an unrelated hand-authored copy.
- App crates may select a showcase and drive generic capture controls only.
  They may not own garden gameplay or alter it for review.

## Fence And Gate Interaction Contract

Add ordinary item kinds for oak fence and oak fence gate. Placement uses the
authoritative `UseItemOn` reach, topology, protection, collision, and inventory
path. A placed fence derives all four connections from realized neighboring
blocks. A placed gate faces the player and presents across the fence line.
Every accepted placement, removal, or gate transition recomputes only the
edited cell and its four horizontal neighbors, publishes exact block deltas,
marks nearby creature paths for recomputation, and persists the resulting
states.

Gate use is a first-class block interaction before generic placement. It does
not consume the held item. The authoritative state transition must produce
confirmed wood feedback and one block delta. An open gate is truly traversable
to player collision and ground navigation; a closed gate and connected fence
are true barriers rather than visual models over full-air physics.

The starter inventory may use currently empty hotbar positions for one bounded
fence stack, one gate, and carrots until crafting/loot acquisition exists. The
same item names and slots must be visible and selectable on desktop, phone,
Android, and XR through the shared hotbar.

## Reusable Crop Contract

Replace wheat-named decisions with a small `CropKind`/crop-spec vocabulary.
Shared code determines state-to-kind/age, support, maximum age, growth speed,
random-tick transition, planting item, and harvest stacks. Wheat keeps its
existing output and visuals exactly. Carrots join through data/typed matching,
not a second copy of the hydration and growth algorithm.

The generic growth-density penalty compares neighboring crops of the same
kind, matching `CropBlock.getGrowthSpeed`. Removing farmland support cleans up
either crop. Harvest always removes the crop first and spawns ordinary item
entities even when inventory is full. Replanting a carrot consumes one carrot
and immediately shows a green age-zero plant without requiring target text.

## Structure Lab And Ordinary Homestead Contract

Author `farmstead-kitchen-garden-v1` through the existing TypeScript Structure
Lab DSL. Extend semantic material roles only with the narrowly reusable garden
roles actually consumed here: `fence`, `gate`, `soil`, `cropPrimary`, and
`cropSecondary`. The recipe should contain:

- a connected oak-fence perimeter with one correctly oriented gate;
- a readable path and gate approach;
- a central irrigation run with hydrated beds in four-block reach;
- mixed mature and growing wheat/carrot rows; and
- restrained flowers or work accents that do not obscure interaction.

Its checked generated JSON and preview are source artifacts, not gameplay
authority. Promote the record into the runtime catalogue and make the accepted
intro-homestead `garden-v1` landscape role consume its footprint/palette while
projecting the bounded bed and boundary pattern onto the surveyed terrain.
The homestead remains an explicit starter-content overlay; pure Java Overworld
output is unchanged. Existing internal homestead worlds may be discarded or
explicitly rebuilt under the compatibility ledger.

## Showcase Contract

Add one focused `kitchen-garden` data recipe only after the ordinary producers
and tests exist. It may compose a fresh tiny save with a closed gate, connected
fence, hydrated wheat/carrot beds, one mature carrot target, and the same
bounded player items. Every non-base fact needs typed evidence naming the live
placement, crop, or homestead producer.

The recipe contains no gate script, crop acceleration, rabbit, refill, loot
override, navigation command, or showcase-only prompt. Desktop and phone gates
must use real controls to:

1. open the closed gate and walk through its former collision line;
2. close it and prove the barrier is restored;
3. harvest a mature carrot into visible ordinary world drops;
4. move to collect the drops, then plant one carrot on farmland;
5. observe at least one automatic crop or soil transition; and
6. retain zero records in every browser world store.

The public URL is a resettable review save. The screenshot, seed, entry pose,
recipe revision, gameplay receipt, and deployed Git revision must match.

## Implementation Slices

1. **Record the contract.** Add this tactical, the living garden topic, and
   series log entry.
2. **Remove the byte ceiling.** Widen raw storage and raw worker codecs, add
   above-255 round trips, and run generator/worker non-regression suites.
3. **Land boundary mechanics.** Add exact fence/gate states, assets, multi-box
   shapes, placement/update/use, sounds, persistence, and navigation tests;
   inspect the first connected/open/closed pixels before continuing.
4. **Generalize crops.** Extract crop specs without changing wheat, add carrot
   states/items/Texture Lab stages, planting/growth/loot/item-prop/persistence,
   and inspect planted/mature/drop pixels.
5. **Author the garden.** Extend the narrow semantic-role vocabulary, add the
   TypeScript kitchen-garden source, regenerate JSON/preview, and promote it.
6. **Place it live.** Replace the accepted homestead's placeholder flower beds
   with the working projected recipe, update plan/content identity as needed,
   and prove deterministic clipped placement, reopen, and edit precedence.
7. **Build the review save.** Add typed evidence and a bounded data recipe,
   then run real desktop/phone gate, harvest, pickup, planting, automatic-tick,
   and zero-storage acceptance.
8. **Close and deploy.** Update the garden, farming, farmstead, Structure Lab,
   showcase, worldgen-ledger, and platform records; run proportional host
   gates; push; deploy the exact revision; inspect public desktop/phone pixels;
   and share the screenshot plus transient URL.

## Human Review Points

- **Review A — boundary readability:** connected fence arms and open/closed
  gate poses must read as one coherent enclosure in actual game pixels.
- **Review B — physical truth:** walking and a ground-creature navigation
  probe must agree that open passes and closed blocks; no invisible full cube
  may remain in the doorway.
- **Review C — crop language:** newly planted, growing, and mature carrots plus
  their world drops must be understandable without crosshair text.
- **Review D — place quality:** the Structure Lab preview and ordinary
  homestead garden must feel like the same intentional kitchen-garden design,
  not a debug grid pasted onto terrain.
- **Review E — phone loop:** the deployed 390x844 view must expose named items,
  gate use, traversal, harvest, pickup, and planting through rendered controls.

The authorized autonomous run may pass these points through inspected evidence.
It must stop rather than compensate for a substantive shared-mechanic failure
inside the showcase.

## Validation

At minimum:

- raw-state above-255 storage plus native/Web job-codec round trips;
- exact asset registry, blockstate bake, first-party inventory, and pack locks;
- fence/gate state helpers, transforms, neighbor refresh, shapes, raycast,
  collision, pathfinding, placement, use, persistence, and sound tests;
- wheat non-regression plus generic crop, carrot support/growth/loot/drop/full-
  inventory/replant/restart tests;
- Structure Lab typecheck, source/generated drift, Rust load/place, preview,
  and runtime-promotion tests;
- homestead deterministic plan, clipped placement, order, restart, player-edit,
  pure-base worldgen, and compatibility-ledger evidence;
- `cargo fmt --all --check`, affected shared crate suites, and Web typecheck;
- inspected native pixels at the fence/gate and carrot milestones;
- local headed-WebGPU desktop and phone kitchen-garden gates;
- affected Android/XR compile boundaries because the shared item/state/UI path
  changes, with a physical device run only if a platform-specific defect is
  exposed; and
- exact pushed/deployed public desktop and phone gates with zero persistence.

## Execution Record

Implementation and evidence will be appended as slices land.

