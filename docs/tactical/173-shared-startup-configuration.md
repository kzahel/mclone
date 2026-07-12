# 173: Shared Startup Configuration

Status: active; opened 2026-07-12. Slice 0 inventory and characterization
landed; Slice 1 canonical defaults and Android overlays landed; Slice 2 began
with canonical scene-host retention.

Workstream: native Rust shared launch/startup configuration in
`mclone-app-runtime` and `mclone-scene`, with adoption by desktop flat,
offscreen, web/WASM, flat Android, desktop XR, and Android XR. This is a
configuration-boundary cleanup, not a movement-mode feature tactical.

## Motivation

Adding a hypothetical `--movement-mode fly` option is a useful architecture
test. Parsing the spelling once is already possible, but carrying one new
shared value from launch input to the shared scene currently appears to require
edits in many app and adapter files. The behavior is trivial; the diff is not.

The current path is approximately:

```text
process argv / Android launch argv / browser URL query
  -> mclone-app-runtime::startup_args::StartupArgState
  -> StartupOptions / StartupSceneOptions
  -> desktop SceneOptions, Android defaults, or web JS scalar fields
  -> McloneSceneHostOptions
  -> one of several camera/session construction sites
```

The parser is shared, but the parsed configuration is not retained end to end.
Shared values are copied into overlapping records, manually enumerated in
conversion functions and full default literals, and expanded into scalar
arguments at the browser boundary. This is why a shared option can still create
platform-shaped plumbing.

Tacticals [`167`](167-shared-session-startup-contract.md),
[`168`](168-unified-native-scene-host.md), and
[`170`](170-web-scene-host-adoption.md) unified startup execution and the scene
host. This tactical follows through on their intended thin-adapter shape for
startup *configuration*.

## Problem Statement

There are three overlapping configuration records in the live path:

- `mclone-app-runtime::startup_args::StartupSceneOptions` owns the values
  accepted by the shared argv/query parser.
- `mclone-native-client::cli::SceneOptions` repeats shared scene fields beside
  genuine desktop/harness fields.
- `mclone-scene::McloneSceneHostOptions` repeats parsed scene fields beside
  shared host policy, storage projection, validation, and harness controls.

The duplication creates several failure modes:

- A new shared field compiles only after every full struct literal is updated,
  even when a platform has no distinct behavior.
- Manual `to_*` / `from_*` projections can silently drop a value or replace it
  with a default.
- Native app adapters appear to own engine startup policy because their DTOs
  restate it.
- Web query parsing happens in Rust, but TypeScript receives and forwards
  individual shared scalars, creating a second schema at the wasm boundary.
- Shared camera settings are applied at multiple camera construction and
  replacement sites rather than by one configured camera/session factory.
- Lists such as `STARTUP_ARG_FLAGS` and `STARTUP_QUERY_KEYS` protect parser
  coverage, but they do not prove that a parsed value reaches the scene host.

The issue is not that every platform must use process CLI syntax. Each platform
may obtain launch values differently. The issue is that source adaptation and
shared configuration ownership are currently entangled with value projection.

## Necessary Platform Differences

The target must preserve real rim responsibilities:

- Desktop owns process argv, window/surface modes, filesystem paths where they
  are platform facts, and desktop diagnostics/probes.
- Flat Android and Android XR own launch-intent/property collection, activity
  lifecycle, app-private roots, and device validation controls.
- Web owns URL query collection, browser storage/worker/socket resources,
  canvas lifecycle, and promise-based startup mechanisms.
- XR adapters own OpenXR runtime/session/swapchain controls and XR-only
  validation flags.
- Offscreen/perf harnesses own capture paths, scripted input, frame counts, and
  measurement policy.

Those adapters may translate raw strings into the shared parser and supply
platform resources. They should not restate every shared scene field or decide
its semantics.

## Target Shape

Retain one canonical, host-neutral startup configuration after parsing:

```text
platform launch source
  -> shared token/query parser
  -> SharedStartupConfig
       |- scene/session product configuration
       |- render configuration
       |- initial camera configuration
       `- storage intent (not a resolved platform resource)
  -> McloneSceneHost configured once
  -> centralized session/camera construction
```

Platform and harness controls sit beside, rather than copy, the shared value:

```rust
struct DesktopRunOptions {
    shared: SharedStartupConfig,
    window: WindowOptions,
    diagnostics: DesktopDiagnostics,
}

