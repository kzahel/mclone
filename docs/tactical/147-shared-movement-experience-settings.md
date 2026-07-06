# 147: Shared Movement Experience Settings

Status: proposed parent; opened 2026-07-06.

Workstream: native Rust shared client experience, input, movement, and UI. This
is not an XR-only menu plan. The target is one shared movement/settings model
that desktop flat, offscreen, web, flat Android, desktop XR, and Android XR can
project through their own input adapters and capability masks.

## Purpose

The current movement UI and XR input path mix several different ideas into a
small set of labels:

- `Fly` currently means no-clip free movement, not just flying.
- Blink teleport currently reserves the XR left stick as an initial playable
  comfort path, so continuous stick movement is effectively unavailable in XR.
- `XR Turn` is really a stick/gamepad turn policy, not inherently XR-only.
- Gorilla / hand-push locomotion is shared engine behavior with XR controller
  input first, but desktop emulation should remain a shared-profile capability,
  not a separate flat-only experiment.

This tactical separates the concepts into shared axes, then lets each profile
hide, disable, or auto-correct unsupported combinations. It should not create an
XR submenu or a separate XR UI system.

## Current Problem

Relevant current behavior:

- `mclone-ui::GameMovementMode` exposes `Walk / Fly / Hand Push`.
- The options panel exposes `Movement` and, when available, `XR Turn`.
- `mclone-xr-scene` maps `Fly` to
  `EngineCameraMovementMode::NoClip`.
- XR Blink teleport arms from left-stick magnitude above `0.5` and suppresses
  left-stick movement while active. It also suppresses normal left-stick
  movement whenever the stick is merely outside the dead zone, even before
  Blink engages.
- Tactical `125` intentionally called this left-stick reservation the initial
  playable Blink path and deferred Continuous / Blink / Shift / Hand Push
  options to later UI work.

The result is that normal continuous XR movement is hard to recover from the
menu, especially in fly/no-clip mode, even though the engine still has continuous
movement internally.

## Target Shared Axes

Use shared settings names that are not XR-specific by default:

| Setting | First values | Meaning |
|---|---|---|
| `Movement` | `Player`, `Fly`, `Gorilla` | Body/game locomotion model. |
| `Collision` | `Normal`, `NoClip` | Whether movement collides with world geometry. |
| `Travel Assist` | `Off`, `Blink`, `Warp` | Optional target-based relocation assist. |
| `Turn` | `Snap 15`, `Snap 30`, `Smooth` | Stick/gamepad artificial turn policy. |

Notes:

- `Fly + Normal` is creative-style flight: no gravity, continuous fly movement,
  but blocked by collision.
- `Fly + NoClip` is debug/spectator-style free movement.
- `Travel Assist` is shared. XR feeds controller aim; flat desktop can feed a
  camera/mouse debug or product input adapter; offscreen tests can feed scripted
  intent.
- `Turn` is shared. XR and gamepad profiles can expose it; mouse/keyboard-only
  flat profiles can hide it.
- `Gorilla` means the shared hand-push movement model. XR feeds real hand/head
  poses; flat desktop may expose emulated hand-push only when the emulation path
  is usable enough.

## Compatibility Rules

The shared settings reducer should own compatibility corrections. UI rows can
show the corrected state, disabled state, or a capability projection, but the
same rules must apply to keyboard/controller/menu shortcuts and tests.

Initial rules:

- Switching `Movement` to `Fly` sets `Travel Assist = Off`.
- Switching `Collision` to `NoClip` sets `Travel Assist = Off`.
- Switching `Travel Assist` to `Blink` or `Warp` sets
  `Collision = Normal`.
- `Travel Assist = Blink/Warp` is invalid with `Movement = Fly`.
- `Movement = Gorilla` defaults `Collision = Normal`.
- `Movement = Gorilla + Collision = NoClip` is disabled initially unless a real
  shared debug use case is implemented.
- `Warp` may be exposed as `Pending` or hidden until tactical `125` Slice 5 is
  implemented. `Blink` can use the current shared teleport preview/commit path.
- Profiles without a stick/gamepad/artificial-turn input source may hide
  `Turn`, but the setting remains shared.

Avoid surprising reverse corrections where possible. For example, selecting
`Blink` should not silently change `Fly` back to `Player`; instead, either keep
Blink disabled while Fly is active or apply a visible reducer rule in the shared
settings state if the product chooses to make that transition explicit.

## Ownership

Shared crates own policy and behavior:

- `mclone-ui`: shared setting enums, labels, actions, and one common options
  surface. No XR submenu in this slice.
- `mclone-app-runtime::client_experience`: shared settings state, capability
  profile, compatibility reducer, and action effects.
- `mclone-render-session`: engine camera/player movement taxonomy. Split
  movement model from collision/no-clip policy instead of treating no-clip as
  the Fly movement mode.
