# Tactical 268: Terrain Lab Landform-Plan Diagnostic

Status: complete; awaiting interactive human review

Topics: `mclone-macro-landscape-planning`, `gpu-procedural-terrain`

## Motivation

Tactical 267 produced a research-only hybrid macro-landform planner and a
static plan map whose basin ownership, divides, drainage hierarchy,
confluences, protected sinks, and quiet space made the candidate much easier
to reason about. Human Review B provisionally accepted the planning direction
and specifically requested an interactive, durable version of that diagnostic
in Terrain Lab.

The static artifact is not sufficient for continued generator work. It cannot
share coordinates with exact terrain, follow Terrain Lab navigation, isolate
individual plan facts, or grow into additional diagnostic layers without
another one-off renderer.

## Outcome

Add an optional **Landform plan** pane to the mclone Terrain Lab profile. The
pane is a two-dimensional diagnostic map at the same seed, center, and scale as
the other panes. It is not a terrain material view and remains two-dimensional
when the terrain panes use 3D presentation.

The first interactive view must expose independent controls for:

- basin-ownership fill;
- quiet-space modulation;
- drainage routes;
- drainage divides;
- confluences; and
- protected closed sinks.

The controls are URL-addressed so a useful review composition can be shared.
A point inspector must report the selected cell's basin, receiver, drainage
accumulation/order, envelopes, and structural flags.

## Ownership and Boundaries

- `mclone-worldgen` owns the research-only planner summary, its schema, and
  point facts. The production overworld evaluator does not consume the plan.
- `mclone-terrain-lab` owns the Wasm adapter that builds and serializes the
  summary.
- the Terrain Lab web app owns Worker transport, Canvas drawing, controls,
  legend, and URL/browser glue.
- the shared Terrain Lab navigation session continues to own pan, zoom, and
  keyboard semantics.

The browser must not reimplement basin selection, drainage classification,
divide extraction, or other planning semantics. Rendering colors and overlay
visibility are presentation policy.

## First-Pass Domain

Tactical 267 deliberately studied one bounded 6,144 by 6,144-block domain at
32-block cells. This slice promotes that same fixed plane domain centered at
the origin. Terrain Lab may navigate outside it, but the pane must clearly draw
and label the available study boundary rather than silently tiling or inventing
adjacent plans.

Periodic-X and toroidal interactive domains remain follow-up work. The shared
summary format must retain topology and domain metadata so they can be added
without changing browser ownership.

## Performance Contract

Planner construction and summary packing run in a dedicated Worker. Changing
only center, zoom, camera, or overlay visibility must not rebuild the plan.
The session caches the current seed result. The browser receives compact cell
and segment arrays, not the prototype's full reconstruction index.

The pane reports build time and transfer size. Its first-pass target is to keep
navigation and overlay changes main-thread-cheap even if a cold seed change
takes tens of milliseconds in Wasm.

## Validation

- deterministic Rust tests cover summary shape, classification invariants, and
  point inspection;
- Terrain Lab state tests cover pane/profile constraints and complete URL
  round trips for every diagnostic toggle;
- the Rust ownership lock permits browser code to perform transport and Canvas
  presentation but rejects semantic planner constants or algorithms;
- TypeScript typecheck, unit tests, Wasm build, and production web build pass;
- a headed browser capture shows the plan pane at the fixed review seed;
- the captured pixels are inspected at one broad view and one closer view;
- changing overlay controls updates presentation without rebuilding the plan;
- production overworld fingerprints remain unchanged.

## Human Review

Pause after the interactive pane, legend, and inspector are usable. Review
should answer:

1. Does the plan view make the global structure legible at useful scales?
2. Are the independent overlays sufficient to distinguish authored structure
   from incidental raster/noise texture?
3. Does inspecting a cell explain why terrain belongs to a basin, route,
   divide, sink, or quiet region?
4. Which next diagnostic deserves promotion: envelope fields, coast arrivals,
   journey transects, or reconstructed terrain influence?

## Implementation Record

The completed slice adds:

- `mclone-worldgen::landform_plan`, a deterministic research-only summary over
  the Tactical 267 plane domain;
- a Terrain Lab Wasm adapter and dedicated Worker that build once per seed and
  transfer full cell, skeleton, and sink arrays;
- an optional mclone-only **Landform plan** pane with shared pan/zoom,
  independent URL-addressed overlays, a fixed-domain boundary, legend, and
  Rust-owned point inspection;
- exact transferred-byte, cold-build-time, checksum, graph-count, and sink
  evidence in the UI;
- plan-only controls and explanatory copy that do not imply terrain material,
  projection, or benchmark settings affect the diagnostic; and
- desktop and phone responsive layouts.

The implementation is threaded through commits:

- `aaa75b95` — tactical and ownership contract;
- `86e12893` — shared research planner summary; and
- `d05da009` — Wasm/Worker adapter, pane, controls, inspector, and validation.

Validation completed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
  landform_plan --lib`;
- both pinned production terrain and surface-chunk fingerprint tests;
- `cargo check --manifest-path native/Cargo.toml -p mclone-terrain-lab
  --target wasm32-unknown-unknown`;
- the Terrain Lab browser ownership lock;
- Terrain Lab state tests, TypeScript typecheck, Wasm build, and production web
  build;
- focused headed Chrome tests for desktop and phone; and
- visual inspection of broad, closer, full-workspace, and responsive captures
  under `/tmp`.

The browser test proves that overlay changes and zoom preserve the same plan
checksum and cold-build receipt. Production overworld output remains pinned
and unchanged.
