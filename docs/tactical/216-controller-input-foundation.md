# Tactical 216: Controller Input Foundation

Status: active 2026-07-22.

Topic: [`controller-input`](../topics/controller-input.md).

## Instruction Synthesis

Implement the accepted all-target controller foundation unattended and commit
it as a reviewable series. Complete shared semantic actions, contexts,
per-source state, scene routing, controller-accessible UI, ordinary desktop,
browser, and Android collectors, and the overlapping OpenXR action refactor.
Add preferences, rebinding, prompt-layout, haptic, scripted-input, and
cross-target validation foundations where they can be proved without physical
hardware.

Do not claim final controller acceptance without real Xbox-like,
PlayStation-like, generic, browser, Android, Steam Deck, and XR device passes.
Record those device gates explicitly instead. Native Steam Input, exact Steam
action-origin glyphs, gyro/trackpad specialization, and tactile-quality tuning
remain later device/runtime-backed work.

## Starting Point

- `mclone-input` owns capability/preference resolution, keyboard/mouse and touch
  adapters, gamepad bindings/dead zones, a first flat gamepad adapter,
  session-local `InputSourceId`, neutral descriptors, a complete normalized
  `StandardGamepadSnapshot`, and deterministic four-seat assignment.
- No shipping desktop, browser, flat Android, or Android XR host currently
  polls an ordinary physical gamepad.
- `FlatInputFrame` still combines mouse/touch deltas with right-stick look and
  therefore cannot express presentation-rate-independent controller turning.
- `MonoInteractiveInputRouter` owns keyboard/mouse state while app hosts merge
  touch separately; it does not own a multi-source controller session.
- `mclone-ui::GuiKey` exposes only Escape and F1. Focus visuals exist, but
  directional traversal, confirm/back, repeat, and controller-origin prompts
  do not.
- OpenXR retains neutral poses but exposes physically named face-button actions
  to shared scene policy.
- Flat Android uses `winit 0.30`, whose Android backend drains the raw activity
  queue and currently maps motion events without exposing gamepad source/axes.
  Android XR already owns the raw `android_activity` queue directly.

## Architecture Invariants

1. Platform collectors emit neutral snapshots or already-semantic backend
   actions. They never own gameplay, UI, binding, dead-zone, repeat, prompt, or
   participant policy.
2. `mclone-input` remains free of `winit`, `web-sys`, Android, OpenXR, GilRs,
   Steamworks, scene, renderer, and app dependencies.
3. Mouse/touch pointer delta and controller look rate remain distinct until the
   shared scene applies rate using frame `dt`.
4. Held, pressed, and released facts are derived once from per-source state.
   Platform event repeat is not an action edge.
5. Drift below the normalized dead zone never becomes activity or switches the
   displayed prompt.
6. Disconnect, focus loss, pause, visibility loss, and source replacement clear
   held state deterministically.
7. Input contexts are shared facts selected by the scene/UI state. The same
   physical south button may resolve to Jump in gameplay and Confirm in menus.
8. Tracked XR controllers converge at semantic actions but retain hand, pose,
   ray, validity, and XR-only comfort facts in a typed extension.
9. Ordinary gamepad use in XR supplies non-spatial actions and uses a shared
   head-gaze/crosshair fallback for Attack/Use when no tracked ray is selected.
10. Existing keyboard, mouse, touch, tracked-controller, mono, stereo,
    multiview, headless, and scripted-input paths remain regression gates.
11. A platform capability is advertised only when a real collector is active;
    synthetic fixtures never make a shipping host claim hardware support.
12. Real-device acceptance remains open until the exact platform/device pass is
    recorded, even when builds, unit tests, mocks, and emulation are green.

## Slice Plan

### Slice 0 — plan and source locks

- Add this tactical and link it from the controller topic and tactical index.
- Record source locks for shared ownership and forbidden platform types.
- Capture the exact shared, native, WASM, browser, Android, and XR gates used by
  later slices.

### Slice 1 — semantic controller session

