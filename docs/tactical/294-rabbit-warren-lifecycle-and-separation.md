# Tactical 294: Rabbit Warren Lifecycle and Separation

Status: active 2026-08-13

Topic: `rabbit-burrow-ecology`

## Instruction Synthesis

Correct the rabbit chapter after interactive review. A rabbit whose chosen
carrot is harvested must stop eating empty space immediately. Deep sheltering
must be a normal, observable part of the same persistent rabbit's day, not a
technically correct state that the frozen review scene never reaches. Rabbits
must gently separate instead of remaining interpenetrated. Burrow mouths must
be disturbable and destructible through ordinary cross-platform interaction;
residents must survive collapse, emerge, flee, lose the invalid home, and seek
real replacement habitat. Mature offspring should disperse when a warren is
full. Prove these facts in shared server ownership, persistence, the bounded
data-only showcase, and exact deployed desktop/phone review.

## Diagnosis

- `on_block_changed` invalidates a nearby rabbit intent, but the `Raid` state
  still waits out its 26-tick chew epoch. Harvesting the selected carrot can
  therefore leave a rabbit foraging at air for about 1.3 seconds.
- Deep `Underground` state already removes the same persistent rabbit from
  client tracking, but `rabbit-burrow` freezes time at active noon. Housed
  rabbits have no ordinary reason to enter during the review window.
- Rabbit movement resolves against blocks but has no living-entity contact
  pass. Multiple rabbits can retain the same horizontal center indefinitely,
  especially around one home mouth.
- `RabbitBurrowRuntimeState::disturbance_ticks` persists but has no live
  producer or consequence. Burrow props cannot be selected by the ordinary
  attack ray, so an invalid home cannot be removed by a player.
- Full warrens reject birth but never release mature offspring. Only rabbits
  that already have no home can found another burrow.

## Reference and Deliberate Extension

Minecraft Java 1.17.1 `LivingEntity::pushEntities`, `doPush`, and
`Entity::push(Entity)` are the contact reference: overlapping pushable living
entities apply small mutual horizontal impulses rather than becoming rigid
obstacles. Mclone will translate that shape into its direct authoritative
movement model with deterministic pair ordering and normal block collision.
Hidden underground rabbits do not participate. Entity cramming damage is not
part of this slice.

Persistent excavated warrens, disturbance, collapse, resettlement, and
offspring dispersal remain intentional Mclone ecology extensions. They must be
ordinary world mechanics and may not branch on a showcase ID.

## Binding Behavior

### Interrupted feeding and observable shelter

- A block update that invalidates the active raid clears its route and exits
  `Raid` on that server tick. The next ordinary choice may target another
  mature carrot or forage; no completion event may mutate the removed crop.
- Housed rabbits take deterministic, identity-staggered short rests even
  during an active dawn/day window. They route to their real home, play the
  visible `enter_burrow` threshold transition, become untracked for a bounded
  underground interval, then republish the same entity and persistent IDs on
  `emerge`.
- Threat, temptation, inactive-time shelter, digging, and valid in-progress
  raids retain priority. Active rests do not synchronize a whole family at
  one instant.

### Soft rabbit separation

- After ordinary rabbit movement, each visible alive overlapping pair
  receives equal and opposite bounded horizontal separation derived in stable
  entity order.
- The displacement is resolved through the shared block-collision owner, so
  pushes cannot leak through a closed fence, gate, or bank.
- Exact coincident centers use a stable identity-derived direction. Kits use
  their actual smaller width. Contact and brief mouth congestion remain
  possible; sustained same-space occupation does not.

### Disturbance, collapse, and resettlement

- The normal attack ray may select a rabbit burrow with any held item while
  deer remain spear-only. Server reach, target identity, line of sight, world
  permissions, and authoritative kind are revalidated.
- A hit adds bounded, decaying disturbance to the persisted habitat prop.
  Disturbed mouths are temporarily unsafe: resident rabbits visibly flush
  from deep shelter and flee from a nearby player while retaining the home if
  the pressure subsides.
- Three prompt hits collapse the mouth. The semantic prop is removed through
  normal entity tracking/persistence. Every loaded resident keeps its own
  identity and family data, becomes visible, loses the invalid home, flees,
  and then uses the existing habitat-qualified path/dig producer to found a
  replacement. Collapse never deletes rabbits or invents a showcase spawn.
- Failed competing digs clear their stale target and search again rather than
  repeatedly chewing an already excavated cell.

### Full-warren dispersal

- Resident slots are reconciled against live rabbits whose saved home still
  names the mouth.
- When occupancy reaches capacity, one stable mature offspring with recorded
  parents disperses. Founding adults and immature kits remain. The released
  rabbit keeps lineage and identity, emerges if sheltered, and enters the
  same normal founder/dig loop.
- Capacity still bounds birth; dispersal creates future space through an
  ecological consequence rather than silently growing the record.

## Shared Ownership and Persistence

- `mclone-server::entity::mob` owns raid cancellation, short-rest state
  transitions, and rabbit-local release helpers.
- `mclone-server::entity::store` owns pair separation, habitat-prop damage,
  resident reconciliation, collapse outcomes, and dispersal.
- `mclone-client` owns kind/tool-aware attack targeting; `mclone-scene`
  continues to submit the same neutral `AttackEntity` command on flat and XR
  paths.
- Existing rabbit/burrow save payloads remain the durable record. Decaying
  disturbance and changed resident/home links must dirty the owning entity
  chunks, survive a save boundary, and not require protocol or platform-local
  behavior.

## Showcase and Acceptance

Advance `rabbit-burrow` as data only after the ordinary mechanics pass focused
tests. Its frozen active time remains useful only if the live short-rest loop
makes shelter observable there. The browser gate must:

1. observe an initial rabbit ID leave visible tracking and later return with
   the same ID during an autonomous window;
2. reject sustained near-zero pair separation while retaining the existing
   off-axis gate, travel, and real carrot-raid evidence;
3. attack the real initial mouth through keyboard and rendered phone `ATK`,
   observe disturbance and collapse, and verify residents are not lost;
4. observe ordinary replacement digging after collapse; and
5. retain exact recipe/seed/camera receipts and zero records in all browser
   persistent-world stores.

Focused tests must additionally prove interrupted raids, collision-respecting
mutual separation, disturbance decay/persistence, hidden-resident release,
full-warren mature-offspring dispersal, and reload identity/home truth. Inspect
native flat/stereo pixels, local Web desktop/phone pixels, then push, deploy the
exact revision, and inspect deployed desktop/phone pixels before handing off
the fresh URL.

## Non-Goals

- player-walkable tunnel networks, voxel cave-ins, burrow repair, trapping,
  relocation tools, artificial rabbit boxes, predators, or unloaded
  population-summary simulation;
- general rigid-body crowd avoidance, cramming damage, or a rabbit-only
  navigation system; and
- showcase scripts, timers, commands, damage exceptions, or alternate
  behavior state.

## Execution Record

Pending implementation and validation.
