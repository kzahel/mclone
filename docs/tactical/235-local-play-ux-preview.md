# Tactical 235: Local Play UX Preview

Status: completed 2026-07-24.

Topic: [`local-couch-multiplayer`](../topics/local-couch-multiplayer.md).

## Instruction Synthesis

Turn the completed controller, participant, layout, and auxiliary-view
foundations into an initial visible Local Play UX. Support the concrete
one-keyboard/mouse plus one-gamepad case: Player 1 keeps the existing
installation profile and ordinary gameplay path, while a clearly labeled
session guest claims one specific gamepad and drives an independent second
view.

Do not invent a full multi-profile system to unblock the prototype. The
additional seat is `Guest 2`, has no cross-launch identity promise, and must
not be presented as a persisted authoritative avatar. Keep the existing Debug
`Auxiliary View` feature separate. Use the same shared Rust UI, input, scene,
layout, and presenter contracts on every flat host; platform collectors remain
mechanical.

## Initial Product Decisions

1. Add a dedicated `Local Play` options category. It shows layout, Player 1's
   fixed `Keyboard + Mouse` assignment, Guest 2's live gamepad assignment,
   session-only profile scope, and view-only access.
2. Assignment is explicit capture, not a persistent `Gamepad (Auto)` rule.
   Activating the Guest 2 row captures the controller that confirmed it, or
   arms `Press A on a gamepad` when activated with pointer/keyboard.
3. One physical source belongs to at most one seat. The captured source is
   removed from Player 1's controller reducer immediately and remains assigned
   until Guest 2 is removed or the source disconnects.
4. Layout and participant assignment are separate. Changing `Left / Right` to
   `Top / Bottom` does not swap inputs or identities.
5. Guest 2 is a session-only presentation/input participant in this tactical.
   The view may move independently against already resident client facts, but
   owns no command stream, HUD/inventory, interaction authority, persistence,
   audio listener, or distant observer interest.
6. The UI must state that limitation rather than depicting Guest 2 as a saved
   ordinary player. A later authoritative slice will replace the preview
   boundary with a second ordinary client endpoint.
7. Menus remain full-surface and globally navigable. This tactical does not
   choose pane-local inventory or group pause semantics.

## Architecture Boundaries

- `mclone-ui` owns the neutral Local Play render state and actions.
- `mclone-app-runtime` owns session-local Local Play configuration policy.
- `mclone-scene` owns source capture/routing, participant camera state, menu
  projection, and selection of the existing shared split presenter.
- Desktop, browser, and flat Android collectors continue emitting source IDs,
  descriptors, and ordered canonical batches without participant policy.
- The Debug auxiliary mode remains available only when Local Play is inactive.
- No fixed pair enters the durable participant model; the first live consumer
  is intentionally capped at Guest 2 while the accepted 1-4 contracts remain
  unchanged underneath it.

## Slice Plan

### Slice 1 — shared UX state and menu

- Add a `Local Play` options category with layout, Player 1, Guest 2, profile
  scope, and honest preview-access rows.
- Add shared actions for layout, assignment capture, and removal.
- Keep the state session-local and unavailable on XR.
- Prove menu navigation, disabled/unavailable projection, and exact labels.

### Slice 2 — source capture and isolated routing

- Retain connected source descriptors in the shared interactive router.
- Capture the confirming source, or the next meaningful unassigned source
  after keyboard/pointer arming.
- Filter ordered controller batches so the assigned source no longer reaches
  Player 1.
- Drive a separate semantic controller session for Guest 2, suppress the join
  edge until neutral, and clear safely on disconnect/removal.

### Slice 3 — live independent guest view

- Add a scene-owned Guest 2 presentation state initialized near Player 1.
- Advance its camera from the isolated semantic action frame against the
  active resident client world without sending authoritative commands.
- Reuse the shared two-pane layout/GPU presenter with the ordinary primary HUD
  and a world-only Guest 2 view.
- Preserve the direct mono path and existing elevated Debug auxiliary camera.

### Slice 4 — rendered and platform validation

- Add shared unit/contract coverage for source capture, routing isolation,
  disconnect, layout changes, and mono restoration.