- Add shared `InputContext`, semantic action identifiers, continuous axes,
  held/pressed/released sets, activity/source facts, and prompt origins.
- Add per-source snapshot history, stick curves, trigger hysteresis,
  navigation repeat, disconnect/focus clearing, and active-source arbitration.
- Separate `look_rate` from `look_delta` and prove frame-rate invariance.
- Preserve a bounded compatibility projection to `FlatInputFrame` while
  existing hosts migrate.
- Exercise multiple sources, reordered samples, drift, non-finite values,
  context changes, duplicate samples, edges, reconnect, and mixed input.

### Slice 2 — shared scene routing and scripted proof

- Make `MonoInteractiveInputRouter` own the controller session and accept
  connect/sample/disconnect/lifecycle events.
- Select Gameplay/Menu/TextEntry context from shared scene state.
- Apply semantic continuous and edge state once to camera, locomotion,
  interaction, hotbar, UI, and client commands.
- Add offscreen/scripted traces that prove equivalent overlapping outcomes for
  keyboard, gamepad, and XR-shaped action input.

### Slice 3 — controller-complete shared UI

- Add directional navigation, confirm, back, page/tab, stable focus order,
  disabled-widget skipping, slider adjustment, and deterministic repeat.
- Cover title, world list/create/delete, New World, Join Remote, pause,
  options/categories, server settings, asset packs, storage confirmation, help,
  block palette/inventory, and failure/recovery surfaces.
- Replace literal Xbox prompt assumptions with semantic action plus
  layout/origin projection and retain text fallback when no glyph exists.
- Capture and inspect focused/menu/prompt pixels outside the repository.

### Slice 4 — desktop and browser collectors

- Add an app/platform-owned GilRs collector for desktop flat and desktop XR.
- Translate hotplugged cached state into `StandardGamepadSnapshot`, preserving
  neutral source identity and clearing on focus/lifecycle loss.
- Poll the browser Gamepad API near `requestAnimationFrame`, normalize only the
  W3C standard mapping, and keep TypeScript domain-blind.
- Add collector conversion tests, browser mocks, capability/activity gates,
  and desktop/web build and smoke coverage.

### Slice 5 — Android collectors

- Add one neutral Android raw-controller normalizer in
  `mclone-android-platform` for source classification, standard axes/buttons,
  lifecycle, and stable session-local device handles.
- Feed it from one shared source-aware Java activity bridge in both Android
  packages, before flat Android forwards non-controller events to `winit`.
- Keep the bridge mechanical: enumerate devices, forward descriptors and raw
  standard keys/axes, and let the Rust normalizer own identity and mapping.
- Build both APK lanes and add pure conversion/lifecycle tests. Leave wired and
  Bluetooth hardware acceptance open.

### Slice 6 — XR semantic convergence

- Separate shared semantic action state from tracked controller pose/ray state.
- Rename OpenXR application actions around meaning and keep physical controls
  in suggested interaction-profile bindings.
- Preserve existing tracked locomotion, interaction, teleport, hand-push,
  thruster, menu, stereo, and multiview behavior.
- Route ordinary gamepad actions through both XR hosts and add head-gaze
  interaction fallback without pretending the gamepad has a pose.
- Prove the shared action seam through XR emulation and existing OpenXR compile
  and source-lock gates; leave headset feel/regression acceptance open.

### Slice 7 — preferences, rebinding, haptics, and closeout

- Persist controller sensitivity, inversion, dead zones, layout override,
  preferred input, and schema-versioned semantic bindings through shared
  preference codecs and existing platform storage executors.
- Add a neutral haptic request/capability contract and platform no-op behavior;
  add backend execution only where it can be compiled and deterministically
  tested without claiming tactile quality.
- Reconcile Tactical 098, this tactical, the controller topic, platform matrix,
  validation evidence, and remaining real-device checklist.
- Run the full affected gate matrix and commit each independently validated
  slice with `Topic: controller-input`.

