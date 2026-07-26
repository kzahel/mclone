# Tactical 254: UI-Less World Explorer Host

Status: proposed.

Topics:

- `platform-host-boundary`
- `world-view-navigation`

Parent:

- [`253`](253-world-explorer-cross-host-parity.md) sequences the complete
  cross-host parity campaign.

## Objective

Return the lightweight World Explorer to one terrain canvas with no
host-specific visible runtime UI.

The browser may retain a minimal pre-Wasm startup and fatal-error fallback.
After Rust is running, ordinary status, readiness, clipmap, memory, tree, and
camera diagnostics must not be formatted or rendered through HTML/CSS.

## Current Problem

The native Explorer has no in-frame overlay. It reports details through logs,
its Rust-authored window title, capture receipts, and smoke diagnostics.

The browser shell currently:

- parses the semantic frame report on every animation frame;
- mirrors readiness, residency, and focus into DOM data attributes;
- formats seed, view, center, slots, pending work, levels, draws, trees, and
  memory into a visible `<output>`; and
- styles that output as a translucent CSS panel over the terrain.

Although the underlying report originates in Rust, this is not an opaque
platform boundary. JavaScript understands Explorer vocabulary and owns a
visible product/debug surface that desktop does not share.

## Binding Boundary

Ordinary browser HTML/CSS may own only:

- document and canvas sizing;
- focus, overflow, and touch-action mechanics;
- module/Wasm loading;
- a generic pre-Wasm starting indication if useful; and
- a generic fatal fallback when no Rust-rendered frame can be produced.

It must not own:

- post-startup status or debug panels;
- Explorer labels or formatting;
- menus, buttons, map controls, legends, or settings;
- terrain or vegetation readiness interpretation; or
- UI visibility policy inferred from browser capabilities.

No replacement Rust-rendered overlay is required by this tactical. The
Explorer intentionally remains UI-less. If a later product needs visible
debugging or menus, it requires a separate shared-UI decision.

## Diagnostic Observer

Automated acceptance still needs semantic evidence. Preserve it through an
explicit read-only diagnostic observer:

- Rust authors the snapshot schema and values.
- The observer is opt-in through a clearly diagnostic construction/query
  surface rather than the ordinary production frame result.
- Browser code exposes or returns the opaque snapshot mechanically.
- Playwright may interpret the snapshot and perform semantic assertions.
- The ordinary deployed page does not install a rich mutable production
  mirror merely because tests need one.

The exact observer activation may be a diagnostic query, a smoke-only
constructor flag, or an explicitly named facade. It must remain distinct from
normal input and rendering.

## Implementation Order

1. Separate the current smoke observer needs from ordinary frame-loop needs.
2. Remove semantic status formatting, DOM data mirrors, and the successful
   runtime overlay.
3. Keep the startup/fatal surface generic and hide it after successful Rust
   initialization.
4. Update browser smokes to request the explicit observer.
5. Inspect equivalent native and headed-browser frames with terrain unobscured.
6. Lock the browser source against representative Explorer diagnostic and UI
   vocabulary in the ordinary shell.

## Acceptance

- After initialization, the browser content area contains only the rendered
  canvas.
- Desktop and browser terrain captures have no host-specific in-frame overlay.
- Browser JavaScript does not format seed, view, center, clipmap, draw, tree,
  vegetation, memory, or readiness labels.
- Real pointer, wheel, keyboard, resize, visibility, and rAF forwarding remain
  unchanged.
- Browser smoke assertions consume an explicit Rust-authored observer rather
  than DOM data attributes or visible status text.
- Startup and fatal errors remain understandable when Wasm or WebGPU cannot
  produce a frame.
- The Explorer does not add `mclone-ui`, `mclone-render`, or game-runtime
  dependencies in this UI-removal slice.

## Non-Goals

- Designing or rendering a replacement debug HUD.
- Adding player-facing menus or controls.
- Making browser and desktop OS chrome identical.
- Changing terrain, color, tree, input, or clipmap behavior.
