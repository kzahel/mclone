# Tactical 290: Wheat World Drops and Visual Language

Status: **complete 2026-08-13, including exact-revision public desktop and
phone acceptance.**

Topics:

- `wheat-farming`
- `playable-showcases`

## Instruction Synthesis

Human review found that harvested wheat appeared not to leave the field and
produced no ordinary pickup like a hunted deer's loot. The crosshair label was
useful as debug or accessibility information but had become necessary to read
planting, while the planted crop itself still lacked visible green in the
default presentation. Replace the inventory shortcut with the shared item
entity lifecycle, make removal and pickup visually unambiguous, hide the
diagnostic label by default, and give age-zero wheat a pack-independent green
silhouette.

## Rejected Baseline

- Wheat harvest removes the authoritative block but atomically writes loot
  straight into the player's inventory. It bypasses the generic replicated,
  persistent, moving, merging, delayed-pickup item entities already used by
  deer, mallards, chickens, and bees.
- The revision-2 gate checks the air state and immediate inventory increase,
  then turns away. It never captures the post-harvest field or proves a world
  item appears and is collected through movement.
- The entry target sits in front of dense wheat. Once it becomes air, another
  crop along nearly the same sight line can make removal visually ambiguous.
- The ordinary HUD always renders crop-state text. The default reference
  age-zero texture contains only seven opaque green pixels on vertical cards,
  so a steep planting view can depend on text or the selection outline.

## Contract

- Block harvest first removes wheat, then spawns its loot as ordinary item
  entities through the shared server entity store. Default pickup delay,
  gravity, movement, merging, tracking, persistence, capacity handling, and
  inventory collection remain generic item-system responsibilities.
- Full inventory never prevents crop removal. Unaccepted loot stays in the
  world as an item entity, matching the normal block-drop semantic shape.
- Wheat and seed item entities use dedicated Asset Lab `item_prop` figures,
  not the egg-shaped compatibility fallback.
- The default HUD does not name the targeted crop. Keep the existing labels
  behind the existing debug-diagnostics switch; a future focused accessibility
  preference may reuse the neutral UI model.
- Add a small top-readable green leaf surface to age-zero wheat in the shared
  textured mesh catalog for both reference and first-party packs. It may not
  change collision, selection height, growth state, or interaction reach.
- Showcase revision 3 places the entry harvest crop on an isolated sight line.
  Composition may expose the mechanic but must not change any behavior.

## Acceptance

- Authoritative tests prove mature and immature harvests create the exact
  item stacks, publish their entities, leave air immediately, and enter the
  inventory only after ordinary pickup.
- A full-inventory test proves the crop still breaks and its drops remain.
- Asset Lab validates and renders dedicated wheat and seed pickup props.
- Mesh tests prove only age-zero wheat receives the top-readable leaf surface
  and its ordinary selection/collision contract is unchanged.
- Desktop and phone browser gates capture the isolated empty harvest cell with
  visible dropped loot, use real movement controls to collect it, then plant
  and capture green age-zero geometry without the crop label or outline as the
  primary evidence.
- Native and local Web pixels are inspected before the exact pushed revision
  is deployed. Public desktop/phone gates and zero IndexedDB world records pass
  before this tactical closes.

## Guardrails

- Do not add showcase-only loot, pickup, crop removal, prompts, or rendering.
- Do not command dropped items from the harness or award inventory directly.
- Do not enlarge crop collision or selection geometry for visibility.
- Do not make a permanent tutorial banner substitute for object-level visual
  language.

## Execution Record

Commits `040b2ffa` and `a6729567` implement the shared correction. Harvest now
sets the crop to air and emits its reference-shaped wheat/seed stacks through
the existing persistent item-entity store. Ordinary delay, gravity, movement,
tracking, merging, capacity-aware pickup, and saving own the rest of the
lifecycle. A full-inventory test confirms that breaking succeeds while all
unaccepted loot remains in the world.