## Validation Gates

Minimum shared gates for every affected slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-input
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml -p mclone-input \
  --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml -p mclone-scene \
  --target wasm32-unknown-unknown
pnpm native:thin-adapters:purity
pnpm native:scene-host:purity
pnpm native:xr:frame-driver:purity
```

Use the current commands in [`../platforms.md`](../platforms.md) for desktop,
browser, Android, and XR adoption. Android builds always run through the
repository scripts. Browser pixels use the headed Wayland lane after
`pnpm host:check`; generated captures go under `/tmp` and are inspected.

## Hardware-Only Acceptance Ledger

These remain open throughout unattended implementation unless an applicable
device becomes available and evidence is recorded:

- Xbox-like, PlayStation-like, and generic desktop controllers;
- Steam Deck built-in controller, suspend/resume, and prompt layout;
- Steam Controller compatibility through the ordinary virtual-gamepad path;
- real controller in each supported browser family;
- wired and Bluetooth controllers on flat Android;
- ordinary gamepad plus tracked controllers in desktop XR;
- ordinary gamepad plus tracked controllers on Quest/Android XR;
- haptic strength/feel and vendor-specific layout/glyph correctness.

## Progress Evidence

### Slice 1 — semantic controller session

Implemented on 2026-07-22. `mclone-input::ControllerInputSession` now owns
Gameplay/Menu/TextEntry contexts, per-source normalized snapshots, semantic
held/pressed/released sets, continuous movement and look rate, active source
and layout projection, radial dead zones with response curves, trigger
hysteresis, navigation threshold/repeat state, lifecycle clearing, and
deterministic multi-source arbitration. `PlayerActionFrame::to_flat_frame`
provides the bounded migration projection and integrates look rate using
clamped frame `dt`; mouse/touch pointer delta remains separately represented.

The standard control vocabulary now covers all seventeen standard buttons.
Defaults use right/left trigger for Attack/Use, south for Jump, east for Sneak,
left-stick click for Sprint, shoulders/D-pad for hotbar stepping, Start for
Menu, and Select for the block palette. Existing keyboard, touch, XR-emulation,
assignment, and legacy adapter tests remain green.

Focused evidence:

```text
cargo test -p mclone-input: 34 passed
cargo check -p mclone-input --target wasm32-unknown-unknown: passed
```

Tests cover 60/120 Hz look invariance, drift filtering, exact action edges,
trigger hysteresis, context-change non-replay, monotonic navigation repeat,
meaningful-source arbitration, and disconnect release generation. Shipping
hosts do not consume this session yet; Slice 2 owns adoption.

### Slice 2a — shared scene router adoption

Implemented on 2026-07-22. `MonoInteractiveInputRouter` now owns the semantic
controller session beside keyboard/mouse held state. It exposes neutral source
connect/disconnect/sample entry points, selects Gameplay or Menu from the real
scene UI state, retains the latest semantic frame, composes controller movement
with keyboard/touch in the existing shared held-frame advance, and clears all
held device classes through one transient/lifecycle path.

Controller action edges route through the existing shared gameplay/UI dispatch
boundary. Continuous movement and look rate apply only during shared frame
advancement; the immediate edge route explicitly excludes them, preventing
double turning. A focused router trace proves gameplay movement/Jump, a context
transition, menu navigation, and exhaustive clearing through one source.

Focused evidence:

```text
cargo test -p mclone-scene interactive_input::tests: 3 passed
cargo test -p mclone-input -p mclone-scene: 34 + 128 passed
cargo check -p mclone-scene --target wasm32-unknown-unknown: passed
```

No platform collector calls the new route yet. Slice 3 completes the shared
focus and activation consumer required before product adoption.

### Slice 3 — controller-complete shared UI

Implemented on 2026-07-22. `mclone-ui::GuiNavigation` now provides directional,
confirm, back, and previous/next-page input over the retained UI surface.
Navigation uses committed widget geometry, stable wraparound order, skips
disabled or non-actionable widgets, adjusts focused sliders in bounded steps,
and delegates Back to the exact existing Escape policy. Pointer activity clears
controller focus so mouse hover and controller focus cannot leave competing
visual states.

`MonoInteractiveInputRouter` maps semantic menu actions through that contract
and applies returned `GameUiAction` values through the same scene owner as
pointer and keyboard input. Keyboard arrows and Space also reuse navigation;
controller repeat remains owned by `ControllerInputSession` rather than OS key
repeat. The covered surfaces include every non-text menu surface with an
available committed action, while populated world, asset-pack, and block-palette
rows retain their dynamic action ownership.

Gamepad HUD prompts now resolve semantic bindings through
`ControllerLayoutFamily`, with Xbox-like, PlayStation-like, Nintendo-like,
Steam/Deck-like, generic, and unknown text projections. The hidden/visible HUD
API no longer silently assumes Xbox labels. A narrow
`--screenshot-controller-focus` diagnostic establishes controller focus before
an offscreen UI capture.

Focused evidence:

```text
cargo test -p mclone-ui: 95 passed
cargo test -p mclone-scene: 128 passed plus integration suites
cargo test -p mclone-native-client
  cli_parses_full_frame_screenshot_options: passed
