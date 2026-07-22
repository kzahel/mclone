# Tactical 219: Live Auxiliary Split Mode

Status: completed 2026-07-22.

Topic: [`local-couch-multiplayer`](../topics/local-couch-multiplayer.md).

## Instruction Synthesis

Promote the accepted offscreen split-presentation foundation into an opt-in
live single-player auxiliary view that can be selected from the shared menu.
Keep split semantics unified in Rust across desktop native, browser/WebGPU, and
flat Android: platform adapters supply a surface and dimensions, while shared
scene/render owners select the layout, cameras, HUD policy, preparation, and
GPU composition. Browser TypeScript must not learn whether a frame is split.

Commit incrementally and inspect native and browser pixels. Stop rather than
choose durable product defaults, second-player/profile behavior, global versus
participant pause policy, auxiliary observer authority, or a shipping helper
role.

## Locked Boundaries

1. The shared menu exposes a session-local `Off`, `Horizontal`, and `Vertical`
   auxiliary-view mode under Debug. `Off` is the default; this tactical does
   not persist a choice or select a product orientation.
2. Split presentation is active only during unobscured flat gameplay. Title,
   pause, options, death, and other full-screen menu surfaces remain one
   full-surface view, so this diagnostic does not settle future couch pause or
   menu ownership.
3. The primary pane retains the ordinary participant camera, HUD, input,
   interaction, and authoritative interest. The auxiliary pane is a
   deterministic elevated follow camera with no HUD, player identity, command
   stream, or automatic observer interest.
4. One shared scene frame prepares runtime/upload/actor inputs once and renders
   two independently culled, aspect-correct views into separate color/depth
   targets. A shared GPU compositor places them into the validated
   `FlatSurfaceLayout` rectangles.
5. Desktop winit, browser Rust, and flat Android reuse the same compositor and
   mode/layout contract. They may own surface acquisition, size, depth/target
   allocation lifetimes, cadence, and presentation only.
6. Browser TypeScript owns canvas lifecycle and calls the existing Rust frame
   boundary. It receives no split mode, pane, camera, HUD, culling, or
   participant semantics.
7. XR remains stereo/multiview for one tracked participant and must not route
   this flat auxiliary mode through its eye topology. The shared menu projects
   the setting as unavailable there.
8. The ordinary mono path remains direct when the mode is `Off` or a menu is
   active. It allocates no auxiliary targets and performs no second cull or
   render.
9. Desktop render scale keeps its existing behavior when split is off. The
   debug split renders each pane at its rectangle's native pixel extent; a
   future product-quality/per-pane scaling policy is outside this tactical.
10. Existing embedded-world preview exclusion for multi-flat frames remains
    explicit. Do not silently combine the two composition modes.

## Slice Plan

### Slice 0 — plan and contracts

- Add this tactical, update the couch topic, and pin source locks keeping
  split semantics out of browser TypeScript and platform event collectors.
- Record the live desktop, browser, and Android flat presentation seams.

### Slice 1 — shared mode, menu, and camera

- Add a neutral auxiliary split mode to shared UI render state, actions,
  client-experience settings effects, and scene ownership.
- Add one Debug menu cycle row with disabled projection outside flat clients.
- Derive an elevated follow-camera render view from the ordinary primary view
  without changing the scene camera or authoritative interest.
- Prove `Off -> Horizontal -> Vertical -> Off`, full-menu suppression, and
  camera/aspect behavior with shared tests.

### Slice 2 — shared GPU pane presentation

- Add a host-neutral wgpu presentation owner beside the existing flat scale
  presentation. It consumes validated `FlatSurfaceLayout`, owns per-pane
  color/depth targets, resizes deterministically, and composites with viewport
  plus scissor rectangles into one caller-owned output target.
- Prove 1-4 target allocation, layout rebuilds, and exact presenter viewport /
  scissor plans without acquiring a platform surface.

### Slice 3 — flat host adoption

- Route live desktop winit, browser Rust/WebGPU, and flat Android through the
  same two-view scene and GPU presentation path when shared state admits it.
- Preserve each host's existing mono fast path and frame summaries. The
  primary view remains the host's diagnostic/accounting source.
- Keep product TypeScript and platform collectors free of split semantics.

### Slice 4 — rendered and platform evidence

- Repair the HUD-enabled dual-view diagnostic and make its composites exercise
  the shared GPU presenter rather than a CPU-only placement path.
- Inspect horizontal and vertical native captures with primary HUD, auxiliary
  world-only presentation, correct aspect/depth, and no seam bleed.
- Run the headed Wayland browser lane, select the shared menu option through
  the existing Rust UI boundary, and inspect a WebGPU split frame.
- Run affected native, Wasm, web ownership, Android build/check, formatting,
  and source-contract gates.

### Slice 5 — closeout