struct AndroidLaunchOptions {
    shared: SharedStartupConfig,
    app_world_root: Option<PathBuf>,
    surface: AndroidSurfaceOptions,
}
```

The exact final names should follow the existing public API where practical.
`StartupOptions` may be evolved into the canonical value instead of introducing
a synonym. The invariant matters more than the name:

> Once a shared launch value is parsed, app/platform code carries the canonical
> configuration without enumerating its fields.

`McloneSceneHostOptions` should either contain the canonical shared scene
configuration or consume it through a single lossless constructor. It should
not duplicate parsed fields and then expose bidirectional hand-written
projections. Host-only policy may remain in a separate nested record.

## Ownership Rules

`mclone-app-runtime::startup_args` owns:

- canonical launch configuration and host-neutral defaults;
- argv and browser-query spellings for shared values;
- parsing, validation, aliases, and useful error messages;
- storage intent and remote/local launch intent before resources are resolved;
- source-independent parser and round-trip/conformance tests.

`mclone-scene` owns:

- consuming canonical shared scene configuration;
- shared host/session policy not selected by platform syntax;
- centralized player-camera/session initialization;
- preserving configured values across local/remote startup and session or
  camera replacement.

App/platform crates own:

- collecting argv, query parameters, intent extras, or properties;
- true platform defaults as a small overlay or builder call;
- resolving storage, transport, surface, and lifecycle resources;
- platform-only run modes and validation flags;
- presenting source-appropriate help.

App crates must not introduce a parallel enum or field for a shared startup
setting merely because one source needs it first.

## Defaulting Contract

Full platform-local literals of the canonical shared record are forbidden as
the steady-state shape. Use a neutral `Default` plus named overlays:

```rust
let shared = SharedStartupConfig::default()
    .with_initial_time_frozen_at(6000)
    .with_render_distance(5)?;