cargo check -p mclone-ui -p mclone-scene -p mclone-web-client
  --target wasm32-unknown-unknown: passed
controller-focus screenshot: 960x540, 97 GUI commands, visually inspected
```

The inspected `/tmp/mclone-controller-focus.png` capture showed a clear focused
outline on the first enabled Options row with the remaining menu geometry and
world composition intact. The capture remains outside the repository. Physical
device navigation and prompt-family acceptance remain in the hardware ledger.

### Slice 4 — desktop-flat and browser collectors

Implemented on 2026-07-22. The desktop-flat host now owns a GilRs 0.11
collector that drains hotplug events and polls cached connected state before
shared held-frame advancement. It retains session-local source identity across
reconnects, classifies prompt layout from neutral name/vendor facts, and emits
only `StandardGamepadSnapshot`. GilRs default filtering is disabled so the
shared controller session remains the sole dead-zone/activity owner; force
feedback remains disabled until the shared haptic contract lands.

Browser Rust now polls `navigator.getGamepads()` at its animation-frame
boundary, accepts only the W3C standard mapping, normalizes its four axes and
seventeen buttons, and feeds the same scene router. TypeScript remains unaware
of buttons, actions, bindings, and dead zones; it consumes only the generic
pointer-capture and transient-clear effects reported by the Rust scene host.
Browser mock tests cover standard mapping, axis orientation, analog triggers,
hotplug, reconnect, and reused browser indices.

Both hosts advertise gamepad capability only while a compatible source is
connected and switch prompt origin only on meaningful post-dead-zone activity.
Lifecycle clearing suppresses a physically held control until a neutral sample
is observed, preventing focus-resume replay. The active layout now reaches the
shared HUD prompt projection. Ordinary gamepads in desktop XR remain Slice 6
work so adoption happens with the tracked/semantic convergence rather than as
a second XR-only merge.

Focused evidence:

```text
cargo test -p mclone-input: 36 passed
cargo test -p mclone-scene: 128 passed plus integration suites
cargo test -p mclone-native-client --bin mclone-native-client: 171 passed
cargo test -p mclone-web-client: 38 passed plus integration suites
cargo check -p mclone-web-client --target wasm32-unknown-unknown: passed
cargo fmt --all -- --check: passed
pnpm native:thin-adapters:purity: passed
pnpm native:scene-host:purity: passed
pnpm host:check: headed Wayland browser/WebGPU path available
pnpm native:web:typecheck: passed
pnpm native:web:app-smoke: passed; canvas pixels inspected
```

Desktop, Steam Deck, and real-browser device acceptance remains in the hardware
ledger. XR ordinary-gamepad adoption has not yet landed.

### Slice 5 — Android collectors

Implemented on 2026-07-22. Both Android packages now compile one shared
`ControllerInputBridge` Java source. It listens for device changes through
`InputManager`, classifies `GAMEPAD`, `JOYSTICK`, and `DPAD` sources, and
forwards only standard controller keys and motion axes through three JNI entry
points. Flat Android uses a small `NativeActivity` subclass so controller
events are consumed before `winit`; keyboard, mouse, touch, and system events
continue through the existing activity and surface host without a dependency
fork. Android XR owns the same bridge beside its OpenXR activity.

`mclone-android-platform::AndroidControllerCollector` is the pure Rust owner of
source-aware device reduction, session-local identity, hotplug/reconnect,
layout classification, Android axis orientation, right-stick and trigger
fallbacks, standard snapshot emission, and lifecycle clearing. It accepts
device-ID reuse without inheriting held state and can reannounce connected
sources when a rebuilt scene begins consuming input. A process-local queue is
the narrow JNI-to-frame boundary.

Flat Android drains the queue before redraw and routes snapshots through the
same `MonoInteractiveInputRouter` as desktop and browser. It advertises a
gamepad only while a compatible device is connected and changes prompt layout
only after meaningful shared-session activity. Android XR drains and retains
the same ordinary-device facts now; Slice 6 owns their semantic merge with
tracked OpenXR controls.

Focused evidence:

```text
cargo test -p mclone-android-platform: 13 passed
pnpm native:android:apk: arm64 and x86_64 APK builds passed
pnpm native:android-xr:apk: arm64 APK build passed
flat and XR native libraries: all three JNI symbols present
flat and XR APKs: classes.dex and target native library present
pnpm native:android:avd-smoke -- --skip-build: passed
AVD screenshot: 2000x1200 shared terrain/HUD pixels visually inspected
```

The AVD proves packaging, lifecycle, JNI linkage, and unchanged rendered
output, but has no physical controller. Wired/Bluetooth flat-Android and Quest
ordinary-gamepad acceptance therefore remain in the hardware ledger.

### Slice 6 — XR semantic convergence

Implemented on 2026-07-22. `mclone-input::XrInputFrame` now has three explicit
channels: shared `PlayerActionFrame`, pose-only tracked controller state, and
XR-specific spatial analog extensions. `XrInputFrameAssembler` owns shared
stick policy, trigger hysteresis, edges, and lifecycle re-arming. A separate
stateful action combiner derives aggregate edges after simultaneous tracked and
ordinary sources are composed, so one source cannot release an action another
source still holds.

`mclone-xr-host::OpenXrControllerActions` now creates semantic application
actions. Physical controller paths appear only in suggested OpenXR interaction
profile bindings. Desktop XR polls its existing GilRs collector and Android XR
polls the shared Android collector beside OpenXR; both feed one
`XrControllerInputRouter` before the same shared scene frame path.

The scene consumes common movement, turn, jump, sprint, sneak, descend, menu,
palette, attack, and use semantics without physical button names. It retains
tracked teleport, hand-push, thruster, poses, and controller pointer selection
as XR-only facts. Pose-less ordinary Attack/Use use the latest stereo head gaze,
while an available tracked aim ray remains preferred. Focus/session loss clears
both tracked and ordinary reducers and requires neutral before re-arming.

Focused evidence:

```text
cargo test -p mclone-input --lib: 40 passed
cargo test -p mclone-scene --lib: 127 passed
cargo check -p mclone-xr-host: passed
cargo check -p mclone-native-client --features xr: passed
cargo check -p mclone-input -p mclone-scene
  --target wasm32-unknown-unknown: passed