Wheat and seeds now map to dedicated checked Asset Lab semantic props. The
first-party asset inventory requires both files, so native and Web staging
cannot silently omit them. Age-zero wheat gains a small top-readable surface
using its active pack sprite; the normal collision and selection facts remain
unchanged. Crop target text is no longer populated in the ordinary HUD and is
available only when debug diagnostics are visible.

Pixel inspection uncovered and corrected a general Web rendering fault rather
than hiding it in the recipe: `SectionBlockUpdates` patched the client snapshot
without advancing its content revision, so the browser render-worker mirror
did not receive the changed column. Effective section deltas now advance that
revision, while idempotent deltas do not. This makes section-boundary additions
and removals remesh from current state for all live blocks.

Revision 3 keeps seed `17506` and entry eye `8.5,65.62,14.5`, moves the
untouched-camera target to isolated crop `11,64,10`, and changes no behavior.
Both local desktop and 390x844 phone gates:

- remove state 236 to visible air;
- capture one wheat sheaf and the reference seed roll as normal item entities
  before the inventory changes;
- use real keyboard or rendered touch movement to collect those entities;
- till and plant through ordinary controls;
- capture bright green age-zero geometry with debug labels disabled; and
- retain zero records in all eight browser world stores.

Inspected desktop initial, harvested, and planted SHA-256 digests are
`1a624c6dd7785a948137f2a69190fc623ee1e3d00b96a46360aa3aec9952097c`,
`b43edcee22b7aa9e7083955f748e0d60ff9d960fdeab012fac48b89365b76984`,
and
`01c66226e3b29d96985b2b6b378f22ad09781c71cd43393093d2c912c1c78a66`.
The corresponding phone digests are
`238cbbcf901e065decc4c57f0bd011d10188c736ffc56065cc9a138f0302e69c`,
`d7e62749744f345cc1935918d77d611155cac45be1a5901e9bd73df916e65ae3`,
and
`c17abedc91702af4fcea639bf7dbf5c200fa48b7ac5db657c2aee533c87e488f`.
The inspected native initial frame is
`affb5450ceff18f072d9e236bd4529db2eb745b7df198f0921d2ce783aec9562`.

Asset Lab checks and the full affected Rust suites pass: 77 asset, 143 client,
103 mesh, 132 render-session, 175 scene plus contract, and 670 server tests.
The one broad server-suite resident-chicken failure passed alone and the full
suite passed on rerun, so it remains an unrelated order-sensitive flake rather
than accepted farming evidence.

Exact pushed revision `0f746896545ee3bc9ce446257d656b5ab28fbffe`
deployed as asset version `0f746896545e-20260813081257` and Cloudflare Worker
version `a2306ede-be7a-4056-9b7c-73a2a1b9a89c`. The first public request reached
revision 2 during edge propagation; direct app, JS, and Wasm hashes then
matched the deployed bundle, and both gates passed on retry against the clean
public route.

Public desktop and phone receipts reproduce revision 3, seed `17506`, crop
state 236 at `11,64,10`, one wheat entity, one seed entity for this reference
roll, unchanged inventory before pickup, `4.14` blocks of real movement,
collection, tilling, planting, debug labels off, and zero records in every
browser world store. The public initial frames are byte-identical to local.
Inspected public desktop harvested/planted digests are
`a93ea616dee3086edffd2c6cdf20e3cef860068f989021f903e447017f609934`
and
`9637b7ec8be7f7daee10b67d0644a2cad3bef3efd626460d8ebc695582b2aacf`;
phone harvested/planted digests are
`1899ec9b17b88877354311bcb4875d6117263d9c71f81e754633caf955274518`
and
`af7a0cd01f76ee8eb0a05abff7f7f4b40dae0ae30c863e1a63a7602f58a63172`.
All four were inspected and show an empty harvested cell with ordinary loot or
clearly green planted growth. This closes the tactical.