```

An overlay must state why the platform differs. Adding an unrelated shared
field must not require editing that overlay. Defaults that are actually product
or host-profile policy belong in shared constructors, not in every platform.

## Browser Boundary

Rust already parses the URL query. Do not immediately explode the result back
into a TypeScript-owned list of shared scalar fields.

Preferred shapes, in order:

1. Keep the parsed Rust configuration behind an opaque wasm-bindgen handle and
   pass that handle to the Rust scene-host factory.
2. If TypeScript must choose a browser resource, expose a narrow derived plan
   or accessor such as local-worker versus remote-WebSocket, then return the
   same configuration handle to Rust.
3. Use one versioned serialized configuration object only if an opaque handle
   is impractical. Rust remains the schema owner and validates it once.

Do not add another positional/scalar parameter to the local and remote web
scene constructors for each new shared option.

## Central Application Contract

Shared camera/session settings must be applied at one construction boundary.
Camera replacement paths must use the same configured factory or copy a typed
camera configuration object, rather than reapplying selected scalars manually.

For the movement-mode canary, for example, the shared scene owner would set the
initial movement and collision state once. Desktop, web, Android, and XR
adapters would not call `set_movement_mode` during startup.

## Slice 0: Inventory And Classification

- [x] Inventory every field in `StartupSceneOptions`, desktop `SceneOptions`,
  and `McloneSceneHostOptions`.
- [x] Classify each field as shared launch configuration, shared host policy,
  platform resource/fact, or harness-only control.
- [x] Inventory every complete literal and `to_*` / `from_*` projection of
  those records, including web Rust-to-JS scalar projection.
- [x] Inventory all camera/session constructors that manually apply startup
  settings such as movement speed, visibility, collision, or camera mode.
- [x] Record the before-state footprint using one existing tracer field such as
  `movement_speed_multiplier`; do not add a synthetic production option.
- [x] Lock current defaults and local/remote behavior with focused tests before
  changing ownership.

Exit criteria: every duplicated field and projection edge has an explicit
owner and migration destination. No field is moved merely because it is near
another field.

### Slice 0 Evidence And Ownership Map

Inventory completed 2026-07-12. The field groups and destinations are:

- Canonical shared scene/session configuration: seed, initial chunk, render
  distance, render-compile request/count/queue limit, remote launch intent,
  initial/frozen day time, movement speed, passive-showcase policy, lighting
  and light batch size, and far-LOD configuration. These stay in
  `StartupSceneOptions`, nested in the retained canonical `StartupOptions`.
- Canonical render and initial-camera configuration: textured-section render
  options and optional eye/target already live beside `scene` in
  `StartupOptions`; they remain canonical siblings rather than scene fields.
- Canonical storage intent: transient/default-root intent plus optional world
  root/directory remain in `StartupWorldStorageOptions`; resolved platform
  roots remain platform resources.
- Shared scene-host policy: simulation cadence, first-person visibility,
  initial-spawn centering, scheduled-fluid freeze, adaptive publication and
  admission, LOD prewarm, underwater detection, debug UI screen, and actor
  suppression belong in a host-policy record owned by `mclone-scene`.
- Platform/harness facts: desktop worker timing, render-capacity reporting
  mode, resolved filesystem roots, XR validation controls, window/surface
  controls, screenshot/probe dimensions and paths, and scripted/perf cadence
  remain outside canonical shared product configuration.

Before-state projection edges are `StartupArgState::finish` into
`StartupOptions`; desktop `SceneOptions::from_startup_scene` and
`to_startup_scene`; desktop `scene_host_options_from_desktop` and the reverse
remote-session projection; scene `McloneSceneHostOptions::from_startup_scene`
and `to_startup_scene`; flat-Android and Android-XR default/projection helpers;
and web `startup_options_to_js_value` followed by scalar local/remote scene-host
constructors. Complete host literals also remain at the two web construction
sites and desktop construction site. Test literals are numerous but do not
create production ownership; migration should convert fixtures only as their
owning production API changes.

Camera/session application sites are the mono host camera creation and the
local/remote, replacement, and startup-pose paths in `mclone-scene::session`.
The current replacement path deliberately copies the runtime camera speed;
that behavior must remain distinct from applying launch defaults to a newly
started session.

`movement_speed_multiplier` currently appears in the canonical parser record,
desktop DTO and both desktop projections, scene-host record and both scene
projections, web JS object and both scalar constructors, multiple scene camera
construction sites, and the runtime settings/effect path. The first migration
commit adds argv/query canonical-equivalence coverage, preserves the existing
host projection tests, makes `StartupOptions` directly defaultable, and
replaces flat Android's complete shared literal with the named
`with_initial_time_frozen_at(6000)` overlay.

## Slice 1: Canonical Shared Configuration

- [x] Make `StartupOptions` (or a deliberately renamed replacement) the
  canonical retained launch value.
- [ ] Separate canonical shared scene/session settings from render, initial
  camera, storage intent, and source-local options without duplicating fields.
- [ ] Replace desktop `SceneOptions` shared-field copies with a nested canonical
  configuration plus desktop/harness-only records.
- [x] Replace full Android shared-option literals with neutral defaults and
  named, minimal overlays.
- [ ] Make invalid combinations fail in shared validation before a platform
  resource is created.
- [x] Add equality/round-trip tests proving that native argv and web query
  sources produce equivalent canonical values for equivalent inputs.

Flat Android now uses the shared neutral default plus a named fixed-time
overlay. Android XR now uses the canonical neutral default directly instead of
deriving it through a scene-host round-trip.

Exit criteria: adding a field to canonical shared scene configuration does not
force a mechanical edit to desktop or Android option structures.

## Slice 2: Scene-Host Consumption And Camera Factory

- [x] Make `McloneSceneHostOptions` contain or losslessly consume canonical
  shared configuration without restating its parsed fields.
- [ ] Split host-only tuning and harness controls into nested records where
  that avoids false platform ownership.
- [ ] Delete bidirectional projections that enumerate shared fields.
- [ ] Introduce one shared configured camera/session construction path.
- [ ] Route initial local, initial remote, menu-driven replacement, startup
  pose replacement, and mono/stereo setup through that path.
- [ ] Preserve runtime user changes across camera replacement where the current
  behavior requires it; distinguish those from launch defaults explicitly.
- [ ] Add tests for initial startup and replacement so configured values cannot
  reset silently.

Exit criteria: scene and camera policy have one shared application point, and
platform adapters pass resources and cadence facts rather than startup values.

The first Slice 2 commit nests `StartupSceneOptions` directly in
`McloneSceneHostOptions`, removes all thirteen restated parsed fields, and
deletes the scene host's enumerating reverse projection. `Deref`/`DerefMut`
temporarily preserve source compatibility for shared scene internals while the
remaining desktop DTO is migrated. Desktop host construction now supplies its
one parser projection as the nested value and enumerates only host policy.
Web local/remote constructors now build that same nested record; the remote
constructor also retains its remote launch intent instead of allowing the old
reverse projection to replace it with `None`. The opaque web configuration
handle and scalar-constructor deletion remain Slice 3 work.

## Slice 3: Browser Configuration Handle

- [ ] Keep parsed query configuration Rust-owned across the wasm boundary.
- [ ] Replace the TypeScript `movementSpeedMultiplier`-style scalar schema with
  an opaque configuration handle or one versioned Rust-owned object.
- [ ] Expose only the derived browser startup facts TypeScript needs to choose
  worker, IndexedDB, or WebSocket resources.
- [ ] Make local-worker, IndexedDB, and remote-WebSocket creation return to one
  Rust scene-host constructor with the same canonical configuration.
- [ ] Delete duplicated TypeScript defaults for shared settings.
- [ ] Add web tests for query parsing, local and remote launch, and preservation
  of at least two unrelated shared values through host creation.

Exit criteria: a new shared startup field does not add a TypeScript interface
field or another positional argument to multiple web constructors.

## Slice 4: Movement Mode As The Canary

This slice intentionally uses the original small request only after the
configuration shape is fixed.

- [ ] Add one shared movement-mode startup value with accepted labels
  `walk`, `fly`, `hand-push`, and `thruster` (final product labels may follow
  the existing shared movement taxonomy).
- [ ] Define its collision interaction through the existing shared movement
  experience reducer; do not encode a desktop shortcut assumption in startup
  parsing.
- [ ] Add `--movement-mode` to shared argv parsing and `movementMode` to shared
  browser-query parsing.
- [ ] Apply it through the single shared scene/camera configuration boundary.
- [ ] Prove desktop flat, offscreen, web, flat Android, desktop XR, and Android
  XR consume the value without platform-specific setting code.
- [ ] Record the actual diff footprint and compare it with the acceptance
  budget below.

Exit criteria: the canary adds no platform DTO field, no app-local movement
enum, no platform startup setter, and no web scalar-constructor parameter.

## Slice 5: Enforcement And Documentation

- [ ] Add a source or API tripwire that rejects new app-owned copies of
  canonical shared startup fields.
- [ ] Add conformance tests covering all shared argv consumers and the browser
  query source against the same expected canonical configuration.
- [ ] Keep `STARTUP_ARG_FLAGS` / `STARTUP_QUERY_KEYS` coverage checks, but add a
  propagation test that reaches `McloneSceneHost` configuration.
- [ ] Reject complete canonical shared-config literals in app crates unless a
  narrowly documented test fixture needs one.
- [ ] Update `docs/native-engine-architecture.md` with the canonical config and
  platform-source boundary.
- [ ] Update `docs/topics/platform-parity.md` only if visible support or a lane
  contract changes.
- [ ] Update this tactical and the tactical index with landed evidence.

Candidate mechanical checks:

```bash
rg -n "struct SceneOptions|StartupSceneOptions \{|McloneSceneHostOptions \{" \
  native/apps native/crates/mclone-scene/src