- `mclone-client`: shared player movement, hand-push locomotion, collision
  helpers, teleport preview/commit contracts, and any new colliding-fly helper.
- `mclone-input`: shared input intents and action names where keyboard/gamepad
  shortcuts need to target these settings.

Profile/platform adapters own only input collection and projection:

- Desktop flat maps keyboard/mouse/gamepad/debug input into the shared movement,
  turn, and travel-assist intents.
- Desktop XR and Android XR map OpenXR controller snapshots into the same shared
  movement, turn, and travel-assist intents.
- Web, flat Android, and offscreen expose only capabilities that have a usable
  adapter, but must not fork setting semantics.

## Feature Additions

### Collision Axis

Add a shared user-facing `Collision` setting:

- `Normal`: collision-backed movement.
- `NoClip`: ignore world collision.

This requires splitting the current engine camera mode shape. Today
`EngineCameraMovementMode::NoClip` is both a movement type and a collision
policy. The target shape should distinguish:

- movement model: player walking, flying, hand-push/Gorilla
- collision policy: normal collision or no-clip
- gravity/vertical policy: walking gravity, flying no-gravity, hand-push
  physics

First implementation can be conservative:

- Keep existing walking and hand-push behavior unchanged.
- Preserve current no-clip fly as `Movement = Fly, Collision = NoClip`.
- Add `Movement = Fly, Collision = Normal` as a colliding fly mode that reuses
  shared collision movement and disables gravity while still respecting world
  blocking.
- Do not expose `Player + NoClip` or `Gorilla + NoClip` broadly until there is
  a clear debug/product reason.

### Travel Assist Axis

Add a shared `Travel Assist` setting:

- `Off`: continuous movement input is not reserved by teleport.
- `Blink`: current validated teleport preview plus immediate/faded relocation.
- `Warp`: path-validated fast relocation, initially pending until tactical
  `125` Slice 5 lands.

The immediate XR regression fix should be a consequence of this axis:

- When `Travel Assist = Off`, XR left-stick input must pass through to
  continuous movement.
- Blink preview/commit may suppress left-stick movement only when
  `Travel Assist = Blink` and Blink is active or arming.
- Future Warp uses the same target preview and compatibility rules as Blink.

### Turn Axis

Rename the shared UI concept away from `XR Turn`:

- Preferred label: `Turn`.
- More explicit option if needed: `Stick Turn`.

The underlying policy can keep using the current snap/smooth implementation.
Profiles decide whether to show it. XR should show it; flat desktop should show
it only for gamepad/emulated profiles where it controls an actual artificial
turn path.

### Gorilla / Hand-Push

Keep Gorilla as shared `Movement = Gorilla`.

Initial capability stance:

- XR profiles can expose Gorilla because they have real head/hand poses.
- Flat desktop can expose Gorilla only if the existing emulated hand-push path
  is usable enough for player-facing controls. Otherwise mark it disabled or
  pending in the profile.
- The shared movement controller remains in `mclone-client` /
  `mclone-render-session`; flat and XR adapters only provide pose/input facts.

## UI Direction

Keep one options/menu system.

Short-term rows:

- `Movement`
- `Collision`
- `Travel Assist`
- `Turn`

Rows may be hidden when the current profile has no meaningful adapter, and rows
may be disabled when the current state makes the option invalid. Do not create a
separate XR submenu until there are enough platform-specific comfort controls
to justify one. Even then, the setting state should remain shared.

The shared UI should avoid labels that imply platform ownership unless the row
is truly platform-specific. `XR Turn` should become `Turn`; `Hand Push` may
become `Gorilla` if that is the product-facing name.

## Slice A1: Shared Taxonomy And Reducer Foundation

Status: landed 2026-07-06.

- [x] Add shared UI enums for `Collision`, `Travel Assist`, and the renamed
  shared `Turn` concept.
- [x] Extend `ClientExperienceSettingsState` with the new shared movement
  experience settings.
- [x] Extend `ClientExperienceSettingsProfile` with capability flags for
  collision, travel assist, and turn so later profile projection can stay
  shared.
- [x] Add an action-aware shared compatibility reducer that applies the rules
  from this doc without putting policy in flat or XR adapters.
- [x] Add focused unit tests for the enum cycles/labels and the shared reducer
  rules.
- [x] Keep the legacy `XR Turn` render-state slot synchronized from shared
  `Turn` so implementation can be sliced without a full UI rewrite.

Landed behavior:

- Selecting `Fly` clears `Travel Assist`.
- Selecting `NoClip` clears `Travel Assist`.
- Selecting `Blink` or `Warp` from a normal player movement state forces
  `Collision = Normal`.
- `Fly` rejects `Blink` / `Warp`.
- `Gorilla` / hand-push stays collision-backed.
- Shared `Turn` mirrors the current legacy XR turn slot.

## Slice A2: UI Actions And Render Projection

- [ ] Add shared UI actions/effects for `Collision`, `Travel Assist`, and
  shared `Turn`.
