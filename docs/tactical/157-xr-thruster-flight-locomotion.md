# 157: XR Thruster Flight Locomotion (Iron Man mode)

Status: active 2026-07-08; Slice 1 (plumbing) landed, Slice 2 (integrator) next.
Hand-driven "Iron Man / repulsor" flight movement model, entered as a new shared
`Movement` value alongside `Player`, `Fly`, and `Gorilla`/`HandPush`.

Workstream: native Rust shared client experience, input, movement, and XR. This
is a shared engine locomotion technique with XR controller input first, **not** a
desktop-only or XR-only feature. It slots into the shared movement taxonomy from
[`147-shared-movement-experience-settings.md`](147-shared-movement-experience-settings.md)
and reuses the XR controller/pose plumbing from
[`109-xr-hand-push-locomotion.md`](109-xr-hand-push-locomotion.md).

## Goal

Add a **full-manual hand-thruster flight mode**: each hand is a repulsor/jet. The
analog trigger sets that hand's throttle; thrust pushes the body in the direction
opposite the palm normal (fire out of the palm → shoved the other way). Both
hands sum. Gravity still applies (with a tunable, per-flying-player scale — the
"suit antigravity field"), so holding altitude takes real thrust and the mode has
a skill curve. No hover assist, no stabilization, no FX in this first slice — the
point is to get it in the headset and feel it.

Reference feel: Marvel's Iron Man VR / HVR palm-thrust flight. Superman-style
"point where you go, ignore gravity" is intentionally **not** this mode — that is
nearly the existing `Fly + Normal` mode with a hand-vs-head steering source, and
is tracked as a small follow-up (see Out of scope), not a new movement model.

## Design decisions (locked with user)

- **New movement model, not a Fly variant.** Iron Man is its own `Movement`
  value (working engine name `Thruster`; user-facing label TBD — "Iron Man",
  "Thruster", or "Repulsor"). It is gravity-bound like `Gorilla`, unlike `Fly`.
- **Entering Thruster must NOT force `Collision = NoClip`.** Unlike `Fly`
  (which auto-enables NoClip today, `camera.rs` toggle path), Thruster defaults
  `Collision = Normal` — you can crash into terrain; that is part of the mode.
  NoClip stays independently selectable via the shared `Collision` axis.
- **Throttle = analog trigger, per hand, independent.** Left trigger drives the
  left thruster, right trigger the right thruster. Independent throttle is what
  gives differential banking/steering. One-handed flight (single nozzle) falls
  out for free. `trigger` is already on `XrControllerSnapshot`.
- **Thrust axis = palm normal, derived from the OpenXR grip pose.** Body
  acceleration per hand is `-palm_normal * trigger * THRUST_SCALE`, summed. Palm
  down → rise; palms back → go forward (authentic, if initially unintuitive).
  This requires an approximate palm-normal, which the current XR path does not
  surface (see below) — deriving it is a first-class part of this work.
- **Forces are tick-rate independent.** The integrator runs in continuous SI
  units (blocks/s², blocks/s) against real frame `dt`, NOT per-tick constants.
  Tick rate is configurable per [`116-simulation-cadence-and-stepping.md`](116-simulation-cadence-and-stepping.md);
  thrust/gravity/drag feel must not change when the host/gameplay/physics rate
  changes.
- **Tunable gravity, drag, and max speed.** All exposed as constants first, wired
  to dev-adjustable knobs after the feel is roughed in. Gravity is a
  per-flying-player scale (client-local "antigravity field"), not a world change.
- **Client-predicted, like Fly.** No new server authority in this slice; the
  client integrates locally and emits the existing `MovePlayerCommand::Pos`.
  Permissive server validation ([`056`](056-native-player-movement-networking.md))
  already tolerates fast client movement; note as a watch-item, do not build
  anti-cheat here.

## Current code map (what exists, what's missing)

Movement modes / dispatch:

- `EngineCameraMovementMode { Walking, Fly, HandPush }` —
  `native/crates/mclone-render-session/src/camera.rs:148`; `.toggled()`,
  `.label()`, `set_movement_mode`, `toggle_movement_mode` around
  `camera.rs:495-545` (Fly auto-enables NoClip here — Thruster must not).
- Independent `EngineCameraCollisionMode { Normal, NoClip }` — `camera.rs:174`.
- Game-facing `GameMovementMode { Walk, Fly, HandPush }` —
  `native/crates/mclone-ui/src/lib.rs:1111`; converters at
  `native/crates/mclone-xr-scene/src/locomotion.rs:268-296` and
  `native/apps/mclone-native-client/src/flat_client_driver.rs:2909-2923`.
- Shared movement-experience settings/reducer/capability profile —
  `native/crates/mclone-app-runtime/src/client_experience.rs:955-1009`
  (`apply_movement_experience_change` / `normalize_movement_experience`); shared
  axes landed in tactical 147.
