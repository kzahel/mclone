# Tactical 289: Wheat Farming Feedback and Harvest

Status: **active 2026-08-13.**

Topics:

- `wheat-farming`
- `playable-showcases`

## Instruction Synthesis

Human review rejected the first deployed farming interaction. Hydration was
not consistently visible beside water, a newly planted seed had no immediate
readable feedback beyond its selection box, and mature wheat could not be
harvested reliably. Diagnose the ordinary path, correct shared gameplay and
presentation rather than teaching the showcase special behavior, strengthen
desktop and phone acceptance, then deploy the repaired transient field.

## Rejected Baseline

The implementation explains all three observations:

- hydration does not require a crop, but a single farmland cell is selected by
  the vanilla-shaped three-random-candidates-per-section system only about once
  every 85 seconds on average;
- wheat age 0 is only 2/16 blocks high and its authored texture contains a few
  dark pixels, so the authoritative crop update can be practically invisible
  at the review camera's phone distance;
- the scene confirms only debug block-item placement with sound, so ordinary
  hoe and seed actions receive no acknowledged audio cue; and
- the showcase camera looks along the water trench while the known mature
  target sits off-axis. The automated probe uses a smoke-only coordinate
  framing method, so its success did not prove human discoverability.

The prior persistence, crop simulation, and exact automated interaction
receipts remain valid. The claim that the public field was a usable manual
review of the loop is withdrawn.

## Corrected Contract

- Field creation remains independent of planting. When a successful till is
  already within the normal four-block water range, create moisture-7 farmland
  immediately. Dry tilling remains moisture 0 and later hydration/drying still
  use ordinary random ticks. This is a deliberate responsiveness divergence
  from 1.17.1's delayed hydration, restricted to the successful player action.
- Make age-0 wheat immediately visible as a compact but substantial bright
  green sprout, while retaining the reference 2/16 selection height, cutout
  rendering, no collision, and age progression.
- Confirm authoritative hoe and seed mutations through the existing shared
  interaction-sound path. Do not introduce a showcase-only prompt or effect.
- Expose a host-neutral target label in the ordinary HUD: `Wheat sprout`,
  `Growing wheat`, or `Mature wheat - harvest`. It derives from the targeted
  replicated block state and appears on desktop, phone, and other flat hosts.
  XR may consume the same neutral label when its world UI gains the matching
  presentation surface; this slice does not add a per-eye-only overlay.
- Reframe showcase revision 2 so the entry crosshair identifies a nearby
  mature crop and the tilled/plantable proof cell remains within ordinary
  movement and reach. Recipe changes may improve composition only; mechanics,
  loot, and feedback stay shared.

## Acceptance

- A server test proves water-adjacent tilling produces hydrated farmland
  immediately and dry tilling remains dry.
- Scene/UI tests prove ordinary hoe/seed actions enqueue confirmed feedback and
  crop states map to the three target labels.
- Texture Lab and rendered capture prove the new sprout is visible without its
  outline at desktop and phone scale.
- Desktop and phone browser gates begin by targeting a mature crop through the
  initial recipe camera without the smoke-only frame helper. They then harvest
  through the actual attack input, and continue to prove till, plant, inventory
  changes, automatic growth/soil ticking, and zero browser persistence.
- The exact pushed revision is deployed; both public gates and inspected pixels
  pass before the clean resettable URL is shared again.

## Guardrails

- Do not accelerate all random ticks or hydrate a showcase patch specially.
- Do not enlarge wheat collision or lie about its selection height to make it
  easier to hit.
- Do not add scripted arrows, text blocks, harvest coordinates, or interaction
  commands to the data recipe.
- Do not count an automated coordinate-framing call as evidence that a human
  can discover the mature target.

## Execution Record

Implementation pending.