- Capture and inspect a native Local Play menu plus both split layouts.
- Exercise keyboard/mouse Player 1 and scripted/gamepad Guest 2 camera motion.
- Run affected native, Wasm, browser ownership, and flat-Android checks.

### Slice 5 — closeout

- Reconcile this tactical and the living couch topic with exact evidence.
- Record the required authoritative follow-up: multi-connection local session,
  second ordinary replica, per-pane HUD/interaction, durable profile choice,
  persistence, audio, and pause policy.

## Completion Bar

- The Local Play screen explicitly shows Player 1 as keyboard/mouse and Guest
  2 as unassigned, waiting, assigned to a named controller, or disconnected.
- A named controller can be claimed without its confirm edge affecting
  gameplay, and thereafter it cannot move Player 1.
- Keyboard/mouse moves Player 1 while the assigned gamepad independently moves
  Guest 2's view in live left/right and top/bottom presentation.
- Removing Guest 2 returns to mono and restores the gamepad to the unassigned
  pool without stuck input.
- The UI never claims Guest 2 has authoritative gameplay or persisted profile
  state.
- Existing Debug auxiliary view and mono behavior remain green.

## Completed Result

- Options now has a full-width `Local Play` setup sheet on every flat host.
  It exposes `Left / Right` and `Top / Bottom`, fixes Player 1 to
  `Keyboard + Mouse`, and shows Guest 2 as Off, waiting, assigned to a
  controller family plus session ordinal, or disconnected.
- Guest 2 is explicitly `Session Only` and `View Only`. The existing durable
  installation profile remains Player 1's identity; no second profile schema
  or persisted guest record was introduced.
- Activating Guest 2 with keyboard or pointer arms capture. Activating it with
  a gamepad captures that confirming source immediately. The captured source
  leaves Player 1's semantic reducer, drives a separately timed controller
  session, and returns to the primary pool after removal.
- The scene retains an optional Guest 2 presentation controller initialized
  near Player 1. It advances movement and look against resident collision
  facts without publishing commands or interest, then supplies the existing
  shared two-pane presenter. Disconnect preserves the pane and clears held
  state; removal restores the direct mono path.
- Full-screen menus remain global. The Debug auxiliary selector is unavailable
  while Guest 2 is armed or active, so the two split consumers cannot conflict.
- Desktop, browser, and flat Android rims retain only source collection and
  target ownership. Local Play policy lives in shared UI, app-runtime, and
  scene code.

## Validation Evidence

- Shared Rust tests:
  `cargo test -p mclone-ui -p mclone-app-runtime -p mclone-scene --lib`.
- Scene ownership lock:
  `cargo test -p mclone-scene --test one_world_ownership_contract`.
- Wasm build: `pnpm native:web:build`.
- Flat Android arm64 packaging: `pnpm native:android:apk`.
- Native Local Play setup capture inspected at
  `/tmp/mclone-local-play-menu.png`.
- Shared left/right and top/bottom presenter captures inspected at
  `/tmp/mclone-local-play-layouts/{horizontal,vertical}.png`; both reported one
  shared preparation, two rendered views, and differing pane pixels.
- `git diff --check` passes.

Three repository-wide baseline gates remain independently red:

- `pnpm native:thin-adapters:purity` rejects the pre-existing
  `WinitFrameDriver::reconcile_capture_camera_pose` diagnostic call, outside
  this tactical's one changed presenter-retention predicate.
- `cargo test --workspace --quiet` reaches 549/550 passing server library
  tests, then the unrelated
  `sqlite_restart_restores_mclone_profile_before_unseen_generation` test reads
  no stored profile version instead of version 2. Its exact isolated rerun
  fails identically; this tactical does not touch server persistence.
- The pre-existing WebSceneHost ABI inventory counts 38 mechanical exports
  while `platform_boundary_convergence_debt` still pins 37. This tactical's
  only `web_scene_host.rs` edit replaces one split-selection predicate and
  neither adds nor removes an export.

## Stop Conditions

Stop and ask for direction before:

- persisting additional profiles or guest player records;
- representing Guest 2 as an authoritative player without an ordinary
  connection/update stream;
- selecting shared replica/GPU-cache ownership for multiple live clients;
- allowing Guest 2 to mutate world, inventory, statistics, or life state;
- choosing pane-local versus global pause/inventory behavior; or
- selecting multi-listener audio policy.
