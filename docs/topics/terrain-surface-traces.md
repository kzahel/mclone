# Terrain Surface Traces

Topic: `terrain-surface-traces`

Status: planned shared rendering prerequisite, selected by Human Review 1 of
Tactical
[`284`](../tactical/284-deer-forest-edge-ecology-and-semantic-props.md) on
2026-08-12. The existing mallard track mechanic remains authoritative and
bounded, but its raised procedural figure is a temporary presentation. The
reviewed rigid `mallard_tracks` semantic asset was rejected and removed.

## Scope

This topic owns lightweight marks that visually conform to an existing world
surface: footprints, hoofprints, scuffs, damp marks, and similar ecological or
gameplay evidence. It does not own the authoritative event that creates a
trace, its discovery/observation meaning, or persistent terrain mutation.

A surface trace is not a rigid `FigureAsset`. It needs a terrain-aware render
contract that can project or fit a small image/shape to the sampled ground,
follow slope, avoid z-fighting, respect occlusion, fade or expire cheaply, and
draw correctly in mono, stereo, per-eye XR, and full-frame multiview.

## Selected Boundary

- Authoritative simulation decides when and where a track exists, its
  orientation, species/trace kind, age, expiry, and observation meaning.
- `mclone-render-session` maps replicated trace state into a host-neutral
  surface-trace presentation record.
- `mclone-render` owns terrain-conforming geometry/projection, depth behavior,
  batching, material/atlas selection, and every view path.
- `mclone-scene` owns admission and bounded per-world presentation lifetime.
- Platform apps only supply their existing render targets and cadence.

The first implementation should be a small shared decal or surface-patch path,
not a general-purpose deferred decal engine. It may use a terrain-fitted quad,
small projected mesh, or another measured representation, provided the public
record stays neutral and does not bake one renderer technique into gameplay.

## First Acceptance Contract

- Mallard footprints sit on grass, mud, sand, and bank slopes without floating,
  visible thickness, severe stretching, or persistent z-fighting.
- Orientation still follows authoritative realized travel, and existing local
  caps, expiry, and field-note unlocks remain unchanged.
- Traces do not mutate chunks, enter durable entity storage, or become semantic
  figure assets merely to obtain geometry.
- Batching and atlas use remain bounded as traces appear and expire; an animal
  cannot cause unbounded resource or draw growth.
- Native flat, synthetic stereo, browser WebGPU, and XR multiview contracts are
  tested. Each eye/layer receives its own view data.
- The temporary raised `MallardTrack` procedural mesh is deleted after accepted
  in-world pixels and behavioral equivalence, with no hidden fallback retaining
  the old visual.

## Deer Dependency

Deer tracks in Tactical 284 should consume this path rather than add a deer
mesh function or revive `trace_prop` in Asset Lab. Deer bed/sign and shed
antlers remain ordinary durable world/item semantic props because they have
meaningful 3D form and interaction identity. This distinction keeps one rigid
asset pipeline without forcing every mark on terrain into it.

Persistent trail wear, snow or mud deformation, accumulated grazing history,
and terrain material mutation are later world-history concerns. They should
consume trace evidence only after the ephemeral presentation path is proven;
they are not requirements for the first decal slice.
