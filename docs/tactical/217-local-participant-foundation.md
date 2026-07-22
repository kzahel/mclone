# Tactical 217: Local Participant Foundation

Status: active 2026-07-22.

Topic: [`local-couch-multiplayer`](../topics/local-couch-multiplayer.md).

## Instruction Synthesis

Use the completed controller and preliminary multi-presentation foundations to
advance couch readiness without waiting for physical controller hardware.
Implement the remaining judgment-free shared seams end to end: introduce a
bounded session-local participant group, integrate scripted source assignment
with one semantic controller session per participant, prove two ordinary
logical clients against one integrated realm, and add a shared flat-surface
layout contract with an inspected scripted split-presentation capture.

Commit incrementally. Stop rather than choose durable guest/profile policy,
default product layout, group pause behavior, audio-listener policy, helper
authority, or a client-replica/GPU-cache sharing architecture that is not
already accepted by the couch topic. Physical controller feel and device
acceptance remain outside this tactical.

## Starting Evidence

- Tactical 215 left neutral 1-4 presentation-view identity, prepare-once /
  render-many flat frames, an inspected auxiliary-view capture, 1/2/4 realm
  interest tests, and a scripted four-seat source-assignment reducer.
- Tactical 216 completed semantic per-source controller state, shared menu and
  gameplay routing, desktop/browser/Android collectors, XR convergence,
  preferences, and automated validation. Its real-device ledger remains open.
- `LocalGamepadAssignmentReducer` is not consumed outside `mclone-input` tests.
  Product hosts currently merge all ordinary controllers into one active-source
  `ControllerInputSession` for one player.
- `DrawableWorldSlot` owns one camera, interaction controller, and player model;
  flat UI and host input routing are also singleton.
- `RealmServer` already owns multiple ordinary players and separate ordered
  publication streams. `LocalRealmSession` and the threaded integrated runner
  deliberately wrap one local connection.
- The multi-flat renderer can draw independent full textures in one prepared
  frame. It does not yet own a presentation-surface rectangle/layout contract.

## Locked Boundaries

1. `LocalParticipantId`, input source identity, participant slot, durable
   profile identity, authoritative player ID, and presentation view identity
   remain distinct types.
2. The bounded participant cardinality is 1-4; no fixed pair becomes the data
   model even when two-player layouts are the first visual proof.
3. One assigned source feeds one participant-local semantic controller session.
   Held, pressed, released, context, activity, and prompt origin never leak
   across participants.
4. The join edge is assignment/session policy, not a platform event. The
   scripted foundation consumes the join press and begins participant gameplay
   after a neutral sample; final product join UX remains open.
5. Two local logical clients are ordinary `ClientCommand`/`ServerUpdate`
   endpoints backed by distinct `SingleViewRuntime` replicas and one
   `RealmServer`. Global simulation advances once and each ordered player stream
   drains independently.
6. Synthetic test identities are explicit fixtures, not a guest persistence
   decision. No installation profile is reused for participant two.
7. Layout is shared validated data. This tactical supplies both horizontal and
   vertical two-pane plans plus arbitrary validated 1-4 rectangles; it does not
   select a product default.
8. The rendered proof uses one scene host, one prepared resident world, and
   independent participant camera/input presentation state. It does not clone
   `McloneSceneHost` or imply that an auxiliary camera is authoritative.
9. A true two-client rendered product path must not merge owner-private replicas
   or choose shared cache ownership implicitly. If it cannot use an already
   accepted boundary, stop and record the required decision.
10. No shipping host enables couch joining or a second participant in this
    tactical. Existing mono, XR, and physical-controller behavior remain
    unchanged.

## Slice Plan

### Slice 0 — plan and source locks

- Add this tactical, reconcile the couch topic with Tactical 216, and capture
  current singleton and assignment seams.
- Add source-contract tests keeping participant policy out of app collectors,
  participant IDs distinct from source/profile/player/view IDs, and scene
  participant state shared rather than desktop-local.

### Slice 1 — bounded participant and cardinality-one migration

- Add session-local `LocalParticipantId` allocation and a bounded
  `LocalParticipantGroup<T>` in `mclone-app-runtime`.
- Prove stable IDs, explicit slots, 1-4 admission, removal without reindexing,
  no duplicate slot/ID, and rejection beyond four.
- Introduce one scene-owned participant presentation envelope and move the
  current camera, interaction controller, and player-model state into it while
  retaining exactly one participant and unchanged public behavior.
- Keep installation profile, world slot, renderer resources, and global UI out
  of the participant envelope until their ownership is deliberately migrated.

### Slice 2 — source assignment to participant-local semantics

- Add a shared local participant input group that owns the existing assignment
  reducer and one `ControllerInputSession` per admitted participant.
- Connect/disconnect/sample canonical snapshots once, route assigned samples
  only to their participant, preserve reconnect reservations, and report join,
  action, and participant facts explicitly.
- Prove two and four scripted sources with reordered samples, independent
  contexts, no held/edge leakage, disconnect clearing, reconnect preservation,
  and a full-group result.
- Do not alter the current one-player product collectors or their active-source
  arbitration.

### Slice 3 — two ordinary local logical clients

- Add a deterministic, host-neutral local client-group harness in
  `mclone-app-runtime`: one `RealmServer`, 1-4 participant-to-player mappings,
  and one `SingleViewRuntime` per ordinary endpoint.
- Use explicit synthetic test identities. Route commands through
  `RealmServer::try_handle_command_for_player`, route global scheduler and
  simulation work once, and drain each player stream independently.