- Per-frame dispatch `apply_movement_input` — `camera.rs:734-760`
  (`Fly` → `tick_flying`, `HandPush` → `tick_hand_push`). Thruster adds an arm.

Movement integration (client, predicted):

- `tick_flying_movement_with_impulse` — `mclone-client/src/player.rs:1060`;
  `flying_displacement` (kinematic, **no gravity**) — `player.rs:1836`.
- `HandPushLocomotionController` (gravity-bound: runs a walking tick after the
  hand logic) — `player.rs:264-460`. Closest structural analog.
- Gravity constants: `LOCAL_PLAYER_GRAVITY = 0.08` blocks/tick² (`player.rs:27`),
  `LOCAL_PLAYER_VERTICAL_DRAG = 0.98`/tick (`player.rs:35`),
  `LOCAL_PLAYER_TICKS_PER_SECOND = 20` (`player.rs:20`). These are **per-tick**
  and MUST NOT be used verbatim for thruster forces.
- SI-unit gravity already exists server-side: `SERVER_PHYSICS_GRAVITY = -32.0`
  m/s² — `native/crates/mclone-server/src/physics_runtime.rs:27-34`. Thruster
  should integrate in this SI form (32.0 blocks/s²), scaled by `dt`, so it is
  cadence-independent by construction.

XR input (controllers, not hand tracking — Quest 3 Touch Plus is the target):

- `XrControllerSnapshot { hand, aim_position, aim_direction, grip_position,
  trigger, squeeze, ... }` — `native/crates/mclone-xr-host/src/actions.rs:5-25`.
  Analog `trigger` is present; per-hand throttle is free.
- **Gotcha — palm orientation is dropped.** `locate_pose` (`actions.rs:471-501`)
  reads the pose quaternion but reduces it to a single forward vector
  (`orientation * -Z`) stored as `aim_direction`; the full quaternion is not
  surfaced. `XrActionPose` keeps only `position` + `forward` (`actions.rs:528`).
  The grip-pose action space is already bound and located; we just need its
  **orientation quaternion** surfaced to compute a palm normal.
- Hand→engine bridge: `EngineHandPushInput` (head + both hand world positions) —
  `camera.rs:67-94`; built by `xr_hand_push_input_from_controllers`
  (`mclone-xr-scene/src/locomotion.rs:195-228`), wired at `locomotion.rs:330`.
  Thruster needs an analogous input carrying, per hand, **orientation/palm dir +
  trigger** (positions optional this slice).

## Palm-direction approximation (Quest 3)

The OpenXR **grip pose** is defined palm-relative (origin at the palm centroid,
axes fixed to the hand), unlike the **aim pose** (a pointing ray). So the palm
normal must come from the grip-pose orientation, not `aim_direction`. The exact
grip-local axis that maps to the palm normal is interaction-profile dependent, so
**calibrate it empirically for `oculus/touch_controller` rather than hard-coding a
guessed axis**:

1. Surface the grip-pose quaternion (see Slice 1).
2. On-device, hold the controller with the palm flat and facing straight down;
   log the grip orientation. The grip-local axis whose world image is ≈ `(0,-1,0)`
   in that pose is the palm normal; record axis + sign as `TOUCH_PALM_AXIS`.
3. Sanity-check with palm-forward and palm-inward poses (world image should track
   the physical palm normal).
4. Bake `TOUCH_PALM_AXIS` as a named constant with a comment citing the
   calibration pose, so a future Index/Vive/WMF profile can add its own without
   guesswork. Body accel uses `-palm_normal` (reaction to the jet).

