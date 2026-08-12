# Figure Animation Actions

Topic: `figure-animation-actions`

Status: shared runtime playback landed 2026-08-12. Canonical figures
carry authored defaults and clip presentation metadata, Asset Lab plays and
completes one-shot actions explicitly, and authoritative entities now replicate
named distance- or elapsed-phase requests through persistence and rendering.
Deer is now the first accepted multi-action runtime asset; authoritative
gameplay selection remains the next slice of Tactical
[`284`](../tactical/284-deer-forest-edge-ecology-and-semantic-props.md).

## Scope

This topic owns extensible figure animation selection across canonical Asset
Lab sources, semantic JSON, shared Rust preparation, the Three.js viewport,
and the deployed animal catalogue. It covers continuous locomotion or idle
clips, optional one-shot actions such as jump and roll-up, action completion,
and catalogue discovery and playback.

It does not define gameplay AI or decide when a game entity jumps, attacks, or
curls. Gameplay remains responsible for requesting an authored clip. It also
does not add layered blending or extract a shared anatomical curl helper before
multiple corrected figures prove that helper's useful shape.

## Motivation And Observed Gap

Batch 26 exposed the gap through Roly-poly. Its first candidate had only one
looping `roll_up` clip, so roll-up was also the catalogue default and no
ordinary crawl existed. The motion curled one front-rooted segment chain by
carrying the tail over the head. A real pill bug closes inward from both ends
with its armored dorsal plates outside.

The lower-level system is already mostly general:

- `FigureAsset.clips` is an arbitrary named map;
- the semantic Three.js scene can switch clips without rebuilding geometry;
- the catalogue manifest and URL already retain clip names; and
- the catalogue already provides a raw clip selector.

The missing contracts are clip roles, an authored default, action completion,
action-aware catalogue controls, and a runtime request shape beyond the
renderer-local `Walk` enum.

## Accepted Semantic Contract

`FigureAsset` gains an optional authored `defaultClip`. `ClipSpec` gains
optional presentation metadata:

- `label`: human-readable text; otherwise derive it from the clip name;
- `role`: `locomotion`, `idle`, or `action`;
- `nextClip`: the clip selected after a non-looping clip completes.

Compatibility remains additive. Existing sources without metadata retain the
current deterministic default selection. A clip with locomotion metadata is
inferred as `locomotion`; other unannotated clips remain `action`/custom for
catalogue presentation until migrated deliberately.

Validation requires referenced defaults and `nextClip` values to exist. A
looping clip cannot declare `nextClip`, because it never completes. A
non-looping action without `nextClip` stops and holds its final pose.

The builder exposes the default without changing every `figure()` call. New
multi-clip figures should author it explicitly.

## Playback And Catalogue Contract

The viewport remains the sole Three.js animation evaluator. It adds a
completion notification and reports the selected clip so the catalogue can
restart through the existing `setClip` operation. Pressing an action button
therefore starts that action at zero even when it is already selected. The
catalogue keeps React playback state synchronized when a non-looping clip
stops.

The catalogue:

- groups continuous locomotion/idle clips separately from actions;
- exposes actions as accessible buttons while retaining the complete clip
  selector and shareable `?clip=` URL;
- restarts an action when its button is pressed;
- holds the final pose when no `nextClip` is authored;
- switches to and plays `nextClip` on completion when present; and
- exposes role and completion behavior in the inspector and searchable static
  manifest.

Thumbnails use the authored default, not a special action.

## Roly-poly Correction

Roly-poly is re-rooted around its middle shell band. Separate front and rear
segment chains close inward together so the head and tail tuck underneath and
the dorsal plates remain outside. Its clip set is:

- `crawl`: default looping locomotion with a fourteen-leg alternating wave;
- `roll_up`: non-looping action ending in and holding a compact closed pose;
- `unroll`: the matching reverse action, followed by `crawl`.

Review evidence shows crawl independently from roll-up/unroll. The asset does
not add a catalogue-only looping demonstration clip.

## Shared Rust Boundary

`mclone-assets` deserializes and preserves default, role, label, and completion
metadata in prepared figures. `mclone-core` owns a compact validated clip ID
and replicated animation state. The authoritative state contains the clip,
phase source, epoch, and action start tick. Protocol version 36 carries that
state on snapshots and updates; entity-chunk record version 4 persists it.

Render-session maps the request to one of two explicit renderer phases:

- distance-driven phase for locomotion; or
- elapsed-time playback for actions and idle clips.

Distance phase continues from interpolated presentation displacement. Elapsed
phase derives from replicated world time minus the persisted start tick, so a
chunk hydration, reconnect, delayed update, or render pause does not locally
restart the action. Renderers resolve the requested authored name directly and
hold the rest pose if it is unavailable; first-party content validation remains
responsible for rejecting missing required clips.