- Prove two participants receive configuration/world state, see one another,
  keep owner-only selected-item/life/statistic state isolated, expand and
  contract separated interest, and allow one participant to leave without
  tearing down the other.
- Add a native temporary-store reopen proof only if it uses existing player
  persistence semantics without deciding product guest durability.

### Slice 4 — shared flat-surface layout and scripted pixels

- Add a target-neutral flat-surface layout contract with pixel rectangles,
  safe-area validation, non-overlap checks, arbitrary 1-4 panes, and named
  horizontal/vertical two-pane constructors.
- Generalize the offscreen compositor to consume that contract while rendering
  each world pane to its own aspect-correct color/depth target.
- Drive two participant presentation states with distinct scripted controller
  actions/cameras. Render both in one prepared flat frame and save horizontal
  and vertical composites plus a mono control under `/tmp`.
- Inspect the captures and record view-local culling, shared preparation count,
  pane bounds, and pixel difference. Do not claim a true two-client rendered
  path unless Slice 3 replicas are connected without violating Locked Boundary
  9.

### Slice 5 — closeout

- Run formatting, focused native suites, source locks, affected Wasm checks,
  and the smallest rendered-output commands.
- Reconcile this tactical, the couch topic, tactical index, and platform matrix
  if any supported capability changed.
- Record exact remaining hardware acceptance and the next product/architecture
  decision; leave the worktree's unrelated Asset Lab changes untouched.

## Required Gates

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-input
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime \
  --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml -p mclone-scene \
  --target wasm32-unknown-unknown
```

Generated screenshots remain outside the repository and must be inspected at
the first drawable milestone.

## Completion Bar

- Participant identity/group contracts admit 1-4 without source, profile,
  server-player, or view identity conflation.
- The current scene behaves identically through one participant presentation
  envelope.
- Scripted source assignment produces isolated semantic action frames for 1-4
  participant sessions, including disconnect/reconnect behavior.
- Two logical clients use one realm authority and retain owner-private streams
  and state through join, separation, command routing, and one-client leave.
- Shared horizontal and vertical layouts produce inspected, independently
  posed split-presentation pixels with one shared scene preparation and an
  unchanged mono control.
- No physical collector, product join flow, default layout, guest policy, or
  unreviewed shared-cache architecture is added.

## Progress Evidence

### Slice 1 — bounded participant and cardinality-one migration

Implemented on 2026-07-22. `mclone-app-runtime` now owns opaque
`LocalParticipantId` allocation and a generic `LocalParticipantGroup<T>` with
explicit stable slots, 1-4 admission, non-renumbering removal, and no
serialization or constructor from neighboring identity types. Focused tests
prove all four seats, replacement without ID reuse, and duplicate-slot/full
rejection.

The current scene camera, interaction controller, and player model now live in
one `LocalParticipantPresentation` envelope inside each drawable world slot.
The cardinality-one compatibility dereference preserves all existing behavior;
conflicting runtime/camera borrows use the explicit nested owner. Updated
one-world locks prove the fields no longer sit directly on `DrawableWorldSlot`,
and collector locks keep participant and join policy out of desktop, browser,
and Android physical adapters. Focused app-runtime and complete scene unit tests
pass, as do the affected scene ownership/composition contract suites.

### Slice 2 — participant-local semantic input

Implemented on 2026-07-22. `LocalParticipantInputGroup` now composes the
existing deterministic gamepad assignment reducer with one independent
`ControllerInputSession` per admitted local participant. A sorted scripted join
admits the stable participant slot lazily, reports source and participant facts,
and suppresses the joining source until a fully neutral sample. The group owns
connect, disconnect, reconnect, reservation expiry, explicit participant
removal, context, and preference routing without changing any product
collector.

Focused tests prove two reordered sources with independent gameplay/menu
contexts, exact four-seat admission plus a full result, held/pressed/released
isolation, disconnect release delivery only to the owner, reconnect identity
preservation, and post-expiry reassignment to the still-stable participant.
The focused app-runtime tests, complete `mclone-input` suite, formatting, and
the app-runtime Wasm check pass. The Wasm check retains only pre-existing
target-conditional unused warnings.

### Slice 3 — ordinary local logical clients

Implemented on 2026-07-22. `LocalClientGroup` now maps 1-4 existing participant
admissions to distinct ordinary `ServerPlayerId` and `SingleViewRuntime`
endpoints while retaining one `RealmServer`. Join accepts explicit identities
and rejects a duplicate live profile before realm admission. Commands use the
ordinary per-player handler; global scheduler and simulation methods advance
once and then drain every ordered player stream separately. Initial position
acknowledgement, view changes, and one-participant leave stay participant
addressed.

Synthetic in-memory records give the two endpoints deliberately different
selected slots, experience, statistics, and life state without selecting a
product guest policy. Tests prove configuration/world-info delivery, distinct
client replicas, mutual remote-player visibility, owner-only command updates,
overlapping then separated interest, one-step global simulation progression,
four distinct ordinary player mappings, duplicate-profile rejection, and one
participant leaving without tearing down the survivor. `PlayerRecordKey` now
owns its public profile-ID projection and `RealmServer` exposes the selected
hotbar slot for neutral ownership evidence. The focused app-runtime tests and
all 536 `mclone-server` tests pass.

## Stop Conditions

Stop and ask for direction if implementation requires choosing:

- durable versus disposable guest profiles or platform-account association;
- a default split orientation or live layout-switch behavior;
- per-participant versus global pause/menu ownership;
- audio listener/mix policy;
- helper/builder mutation authority;
- merging owner-private client state or selecting a shared client/GPU cache
  owner for true two-client rendering; or
- weakening ordinary server player/stream semantics for local convenience.
