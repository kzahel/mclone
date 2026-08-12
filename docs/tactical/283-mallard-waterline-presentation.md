# Tactical 283: Mallard Waterline Presentation

Status: **active 2026-08-12**

Topic:

- `habitat-driven-creature-ecology`
- `playable-showcases`

## Instruction Synthesis

Human review accepted the revised mallard showcase's habitat, motion, and
life-cycle behavior, but rejected the swimming pose: mallards visibly stand on
the water surface. Preserve the accepted simulation and scene while making
ordinary live mallards ride at a credible waterline, with submerged feet and
most of the body above water. Validate the correction in the same native and
deployed interactive showcase rather than introducing fixture-only placement.

## Diagnosis

The authoritative mallard simulation already moves entity feet to the water
surface and publishes an `in_water` state bit. The client snapshot initially
contains the persisted dry default, however, and the client replica currently
ignores mallard metadata carried by subsequent entity updates. Consequently
the browser continues to report zero swimming mallards and render-session
always composes the ordinary ground pose at the water surface.

## Decisions

- Apply authoritative mallard and nest metadata on ordinary client entity
  updates. Do not infer water from showcase coordinates or rendered blocks.
- Keep simulation feet at their existing authoritative water surface. Apply a
  small height-relative visual sink only while the replicated mallard state is
  `in_water`, so collision, persistence, navigation, tracking, and interaction
  positions remain unchanged.
- Scale the sink with the final presented model height so adult and duckling
  waterlines remain proportional.
- Preserve the same single-view, stereo, and XR-capable actor composition
  path; do not add a Web-only or showcase-only renderer branch.

## Acceptance

- A client replica regression proves mallard `in_water` and life-stage updates
  replace the initial snapshot metadata.
- A render-session regression proves a dry mallard remains at authoritative
  feet height while a swimming mallard is lowered by the bounded proportional
  visual offset without changing X/Z or actor identity.
- The behavioral browser probe observes at least one authoritative swimming
  mallard in its initial and final windows.
- Native flat and stereo captures and headed local Web pixels are inspected.
- Focused tests, workspace checks appropriate to the touched crates, Web build,
  and deployed browser smoke pass.
- The exact pushed revision is deployed, its public screenshot is inspected,
  and the temporary review URL remains resettable with zero persistent world
  records.

## Guardrails

- No changes to showcase entity Y coordinates, terrain, AI destinations, or
  water geometry merely to manufacture the screenshot.
- No mutation of authoritative feet positions for a presentation-only
  waterline correction.
- No generic sinking of other entities; species without a defined swim pose
  remain unchanged.
- Do not hide protocol/replica drift behind a local block query in the
  renderer. The existing authoritative state is the contract.

## Execution Record

Pending implementation and acceptance.
