# Tactical 231: Bushy Leaf Rendering

Status: active implementation, opened 2026-07-24

Topic: [`../topics/bushy-leaf-rendering.md`](../topics/bushy-leaf-rendering.md)

## Goal

Ship an original, pack-derived bushy-leaf option through the shared terrain
asset, mesh, scene, settings, preference, and renderer contracts. `Blocky`
must retain the existing leaf mesh with no decorative-card vertices. `Bushy`
must add a stable irregular canopy silhouette without requiring bespoke leaf
art and without creating platform-specific render paths.

This is inspired by the technique researched in the topic document. It is not
a copy of Better Leaves geometry, mask pixels, code, or assets.

## Locked product and architecture decisions

- Expose `Leaf Detail: Blocky / Bushy` as a Graphics row. It is independent
  from the future grass-density control.
- Default conservatively to `Blocky` on every platform until the measured
  closeout supports a different profile default.
- Persist the choice as a client-global, machine-local graphics preference.
  It is not world state, server state, or asset-pack selection.
- Generate one original bushy sprite from each active square leaf texture
  during asset preparation. The generator tiles the source into a doubled
  canvas and applies a deterministic analytic alpha silhouette.
- Keep the ordinary cube on the original source sprite. Decorative cards use
  the generated sprite, so third-party packs remain the art authority.
- Store generated sprite bytes only inside the preparation pipeline. They are
  rebuildable derivatives and do not become checked-in proprietary assets.
- Put the leaf-detail policy on `TexturedMeshCatalog`. The normal shared
  compiler remains the only geometry producer on native and browser paths.
- Change quality through the existing transactional asset-epoch replacement:
  clone the active prepared bundle, switch catalog policy, compile resident
  sections, upload, then commit at a frame boundary. The old epoch remains
  active after any prepare, compile, or upload failure.
- Emit two double-sided vertical cards per eligible leaf block (four quads).
  Pick a stable layout from world coordinates. Suppress cards for leaves with
  leaf neighbors on all six sides.
- Inflate renderer section culling bounds by the exact maximum card overhang.
  Apply the same bound in mono, stereo/per-eye, full-frame multiview, and
  placed-world culling.
- Far LOD stays unchanged.

## Slice 0: Baseline and executable contracts

- Add this tactical and link it from the tactical index.
- Pin generator behavior with RGBA tests:
  doubled dimensions, deterministic output, source-color preservation, and a
  transparent exterior.
- Pin mesh behavior with focused tests:
  `Blocky` emits no extra faces, `Bushy` emits four extra faces for a surface
  leaf, stable layout varies by world position, and a fully enclosed leaf emits
  no cards.
- Pin preference schema/default/invalid-document behavior.

Exit: the intended output and off-path are testable before live UI wiring.

## Slice 1: Derived active-pack leaf sprites

- Discover source leaf materials from the selected first-party or Minecraft
  model catalog.
- Decode square, single-frame sources and generate doubled RGBA sprites using
  an original analytic mask. If a source is missing, undecodable, non-square,
  or animated, retain `Blocky` for that material and report the unsupported
  derivative through deterministic preparation diagnostics.
- Overlay generated PNGs only for atlas planning/stitching.
- Attach the derived atlas sprite and ordinary foliage tint route to leaf block
  models.
- Ensure provenance still records the underlying source texture resolution;
  the derived logical path must not pretend to be an independently authored
  asset.

Exit: both authored/fallback and reference preparations expose leaf-card
materials without bespoke species textures.

## Slice 2: Shared geometry and culling

- Add the `LeafDetail` catalog policy and leaf-card model contract.
- Generate four cutout quads for each eligible leaf in the existing section
  builder. Use normal biome foliage tint and lightmap data.
- Select one of four deterministic angle/size/height layouts from a stable
  world-coordinate hash.
- Use the existing cutout pass and shader. Add no leaf-only render pipeline,
  draw store, per-eye uniform, or platform branch.
- Inflate ordinary and placed section-frustum AABBs by the maximum overhang.

Exit: `Blocky` has byte-for-byte existing section topology apart from atlas
layout, while `Bushy` produces visible, correctly culled cards in all view
topologies.

## Slice 3: Shared setting and transactional apply

- Add `GameLeafDetail`, the Graphics row, typed UI action, capability
  classification, reducer state, and setting effect.
- Add a shared scene setting host method that starts a catalog-only asset-epoch
  replacement. Native preparation remains background work; browser uses the
  existing portable replacement completion contract.
- Do not commit the UI's active value until the replacement succeeds. If the
  current reducer contract makes optimistic display unavoidable, project the
  committed scene value back after failure and show a reason-bearing status.
- Coalesce or reject a second switch while a replacement is active rather than
  mixing epochs.

Exit: one settings route changes real geometry and retains the drawable old
epoch on failure.

## Slice 4: Client-global graphics preference

- Add a versioned graphics-preference document initially owning leaf detail.
- Add the same native file and browser key/value adapters used by the input
  preference boundary; platform adapters know only storage mechanics.
- Load before initial scene/UI projection where each host has storage access.
- Store only after a successful epoch commit. Missing storage is an explicit
  non-persistent capability, not an engine fallback.
- Cover desktop flat, desktop XR, flat Android, Android XR, and browser
  startup/store wiring.

Exit: restarting a capable host restores `Blocky` or `Bushy` without affecting
world or pack identity.

## Slice 5: Pixel and performance closeout

At the first drawable milestone, capture and inspect:

- `/tmp/mclone-bushy-leaves-blocky.png`
- `/tmp/mclone-bushy-leaves-bushy.png`
- `/tmp/mclone-bushy-leaves-stereo.png`

Use a deterministic leaf-rich fixture or ordinary world position and record:

- ordinary versus generated atlas dimensions/bytes;
- blocky versus bushy section vertex/index/face counts;
- prepare, compile, upload, and total asset-epoch replacement time;
- steady drawn index/face delta in the same camera;
- at least one frozen and one moving frame-accounting comparison;
- no far-LOD change.

Validation follows [`../platforms.md`](../platforms.md#validation-policy), with
the smallest focused crate tests first, then workspace tests, native offscreen
pixels, headset-free stereo pixels, browser WebGPU pixels, and affected package
builds. Android and physical-XR validation may use their documented build/smoke
lanes; unavailable hardware evidence stays explicitly open rather than being
simulated.

## Completion conditions

- The generated silhouette is visibly original and not a traced source mask.
- Every current leaf family can derive from either selected first-party or
  reference-pack art without species-specific card textures.
- `Blocky` section meshes contain zero decorative-card vertices and indices.
- `Bushy` adds exactly four quads per eligible surface leaf and none for a
  fully enclosed leaf.
- Toggling is transactional and the last successful choice persists.
- Mono, per-eye stereo, full-frame multiview, browser WebGPU, and native
  renderer code all consume the same mesh contract.
- Captured pixels are inspected before closeout, and the topic document records
  measured costs plus the final default decision.