pnpm native:android-xr:apk: arm64 release APK passed
pnpm native:xr-emulation:smoke: 1280x640 stereo capture passed
XR emulation capture: world/menu pixels and per-eye parallax inspected
pnpm native:xr:frame-driver:purity: passed
pnpm native:thin-adapters:purity: passed
pnpm native:scene-host:purity: passed
```

The automated lane proves shared semantics, typed spatial separation, both app
build boundaries, and rendered stereo continuity. It does not validate button
feel, controller profiles, mixed-source ergonomics, or headset runtime behavior;
those remain explicitly open in the hardware ledger.

### Slice 7a — versioned preferences and neutral haptics

Implemented on 2026-07-22. `ControllerInputPreferences` is the persistable,
backend-neutral runtime profile for preferred input, layout override, dead
zones, response curves, look rate, horizontal/vertical inversion, controller
threshold/repeat tuning, and semantic `GamepadBindings`. Both the ordinary
controller session and XR action assembler consume the same normalized tuning;
changing a live profile clears and suppresses held input until neutral.

`mclone-app-runtime::ClientInputPreferences` now wraps controller and touch
policy in schema 1. The codec defaults fields added within the schema, rejects
unknown future schemas, reads the two legacy browser touch keys when no
document exists, and synchronizes those keys during migration. Browser Rust
loads and applies the complete profile. A native file executor writes
`preferences/input-preferences.v1.json` through an atomic temporary rename, and
factory reset removes only this newly registered preference beside the existing
registered files.

The shared haptic seam now has neutral active-gamepad, source, and XR-hand
targets, normalized low/high-frequency amplitudes, a bounded duration, source
capability counts, and an explicit unsupported no-op output. GilRs, Android,
OpenXR, and future Steam executors remain intentionally unadvertised until a
backend can compile and physical behavior can be accepted.

Focused evidence:

```text
cargo test -p mclone-input --lib: 43 passed
cargo test -p mclone-app-runtime input_preferences: 5 passed
native factory-reset preference test: passed
cargo test -p mclone-scene --lib: 127 passed
cargo test -p mclone-web-client: 38 passed plus integration suites
cargo check -p mclone-web-client --target wasm32-unknown-unknown: passed
pnpm native:thin-adapters:purity: passed
pnpm native:web:typecheck: passed
pnpm native:web:app-smoke: passed; live canvas pixels inspected
```

Slice 7b applies the native file profile in desktop flat/XR and Android flat/XR,
then closes the automated ledger without claiming real-device tuning or haptic
quality.

### Slice 7b — native preference adoption

Implemented on 2026-07-22. One native preference-path helper now loads the
schema-1 document or a normalized default for desktop flat, desktop XR, flat
Android, and Android XR. Every host applies the profile before constructing its
shared controller reducer. Both XR hosts apply identical sensitivity and
inversion to semantic OpenXR actions and identical complete bindings/tuning to
their ordinary-controller reducer.

Flat Android initializes touch sensitivity and visibility preference from the
same document. Touch-option UI changes use the atomic native executor and
preserve every controller field. Desktop and XR currently have no controller
settings editor, so they consume externally or previously persisted profiles
without inventing an app-local options model.

Focused evidence:

```text
cargo test -p mclone-app-runtime input_preferences: 5 passed
cargo check -p mclone-native-client --features xr: passed
pnpm native:android:apk: arm64 debug APK passed
pnpm native:android-xr:apk: arm64 release APK passed
```

The first Android XR cross-compile exposed and then closed a missing direct
shared-input dependency plus an incorrect startup-scope load. The rerun built
the real Android-only code and release package successfully. No host advertises
haptic output; real-device bindings, feel, and tactile acceptance remain in the
hardware-only ledger. Final automated cross-target closeout is still active.

## Completion Bar

- Every product host consumes one shared semantic action and UI-navigation
  contract.
- Desktop, browser, flat Android, and Android XR have real collector code behind
  truthful capability detection.
- Tracked XR controls retain poses and XR-only extensions while overlapping
  actions share the neutral semantic path.
- Every non-text screen is deterministically navigable by scripted controller
  input.
- Automated unit, conformance, build, purity, browser-mock, emulation, and
  rendered-output gates pass.
- Documentation distinguishes implemented/automated evidence from each open
  hardware acceptance item without overstating product readiness.