Mallards now request their authored `waddle` clip instead of the old literal
`walk` mapping. Existing cows, chickens, mannequins, and remote players retain
distance-driven `walk`. A gameplay animation state machine, crossfade/layering,
and deer behavior selection remain later slices.

The Tactical 284 deer review candidate is the first asset authored directly
for this runtime contract. It has explicit `idle`, `walk`, `flee`, `alert`,
`graze`, `lie_down`, `bedded_idle`, `stand_up`, `hit`, and terminal `fall`
clips. Split upper/lower legs make feeding and bedding articulated rather than
whole-model offsets. Tests compare evaluated part matrices at both transition
boundaries: `lie_down` ends exactly at `bedded_idle`, and `stand_up` ends
exactly at `idle`.

## Local Acceptance Evidence

The completed slice has the following evidence:

- `pnpm asset-lab:typecheck` passes the strict TypeScript contract;
- `pnpm asset-lab:test` passes 19 semantic, JSON, scene, discovery, catalogue,
  and first-party drift tests, including action metadata round trips;
- `cargo test --manifest-path native/Cargo.toml -p mclone-assets` passes 63
  unit tests, its runtime/tooling boundary test, and the prepared action
  metadata case;
- `pnpm asset-lab:web:build` produces 104 figures, 109 clips, and 2,141 parts;
- production-subpath Playwright passes three desktop/mobile/action tests,
  including grouped roles, the action filter, restart, final-pose hold,
  automatic `nextClip`, URL state, and clean browser/page errors; and
- the held catalogue pose at
  `/tmp/mclone-roly-poly-action-catalogue.png`, three independent sheets, and
  crawl/roll-up/unroll videos under `/tmp/mclone-asset-lab/roly-actions` were
  inspected.

The corrected sheets prove that the two chains curl down from the middle band,
leaving the armored plates outside. `roll_up` closes and holds, `unroll` begins
from that exact pose, and `crawl` remains a separate continuous clip.

## Follow-up Content Evidence

Snail is the second accepted canonical figure to use the action contract. Its
looping `glide` default remains separate from non-looping `retract` and
`emerge`; `retract` holds its final pose, while `emerge` returns automatically
to `glide`. The soft-foot bottom stays on one ground plane throughout both
interpolated actions, and the independently parented shell settles onto that
same plane at full withdrawal. This proves the metadata on an anatomy and
action shape unrelated to Roly-poly without introducing a shared pose helper.

Batch 28 adds an accepted Frog with `hop` as its explicit
looping locomotion default and `jump` as a separate non-looping action. The
action authors a larger airborne arc and landing sequence, then uses
`nextClip: "hop"` to return to ordinary movement. The user approved it with
Gecko and Mouse after the contact-projection correction below.

The first sinusoidal version read as floaty, while a manual piecewise correction
read as jumpy and inaccurate. Procedural cycle tracks therefore now accept
optional scalar `min` and `max` constraints. The builder evaluates the ordinary
waveform, applies the limits, and bakes the result into normal clip keys. Limits
must be finite and `min` cannot exceed `max`; they are available to `bob`,
`swing`, `contactSwing`, and `followThrough`.

Scalar limits alone constrained the authored body or joint channel rather than
the transformed foot pad, so they did not produce the requested abrupt physical
contact. `walkCycle` now also accepts authoring-time `groundContacts`. After all
tracks and follow-through are sampled, the baker evaluates the actual contact
box through its complete hierarchy. If its lower surface penetrates `groundY`,
a bounded numerical root solve rotates one declared ancestor hinge just enough
to meet the plane. The contact box is counter-rotated by default so its sampled
pad orientation remains flat. Every correction is baked into ordinary keys;
unreachable contacts reject the source.

Frog's two clips remain fully procedural. Their body bobs may dip `0.04` figure
units below nominal rest, while four declared pad contacts project the front-arm
and hind-shin hinges against `y = 0`. The correction becomes zero as soon as a
pad clears the plane, producing the velocity discontinuity at landing that a
scalar clamp could only approximate. This remains a deterministic one-plane,
one-hinge authoring constraint, not general multi-joint IK, runtime collision,
or a semantic figure-format change.

## Recommended Next Direction

Use Tactical
[`284`](../tactical/284-deer-forest-edge-ecology-and-semantic-props.md)'s
accepted `mclone:deer` runtime asset to prove ordinary authoritative gameplay
selection across idle, locomotion, and one-shot actions. Keep the existing
Roly-poly lesson: do not extract anatomy-specific helpers until multiple
corrected figures prove a stable common pattern.