- Reconcile this tactical, the couch topic, tactical index, and platform
  capability evidence.
- Record the real-controller and future authoritative two-player gates without
  claiming couch multiplayer support.

## Required Gates

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-native-client \
  --bin mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client \
  --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
```

Use the Android build scripts if packaging validation becomes necessary. Save
all screenshots under `/tmp` and inspect them before accepting a rendered
milestone.

## Completion Bar

- The shared menu can select Off/Horizontal/Vertical on every flat host while
  menus themselves remain full-surface.
- Native desktop, browser/WebGPU, and flat Android consume the same Rust mode,
  layout, scene frame, and GPU compositor contracts.
- The primary pane has working HUD and ordinary interaction; the auxiliary
  pane follows with an independent camera and no authority or UI.
- Resize and orientation changes rebuild targets without stale aspect/depth or
  cross-pane pixels.
- Mono remains the allocation-free, one-view fast path.
- Product TypeScript contains no split semantic vocabulary.
- Inspected native and browser pixels plus affected automated gates pass.

## Outcome

The tactical completed without crossing any stop condition. Flat gameplay now
has one session-local `Off -> Left / Right -> Top / Bottom -> Off` setting in
the shared Debug menu. Desktop winit, browser WebGPU, and flat Android ask the
same Rust scene owner for a validated layout and render the same primary HUD
view plus deterministic world-only follow view through a shared GPU presenter.
Menus restore the full surface and XR projects the option unavailable.

The ordinary player remains the only participant, client endpoint, input
owner, interaction source, audio listener, and authoritative interest source.
The auxiliary camera does not widen residency. A visible embedded-world
preview temporarily suppresses the split because combining those composition
modes still needs an explicit product contract.

Production browser TypeScript did not gain split vocabulary or state. It
continues to size the canvas, collect DOM facts, and invoke the Rust frame
boundary. Only the query-gated `.mjs` validation harness knows how to click the
setting and classify the resulting pixels.

Implementation commits:

- `0e39ca8a` — tactical and topic contract;
- `06ac9663` — shared setting, capability projection, menu, and camera policy;
- `556fc10c` — shared GPU presentation, three flat-host integrations, and
  rendered acceptance.

## Validation Evidence

- Shared unit suites passed: 306 `mclone-app-runtime`, 130 `mclone-scene`, and
  97 `mclone-ui` tests. These include exact odd-sized horizontal/vertical
  rectangles, one-to-four pane attachment specs, menu cycling, flat/XR
  capability projection, follow-camera aspect, prepare-once/render-many, and
  mono-path policy.
- The native-client binary suite passed all 171 tests. The dual-view capture
  now uses the live GPU presenter and reported one shared preparation, two
  rendered views, primary-only HUD commands, and 125,291 differing paired
  pane pixels. Inspected outputs are
  `/tmp/mclone-aux-split-20260722/horizontal.png` and
  `/tmp/mclone-aux-split-20260722/vertical.png`.
- The full `mclone-web-client` unit and ownership-lock suite passed. Its source
  lock rejects split mode, pane count, layout, and presentation semantics in
  production TypeScript.
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown` passed.
  `pnpm native:web:auxiliary-split-smoke` selected the option through the real
  menu on headed Wayland WebGPU, found 460,346 differing paired pixels and no
  transparent interior pixels, and wrote its report to
  `/tmp/mclone-native-web-auxiliary-split.json`. The inspected menu and frame
  captures are `/tmp/mclone-native-web-auxiliary-split-menu.png` and
  `/tmp/mclone-native-web-auxiliary-split-horizontal.png`.
- `pnpm native:android:apk` compiled the actual Android arm64 adapter and
  assembled `android/app/build/outputs/apk/debug/app-debug.apk` successfully.
- The full `cargo fmt --all -- --check` gate passed after mechanical commit
  `e34418dc` corrected an unrelated pre-existing worldgen test reflow.
  JavaScript passes `node --check`, and `git diff --check` passed before each
  implementation commit.

No physical gamepad, Android device, Steam Deck, or console hardware was
available. Those remain hardware acceptance rather than a prerequisite for
this single-participant presentation mode.

## Deferred Product Gates

This is an auxiliary debug/product-development feature, not couch multiplayer.
The next authoritative two-player slice still requires explicit decisions on
participant profiles, device assignment/join UX, per-player HUD and menu
ownership, pause semantics, audio listening, observer interest, and shared
client-resource ownership. Real controller acceptance should precede claiming
that controller-first couch onboarding is usable.

## Stop Conditions

Stop and ask for direction if the work requires choosing:

- a persisted or shipping default layout;
- a second profile, player, input assignment, or owner-private HUD;
- per-participant versus global pause/menu behavior;
- distant observer interest for the auxiliary camera;
- helper mutation authority;
- audio-listener policy; or
- shared resource ownership between two authoritative client replicas.