Desktop-XR / no-headset emulation (from 109's pattern) should provide a
synthetic palm normal so the integrator can be tuned without donning the headset
every iteration.

## Slices

### Slice 1 — Surface grip orientation + Thruster movement value (plumbing)

Status: landed 2026-07-08.

- [x] `mclone-xr-host`: `XrControllerSnapshot` now carries `grip_orientation:
  Option<Quat>`; `XrActionPose` retains the grip-pose orientation quaternion and
  `locate_pose` no longer drops it.
- [x] `mclone-render-session`: added `EngineCameraMovementMode::Thruster`
  (`.toggled()` cycle Walk→Fly→HandPush→Thruster→Walk, label `THRUST`, STANDING
  dimensions). Entering Thruster defaults `Collision = Normal` (shares HandPush's
  one-time entry default) but is deliberately left OUT of the per-frame collision
  force, so NoClip stays selectable; it also does not hit Fly's NoClip
  auto-enable.
- [x] `mclone-ui` + `mclone-app-runtime::client_experience`: added the
  `GameMovementMode::Thruster` value (label "Iron Man"), and reducer rules —
  entry forces `Travel Assist = Off` (like Fly) and defaults `Collision = Normal`
  (like HandPush), while `normalize_movement_experience` does NOT force Thruster
  collision (NoClip stays selectable). `legacy_collision_mode_for_movement`
  maps Thruster → Normal.
- [x] All four `game_movement_mode` / `engine_movement_mode` converter pairs
  updated (xr-scene, flat native, android, web).
- [x] Added `EngineThrusterInput` / `EngineThrusterHand` (per-hand world palm
  normal + analog throttle) and `EngineCameraInput.thruster`; the camera retains
  it as `last_thruster_input()` for the Slice 2 integrator/client.
- [x] Added `xr_thruster_input_from_controllers` beside
  `xr_hand_push_input_from_controllers`, wired into `apply_locomotion_input`. It
  derives each hand's world palm normal from the grip-pose quaternion via a
  **provisional, uncalibrated** grip-local palm axis (`grip_local_palm_axis`,
  +X left / −X right) — Slice 3 replaces this with an on-device calibrated
  constant.
- [x] The `Thruster` movement dispatch arm is inert (no displacement) but
  retains the per-hand intent.
- [x] Unit tests: engine toggle-cycle/labels include Thruster; Thruster defaults
  Normal collision but leaves NoClip selectable (contrasted with HandPush);
  inert-but-retains-input; reducer forces Travel Assist Off + defaults Normal and
  keeps NoClip selectable.

Deferred from Slice 1 (intentional): no per-profile capability gating was added —
Thruster is in the shared mode cycle everywhere, matching HandPush, which is also
ungated. XR-first capability projection can be added later alongside flat
emulation (Slice 3) if desired; it is not required while the mode is inert.

Exit (met): Thruster is selectable in the mode cycle, defaults to Normal
collision, keeps NoClip selectable, and grip orientation + per-hand trigger are
plumbed to the client via `last_thruster_input`; no movement yet.

### Slice 2 — Core thruster integrator (full manual, tick-independent)

- `mclone-client/player.rs`: add `tick_thruster_movement` beside
  `tick_flying_movement_with_impulse`. Integrate in **SI units against real dt**:
  - `accel = sum_hands(-palm_normal * trigger * THRUST_SCALE)`
  - `accel.y -= GRAVITY_SI * GRAVITY_SCALE`  (`GRAVITY_SI = 32.0` blocks/s²)
  - `velocity += accel * dt`
  - `velocity *= DRAG.powf(dt)`  (continuous exponential drag, cadence-independent)
  - clamp `velocity` to `MAX_SPEED`
  - `displacement = velocity * dt`; resolve via `move_colliding` (Normal) so you
    crash into terrain; carry velocity frame-to-frame (unlike Fly, which zeroes
    `delta_movement`).
- `mclone-render-session`: `tick_thruster` dispatch arm in `apply_movement_input`
  (`camera.rs:734-760`) mirroring `tick_hand_push`, feeding `EngineThrusterInput`.
- Constants first: `THRUST_SCALE`, `DRAG`, `MAX_SPEED`, `GRAVITY_SCALE`, tuned so
  ~60-70% throttle on both hands hovers and full throttle climbs convincingly.
  `GRAVITY_SCALE` starts adjustable (expect to test well below 1.0 — full MC 3×
  gravity is brutal in full manual; the "antigravity field" default may land
  around 0.4-0.6, decided during headset tuning).
- Headset validation on Quest 3 standalone Android XR.

Exit: you can fly around in the headset with per-hand thrusters against gravity;
feel is tunable via constants; no NoClip forced; terrain collides.

### Slice 3 — Palm calibration + dev tuning knobs

- Calibrate `TOUCH_PALM_AXIS` on-device per the procedure above; replace any
  Slice 2 placeholder axis.
- Desktop-XR no-headset emulation for the thrust input so tuning does not require
  the headset each pass.
- Promote `THRUST_SCALE` / `DRAG` / `MAX_SPEED` / `GRAVITY_SCALE` to
  dev-adjustable values (shared settings/debug surface, not a full options UI
  yet) so the feel can be dialed in live.

Exit: palm direction is correct and calibrated on Quest 3; the four feel knobs
are adjustable without a rebuild.

### Slice 4 — Feedback (FX / sound / haptics) — follow-up, own sub-slices

Deferred; owners noted so they are not built app-local:

- **Haptics**: OpenXR haptic output action (new plumbing in `mclone-xr-host`) —
  low continuous rumble scaling with total thrust + a spike on thrust-start.
- **Sound**: procedural jet whoosh in `mclone-audio` (shared owner) — filtered
  noise with volume/pitch tracking total thrust magnitude.
- **Thrust visualization**: palm jet/glow. Subject to the CLAUDE.md **XR
  render-path guardrail** — must render in both per-eye and full-frame multiview
  paths (or document why not); route through an existing multiview-aware renderer,
  do not add a per-eye-only quad.

### Slice 5 — Comfort / assist modes (later)

- Optional hover-snap (bleed velocity toward zero when both triggers past a
  threshold and palms ≈ down) and/or a stabilization assist, behind a setting.
- Superman steering-source toggle on `Fly + Normal` (hands vs head vs stick),
  gravity off — cheap, not a new movement model. Flapping/rowing propulsion is a
  separate later experiment.

## Ownership (shared-first)

- `mclone-xr-host`: grip-pose orientation surfacing, later haptic output action.
- `mclone-render-session`: `Thruster` movement value, dispatch arm, no-NoClip
  entry rule.
- `mclone-client`: thruster integrator, SI-unit physics, collision resolve.
- `mclone-xr-scene`: `EngineThrusterInput` builder from controller snapshots
  (per-hand palm normal + trigger), plus desktop-XR emulation.
- `mclone-ui` + `mclone-app-runtime::client_experience`: shared `Movement =
  Thruster` value, labels, reducer rules, capability projection.
- `mclone-audio`: procedural thruster sound (Slice 4).
- App/platform crates: only pose/trigger collection and surface/session glue.

## Validation

Primary is **manual headset feel on Quest 3 standalone Android XR** (the user's
main test target); desktop XR is the fast iteration path via emulation.

Build/test gates (mirror 147 Slice F):

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test  --manifest-path native/Cargo.toml \
  -p mclone-ui -p mclone-app-runtime -p mclone-render-session \
  -p mclone-client -p mclone-xr-scene -p mclone-xr-host
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
git diff --check
```

Focused tests:

- Enum cycle/labels include `Thruster`; entering `Thruster` yields
  `Collision = Normal` (never NoClip); reducer forces `Travel Assist = Off`.
- **Tick independence:** integrating the same thrust/gravity/drag over a fixed
  wall-clock interval produces the same displacement at two different configured
  tick rates (e.g. 20 Hz vs 60 Hz gameplay), within tolerance. This is the
  regression guard for the cadence-independence requirement.
- Max-speed clamp holds; drag makes velocity decay to a terminal speed with zero
  throttle; per-hand throttle produces asymmetric (banking) accel.

Headset checks (Slice 2+): thrusters respond per hand; palms-down hovers/climbs;
palms-back moves forward; terrain collides (no accidental NoClip); no nausea
blocker at the chosen `GRAVITY_SCALE`.

Per native-validation policy: look at any capture before moving on; rerun failed
GPU/headless captures with elevation before treating GPU validation as blocked.

## Acceptance criteria

- `Thruster` is a shared `Movement` value; selecting it keeps `Collision =
  Normal` by default and does not enable NoClip.
- Per-hand analog trigger drives per-hand thrust along `-palm_normal` (grip-pose
  derived, calibrated for Quest 3 Touch Plus); both hands sum.
- Flight physics integrate in SI units against real `dt`; identical feel across
  configured tick rates (test-verified).
- Gravity (scaled), drag, and max speed are tunable; full-manual mode (no hover
  assist) is playable in the headset.
- Movement stays client-predicted; existing desktop/web/Android/XR crates compile
  as consumers with no forked movement semantics.

## Related docs

- [`147-shared-movement-experience-settings.md`](147-shared-movement-experience-settings.md) — shared Movement/Collision/Travel Assist/Turn axes; Thruster is a new Movement value here.
- [`109-xr-hand-push-locomotion.md`](109-xr-hand-push-locomotion.md) — XR grip/head pose input, desktop emulation, shared client movement technique pattern.
- [`116-simulation-cadence-and-stepping.md`](116-simulation-cadence-and-stepping.md) — configurable cadence lanes; source of the tick-independence requirement.
- [`056-native-player-movement-networking.md`](056-native-player-movement-networking.md) — permissive client-predicted movement / server validation direction.
- [`082-desktop-xr-player-locomotion.md`](082-desktop-xr-player-locomotion.md), [`083-android-xr-quest-standalone.md`](083-android-xr-quest-standalone.md) — XR input → shared movement wiring and Quest validation path.
- CLAUDE.md "XR render-path guardrail" (Slice 4 FX), "Shared-first feature policy".

## Out of scope

- Superman/point-to-fly (a steering-source toggle on `Fly + Normal`, not a new
  model), flapping/rowing propulsion — later experiments.
- Hover assist / auto-stabilization (Slice 5), any full options-UI polish for the
  tuning knobs.
- FX/sound/haptics beyond the Slice 4 stubs.
- Server-authoritative flight or anti-cheat for fast movement.
- Grappling/web-swing locomotion (poor fit for MC terrain; not planned).