rg -n "movementSpeedMultiplier|renderDistance|debugPassiveShowcase" \
  native/apps/mclone-web-client/www native/apps/mclone-web-client/src
```

The checks should be encoded as focused tests or a maintained script with an
allowlist, not left as prose-only audit commands.

## Acceptance Budget

After the migration, adding an ordinary shared startup value should normally
touch only:

1. the canonical configuration/parser and its unit tests;
2. the single shared subsystem application point and its tests;
3. user-facing help or documentation.

A platform adapter edit is acceptable only when the new value has a genuinely
new source representation, platform resource, capability, or platform-specific
default. “The adapter copies all fields” is not an acceptable reason.

The movement-mode canary should be rejected as incomplete if it requires:

- edits to both desktop flat and desktop XR adapters;
- edits to both flat Android and Android XR adapters;
- a TypeScript mirror field plus multiple wasm constructor parameters;
- manual startup setters in more than one camera/session constructor.

## Validation

Use the current commands from [`../platforms.md`](../platforms.md#validation-policy).
At minimum for the configuration refactor:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:typecheck
pnpm native:desktop-offscreen:smoke
pnpm native:web:smoke
pnpm native:android:apk
pnpm native:android-xr:apk
git diff --check
```

Any slice that changes camera initialization or produces pixels must capture
and inspect a native headless screenshot at its first drawable milestone, per
the repository validation policy. Device validation is required when a slice
changes a device-specific source adapter or launch default; mechanical
canonical-config migration alone should not invent a platform behavior change.

## Out Of Scope

- Unifying platform-only diagnostic and perf flags into engine configuration.
- Making web use process-style CLI syntax.
- Moving window, surface, OpenXR, Android activity, or browser resource
  ownership into shared crates.
- Defining a stable public serialization format for every startup option.
- Replacing runtime preferences or persisted client-experience settings with
  launch arguments.
- Implementing movement mode before the configuration boundary is ready.
- Broadly reorganizing completed startup/scene-host tacticals 167–170.

## Relationship To Existing Work

- [`147`](147-shared-movement-experience-settings.md) owns movement taxonomy,
  collision compatibility, and shared reducer semantics. This tactical owns
  launch-time configuration propagation only.
- [`167`](167-shared-session-startup-contract.md) owns the unified startup
  execution/readiness pump. This tactical supplies its configuration without
  reopening readiness policy.
- [`168`](168-unified-native-scene-host.md) and
  [`170`](170-web-scene-host-adoption.md) own the shared host and thin platform
  drivers. This tactical removes remaining configuration DTO duplication at
  their boundary.
- [`171`](171-convergence-and-parity-closeout.md) remains the coordinating
  convergence parent. This tactical is a focused follow-up discovered by using
  a small startup option as a change-footprint probe.
