# Tactical 289: Wheat Farming Feedback and Harvest

Status: **complete 2026-08-13; shared correction, exact pushed deployment,
public desktop/phone interaction gates, and inspected pixels accepted.**

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
  every 68 seconds on average at 20 gameplay ticks per second;
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
- Texture Lab proves the proprietary-free age-zero asset is a bright rosette.
  Rendered desktop and phone captures prove planting has immediate explicit
  `Wheat sprout` feedback at the targeted cell without relying on its outline.
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

Commits `0d1b23b1` and `8e6cab39` implement the shared correction and recipe
revision 2. Nearby-water tilling now creates moisture-7 farmland immediately;
dry tilling stays moisture 0. The replicated target state drives ordinary flat
HUD labels for sprouts, growing crops, and mature crops, and the mature label
names the rendered `ATK` action. Hoe and seed mutations use the existing
authoritative-confirmation sound queue. Texture Lab authors a brighter age-zero
rosette while leaving collision and selection geometry unchanged.

The revision-2 entry eye is `8.5,65.62,14.5` and its target is
`9.5,64.5,10.5`. Both local browser gates began on exact mature state 236 at
`9,64,10`, harvested it before any smoke framing call, then used actual
keyboard or rendered touch controls to till `8,63,14` directly to moisture 7
and plant above it. Both saw one unrelated automatic farming transition and
kept every browser world-record store empty. Inventory receipts were wheat
`0 -> 1` and seeds `10 -> 9` after the harvest seed roll and planting.

Inspected local first-frame desktop and phone digests are respectively
`fd8510a51ef69a8b522e31f1a1d5caa5c562c1b0088fbca53000a51f46a2eb07`
and
`c09833fac7bb6ca09496383cd1ac1cc58dcbc08158d3e7aa21033011f357ef74`.
The corresponding just-planted captures are
`50a1b89e780251b95de52d701a7d16e3178e14dae2a13a37386a3bd3f9b0a017`
and
`be303f33a85d30dde3a4c5ae8a770132100461491abb27b85b6b0b76705688eb`.
The inspected native capture digest is
`0eaef98116ddce000e7838e228b290b5477067ab4f244cb4e6d1f2043b7db735`;
the age-zero Texture Lab contact sheet is
`8eaa8f7df815403bbd719dbe26b07b990a450f76fe4bf57b9aacaf6c6aac6338`.

Validation completed locally:

- full `mclone-server`: 670 passed;
- full `mclone-ui`: 111 passed;
- full `mclone-scene`: 215 tests passed across unit/integration/doc lanes,
  with one existing GPU characterization ignore;
- Texture Lab typecheck and age-zero export;
- deterministic revision-2 fixture compilation; and
- local desktop and phone WebGPU showcase gates with inspected initial and
  planted pixels.

Exact pushed revision
`924edf10a706b43a1965413e359965292eac91a6` deployed with asset version
`924edf10a706-20260813071945` as Cloudflare Worker version
`a917fa06-dbc2-4ddc-ac55-04963368e209`. Public desktop and phone gates
reproduced recipe revision 2, the untouched-camera state-236 harvest,
moisture-7 tilling, planting, inventory changes, an automatic farming
transition, and zero records in all eight browser world stores.

The public initial desktop and phone frames are byte-identical to their local
counterparts at the digests above. The inspected public desktop planted frame
is also byte-identical at
`50a1b89e780251b95de52d701a7d16e3178e14dae2a13a37386a3bd3f9b0a017`.
The inspected public phone planted frame is
`175d4502904c2be544afb2800790bf56d240236b287b3c892872994382f9b47c`;
its live capture frame is visually equivalent but not asserted byte-stable
across runs. The accepted resettable field is:

```text
https://mclone.kzahel.com/app.html?showcase=wheat-farming
```