- [ ] Extend `GameUiRenderState` and the options layout with the new rows,
  using profile/state capability projection for hidden or disabled rows.
- [ ] Update `ClientExperienceSettingsController::apply_ui_action` to call the
  shared reducer for menu actions.
- [ ] Keep old labels/effects shimmed where needed so existing flat and XR
  settings still render.
- [ ] Add unit tests that menu actions, shortcuts, and direct reducer calls all
  produce the same corrected setting state.

Exit criteria:

- Existing flat and XR settings still render.
- Invalid combinations are corrected or rejected by shared policy, not by XR
  frame code.
- No platform adapter owns compatibility rules.

## Slice B: Split Fly From No-Clip

- [ ] Refactor `mclone-render-session` so movement model and collision policy
  are distinct state.
- [ ] Preserve current no-clip behavior as `Fly + NoClip`.
- [ ] Implement first colliding fly behavior as `Fly + Normal`.
- [ ] Ensure `Player + Normal` and `Gorilla + Normal` preserve current
  walking/hand-push behavior.
- [ ] Add focused tests for fly-with-collision stopping at blocks, fly-no-clip
  passing through blocks, and unchanged walking/hand-push behavior.
- [ ] Update debug labels/HUD/menu state to report both movement and collision
  when useful.

Exit criteria:

- `Fly` no longer inherently means no-clip.
- The old no-clip developer workflow still exists.
- Colliding fly is shared and does not live in a flat-only or XR-only adapter.

## Slice C: Shared Travel Assist Setting

- [ ] Add `Travel Assist = Off/Blink/Warp` to shared settings and UI.
- [ ] Route current desktop Blink debug adapter through this setting instead of
  making it an always-debug-only path.
- [ ] Gate XR Blink update/suppression on `Travel Assist = Blink`.
- [ ] Make `Travel Assist = Off` restore continuous XR left-stick movement.
- [ ] Mark `Warp` pending or disabled until its path execution is implemented.
- [ ] Add tests for `Fly -> Travel Assist Off`, `NoClip -> Travel Assist Off`,
  `Blink/Warp -> Collision Normal`, and the XR left-stick pass-through case.

Exit criteria:

- Continuous movement is selectable again in XR.
- Blink remains shared and still works through the existing teleport evaluator.
- Travel assist is not XR-only; non-XR profiles can expose it when they have an
  input adapter.

## Slice D: Turn Setting Rename And Profile Projection

- [ ] Rename UI-facing `XR Turn` to `Turn` or `Stick Turn`.
- [ ] Keep existing snap/smooth behavior for XR.
- [ ] Decide whether desktop gamepad/emulated profiles expose `Turn` now or
  mark it pending.
- [ ] Ensure mouse/keyboard-only flat desktop can hide the row without removing
  the shared setting from state.
- [ ] Add tests for profile-visible and profile-hidden turn rows.

Exit criteria:

- No user-facing setting name implies this is an XR-only concept.
- XR turn behavior is unchanged except for labels/profile projection.

## Slice E: Gorilla Capability And Flat Emulation Decision

- [ ] Audit the current flat hand-push emulation path.
- [ ] If it is usable, expose `Movement = Gorilla` for flat desktop behind the
  same shared movement mode.
- [ ] If it is not usable, mark flat Gorilla as disabled/pending through profile
  capabilities while preserving the shared enum/state.
- [ ] Keep XR Gorilla backed by real controller/head poses.
- [ ] Add tests that unsupported Gorilla selection is rejected or projected by
  shared capabilities, not by platform-specific menu code.

Exit criteria:

- Gorilla remains a shared movement model.
- Flat desktop and XR differ only by input adapter capability, not by separate
  movement implementation.

## Slice F: Validation And Cleanup

- [ ] Update tactical `109` and `125` references if names or states move.
- [ ] Add focused UI v2 layout/action tests for the new rows and disabled/hidden
  states.
- [ ] Add focused `mclone-app-runtime` tests for compatibility corrections.
- [ ] Add `mclone-render-session` and `mclone-client` movement tests for
  colliding fly and no-clip preservation.
- [ ] Run desktop flat/offscreen smoke for player/fly/no-clip settings.
- [ ] Run desktop XR or Android XR headset validation for:
  - continuous movement restored with `Travel Assist = Off`,
  - Blink only reserves the stick when selected,
  - Fly forces travel assist off,
  - colliding fly feels coherent if exposed in XR.

Suggested automated gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-app-runtime -p mclone-render-session -p mclone-client -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:desktop-offscreen:smoke
git diff --check
```

## Out Of Scope

- A separate XR options menu or XR-only settings model.
- Full controls remapping UI.
- Making Warp/Shift complete if tactical `125` Slice 5 has not landed.
- Multiplayer anti-cheat policy for teleport/no-clip.
- Java vanilla parity changes. These are client experience and movement-control
  policies, not vanilla worldgen/simulation ports.
