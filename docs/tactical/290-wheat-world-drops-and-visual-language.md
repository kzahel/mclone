# Tactical 290: Wheat World Drops and Visual Language

Status: **in progress 2026-08-13 after human rejection of deployed revision
2.**

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

