# 136: World Catalog And CRUD UI

Status: active; Slices 1-5 shared catalog contract, catalog-aware session
request vocabulary, native filesystem/SQLite catalog backend, shared UI v2
world catalog screens, desktop flat lifecycle wiring, and desktop
persistence/delete smoke coverage landed. Slice 6 browser IndexedDB catalog
store/smoke first pass landed; tactical
[`141-flat-client-platform-policy-convergence.md`](141-flat-client-platform-policy-convergence.md)
Slice 3 wired the native web menu path through the shared catalog controller
and IndexedDB adapter; the 2026-07-05 follow-up added
`pnpm native:web:catalog-smoke` coverage for menu-driven web
Create/Open/Delete and IndexedDB delete cleanup. Owns follow-up persistence
lifecycle work from closed
[`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md).

## Purpose

Add in-game management for local persisted worlds:

- list existing worlds
- create a named world
- open/join an existing local world
- delete a world after confirmation
- keep remote join as a separate session path

This is shared implementation, desktop validation first. The feature should work
through the same shared model on desktop flat, native web, flat Android, desktop
OpenXR, and Android XR. App crates may own platform storage roots, IndexedDB
handles, Android app-private directories, and lifecycle wiring. They must not
own world-management policy.

The important refactor is to add a shared world catalog and local-world identity.
The current runtime can start local worlds by seed and can open a persistent
backend when a launch argument supplies a directory or web world id, but the
menu cannot say "open this saved world" yet.

## Reference Source Read

Read these Java 1.17.1 files before implementing lifecycle or UI behavior:

- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/worldselection/SelectWorldScreen.java`
  - title-side world list, search, select, create, edit, delete, recreate buttons
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/worldselection/WorldSelectionList.java`
  - loads summaries, disables locked/incompatible rows, confirms delete, opens worlds
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/worldselection/CreateWorldScreen.java`
  - name-to-folder selection, seed/options entry, create flow
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/worldselection/EditWorldScreen.java`
  - rename, backup, icon reset, world access locking
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/LevelStorageSource.java`
  - save listing, access lock, `deleteLevel`, `renameLevel`, `saveDataTag`
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
  - `loadLevel`, `createLevel`, `doLoadLevel`, `clearLevel`
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`
  - pause save behavior, shutdown hooks
- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
  - autosave, save-all, stop-server ordering

Useful vanilla shape:

- World CRUD is title-side UI, not running-world UI.
- Create/open obtains a storage access lock, prepares data, clears the current
  level, then starts the integrated server.
- Delete/edit obtains storage access and operates on a non-active save.
- Delete is confirmed, shown behind a progress screen, then the list refreshes.
- Server stop saves players/worlds/chunks and closes storage before deletion is
  allowed to touch that world directory.

## Current Native State

Relevant landed pieces:

- `094-runtime-world-teardown-and-new-world-menu.md` allows no-world title state,
  quit-to-title, and new seed-based local session replacement.
- `095-shared-session-coordinator.md` owns shared create/open-local and
  `JoinRemote { endpoint }` request state across desktop, web, Android, and XR.
- `101-create-world-chunk-progress-screen.md` owns the shared local startup
  progress/pump path.
- `123-ui-v2-menu-rebuild.md` gives the shared UI path retained layout and
  hit-testing for menu screens.
- `134-shared-persistence-architecture.md` closed the first shared persistence
  architecture pass: host-owned `WorldStore`, native threaded SQLite,
  dedicated/desktop local world-dir wiring, browser IndexedDB chunk/entity
  records, autosave/reload, and browser IndexedDB async load-miss handling.
- Slice 1 added `mclone_app_runtime::world_catalog` identity, summary,
  capability, request/response, and UI-suitable error/status types.
- Slice 2 replaced the seed-only local session request variant with
  `CreateLocalWorld { options }` / `OpenLocalWorld { id }`; seed-only
  developer/headless/menu paths now go through a compatibility helper that
  creates a local-world request with `LocalWorldCreateOptions`.
- Slice 3 added native `NativeWorldCatalog` support: one directory per
  `LocalWorldId`, a `world.json` catalog summary, and `world.sqlite3` opened
  through `SqliteWorldStore::open_world_dir`.

Gaps:

- There is no web/Android catalog backend yet, and the native backend is not
  wired to the visible desktop menu/lifecycle path yet.
- `ActiveSessionDescriptor::LocalWorld` can carry save id/display name, but
  live menu-driven local sessions are still transient seed-only until catalog
  create/open wiring exists.
- `mclone-native-client --world-dir PATH` and web
  `worldStorage=indexeddb&worldId=...` are startup/query selectors, not in-game
  UI.
- Native catalog summaries now live beside the database in `world.json`; the
  SQLite metadata table remains store-internal and is not the UI summary source.
- Web IndexedDB has chunk/entity stores keyed by `worldId` and the native web
  menu can now list/create/open/delete catalog worlds through the shared
  controller. Browser player/world metadata, full lifecycle save/flush
  discipline, and Android catalog adoption are still pending.
- Android app-private world roots are still pending.
- Player/world/saved-data records are not live yet, so first world summaries
  will be mostly display id/name/seed/schema/timestamps and coarse diagnostics.
- Browser IndexedDB writes still use the current dirty-record response bridge;
  load misses no longer require whole-world startup preload.

## Target Shape

Add a platform-neutral catalog above `WorldStore`:

```text
WorldCatalog
  list world summaries
  create world container and metadata
  open world by id into a WorldStore backend
  delete inactive world
  rename/update metadata later

WorldStore
  live opened-world chunk/entity/player/saved-data records
  host-owned dirty/save/flush/close semantics
```

The catalog owns containers and summaries. `WorldStore` remains the opened
world's record API. Do not overload `WorldStore` with listing/deleting other
worlds from inside a live server.

Session requests should become identity-bearing:

```text
CreateLocalWorld { options }
OpenLocalWorld { id }
JoinRemote { endpoint }
```

The active descriptor should include the local world id and display name for
local persisted worlds. Seed remains part of creation/open metadata, not the
whole session identity.

## Lifecycle Policy

First implementation should be conservative:

- World CRUD screens are available from Title/no-active-world.
- From Pause, "Quit To Title" must complete clean shutdown before local world
  create/open/delete controls become active.
- Creating/opening another local world while a local or remote session is active
  must route through the shared teardown-before-start path.
- Deleting the active local world is not allowed. The UI should require Quit To
  Title first, then delete from the world list after the live host is closed.
- A remote session does not expose local save deletion as a connected-server
  operation. Remote server world selection is a separate server-admin/session
  feature.
- Storage errors must surface in UI status. Do not silently fall back to
  transient storage when the user selected a persistent world.

This matches vanilla enough for the first product path: leave the world, return
to the title-side selector, then create/open/delete.

## Platform Plan

### Native Desktop And Dedicated

- Add a native world root concept in app-runtime or a small persistence/catalog
  module.
- Represent each world as a directory containing `world.sqlite3` and catalog
  metadata.
- Keep `--world-dir` as a direct-open developer path.
- Add `--world-root` / default app world root for menu-driven desktop local
  worlds once the catalog exists.
- Dedicated server can continue using CLI world selection first. Server-side
  world CRUD/list APIs are not required for the client menu MVP.

### Web

- Add a `worlds` IndexedDB object store for summaries/metadata.
- Keep chunk/entity records keyed by `worldId`.
- Menu-driven Create/Open should start the web integrated worker with
  `worldStorage=indexeddb` and the selected/generated `worldId`.
- Delete removes the catalog row plus chunk/entity records for that id, with
  future player/saved-data stores included when they exist.

### Android And XR

- Flat Android and Android XR use the same shared catalog/session/UI model.
- Platform adapters supply app-private roots and lifecycle flush hooks.
- XR should render the same world list/create/delete screens on the shared menu
  panel; no XR-only world-management semantics.

## Implementation Chunks

### Slice 0: Tactical And Reference Baseline

Status: this document.

Deliverables:

- reference source list
- vanilla lifecycle summary
- target shared catalog/session/UI plan
- tactical index link

### Slice 1: Shared World Identity And Catalog Contract

Status: landed 2026-07-03.

Add platform-neutral data types without changing visible UI yet:

- `LocalWorldId`
- `LocalWorldSummary`
- `LocalWorldCreateOptions`
- `WorldCatalog` request/result surface
- storage capability flags such as persistent/transient/delete-supported
- error/status types suitable for UI

Keep the first contract small: list/create/delete/open-path construction. Rename
and backup can wait.

Validation:

- unit tests for id normalization, duplicate name handling, and delete-active
  rejection policy at the shared layer
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`

Recorded Slice 1 result:

- Added `mclone_app_runtime::world_catalog` with `LocalWorldId`,
  `LocalWorldSummary`, `LocalWorldCreateOptions`, `WorldCatalogRequest`,
  `WorldCatalogResponse`, `WorldCatalogCapabilities`, UI-suitable
  status/error types, and `validate_delete_inactive_world`.
- Kept IDs platform-neutral and path-safe: display names normalize to
  lowercase ASCII slug ids, explicit IDs must already be normalized, and
  duplicate display-name candidates get deterministic numeric suffixes.
- Kept delete-active rejection in the shared layer so desktop, web, Android,
  and XR adapters cannot accidentally expose active-save deletion with private
  policy.
- No visible UI, session request, or storage backend behavior changed.

Validation after Slice 1 on 2026-07-03:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime world_catalog
```

The catalog-specific test filter passed 8/8. The broader
`cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime` command
compiled and all `world_catalog` tests passed, but nine unrelated
`local_single_view` tests failed during textured terrain asset loading because
the current asset tree reports `minecraft:cactus[age=0]` as missing the
`age=0` blockstate variant. That failure is outside this catalog contract
slice.

Follow-up: the cactus/sugar-cane asset-load blocker is cleared in the shared
asset resolver. Known non-model properties such as cactus/sugar-cane `age=0`
now resolve through vanilla's empty blockstate model variant, while unknown
properties still fail. The focused `mclone-assets` / `mclone-mesh` /
`mclone-app-runtime` test set passed after the fix.

### Slice 2: Session Requests Carry Local World Identity

Status: landed 2026-07-03.

Refactor the session coordinator vocabulary:

- replace menu-facing `NewLocalWorld { seed }` with create/open requests that
  carry catalog facts
- keep compatibility helpers for seed-only developer/headless paths
- update `ActiveSessionDescriptor::LocalWorld` to include world id/display name
  when persistent
- keep `JoinRemote { endpoint }` as the remote path

No platform should grow a private "selected save" global outside this shared
request/descriptor path.

Validation:

- session coordinator tests for create/open/remote state transitions
- desktop and web compile gates after request enum changes

Recorded Slice 2 result:

- Replaced `SessionStartRequest::NewLocalWorld { seed }` with
  `CreateLocalWorld { options: LocalWorldCreateOptions }` and
  `OpenLocalWorld { id: LocalWorldId }`.
- Added `SessionStartRequest::new_seed_local_world(seed)` as the compatibility
  helper for current reroll, headless, smoke, startup, and transient local
  paths. Those paths now produce create-local requests instead of owning a
  seed-only variant.
- Extended `ActiveSessionDescriptor::LocalWorld` with optional `id` and
  `display_name`, plus helpers for transient seed worlds and catalog summaries.
  Persistent descriptors round-trip back to `OpenLocalWorld { id }`; transient
  descriptors keep using the seed helper.
- Updated desktop flat, native single-view, web/WASM, flat Android, desktop XR,
  Android XR, and XR scene replacement/startup code to use the new vocabulary.
  `OpenLocalWorld` is represented but intentionally rejected by runtime
  factories until Slice 3 supplies catalog summaries and backends.

Validation after Slice 2 on 2026-07-03:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
pnpm native:web:build
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
```

### Slice 3: Native Catalog Backend

Status: landed 2026-07-03.

Implement the first catalog backend for desktop validation:

- world root directory
- create world directory and metadata
- list summaries sorted by last played or created time
- open summary into `SqliteWorldStore::open_world_dir`
- delete inactive world directory with retry/clear error reporting
- preserve direct `--world-dir` startup as a developer bypass

Metadata should include at least:

- world id
- display name
- seed
- created/last played timestamps
- storage schema version
- target Minecraft/mclone version stamps available today

Validation:

- create/list/open/delete unit tests using temp dirs
- reopen test proves a chunk record survives through catalog-opened SQLite
  storage
- delete removes the directory only when no session is active

Recorded Slice 3 result:

- Added native-only `NativeWorldCatalog` in `mclone_app_runtime::world_catalog`
  with a persistent-local capability surface and shared
  `WorldCatalogRequest`/`WorldCatalogResponse` dispatch.
- Stores each catalog world under `<world-root>/<LocalWorldId>/` with
  `world.json` metadata and `world.sqlite3` initialized/opened by the existing
  `SqliteWorldStore::open_world_dir` backend.
- `create_world` validates display/id facts, suffixes duplicate display-name
  IDs, rejects duplicate requested IDs, initializes SQLite, stamps schema,
  target Minecraft version, mclone package version, created time, and
  last-played time, and cleans up the world directory if initialization fails.
- `list_worlds` reads catalog summaries and sorts by last-played/created time;
  `open_world` updates last-played and validates SQLite availability;
  `open_sqlite_world_store` returns the opened summary/path plus the durable
  store for session wiring; `delete_world` reuses the shared active-world
  rejection before removing the world directory.
- Direct `--world-dir` startup remains unchanged as a developer bypass; this
  backend is ready for menu/lifecycle wiring in the next slice.

Validation after Slice 3 on 2026-07-03:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime world_catalog
```

The catalog-specific test filter passed 13/13, including create/list/open/delete
coverage, request dispatch, duplicate requested-id rejection, missing-world
errors, active-world delete rejection, and a catalog-opened SQLite store
round-tripping a chunk record across reopen.

### Slice 4: Shared UI Screens

Status: first pass landed 2026-07-04.

Add UI v2 screens in `mclone-ui`:

- Title button becomes "Singleplayer" or opens a world list instead of direct
  reroll-only New World.
- World list screen: rows, selected row, Open, Create, Delete, Back.
- Create world screen: display name, seed display/reroll, Create, Back.
- Delete confirm/progress/error flow.

First pass can avoid text entry if needed by generating display names, but the
target should include a text field for world name once UI v2 text input exists.

Validation:

- `mclone-ui` hit-test/action tests for list selection, create, delete confirm
- desktop headless UI screenshots for title, world list, create, delete confirm
- inspect screenshots in `/tmp`

Recorded Slice 4 result:

- Added dependency-free shared catalog UI types in `mclone-ui`: fixed-capacity
  `WorldCatalogUiState`, `WorldCatalogUiEntry`, fixed-size UI text, opaque
  `WorldCatalogUiWorldId`, status/capability flags, selected/active ids, and
  create draft display text.
- Added catalog UI actions while keeping the old seed-only `NewWorld` path for
  developer/headless compatibility: `OpenWorldList`, `OpenWorldCreate`,
  `SelectWorld`, `OpenWorld`, `CreateCatalogWorld`, `ConfirmDeleteWorld`,
  `DeleteWorld`, and `CancelDeleteWorld`.
- Routed the title "Singleplayer" button to the shared world-list screen.
  Added retained UI v2 screens for world list, create world, and delete
  confirmation, including stable row/button widget ids and hit-test actions.
- Added placeholder handling in desktop flat, web, Android, and XR action
  handlers so submit actions compile and remain inert until Slice 5 wires the
  catalog backend/lifecycle. Navigation and selection are shared UI behavior.
- Extended native headless `--screenshot-ui` with `world-list`, `world-create`,
  and `world-delete-confirm` so the new screens can be rendered directly.

Validation after Slice 4 first pass on 2026-07-04:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
pnpm native:web:build
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client parse_screenshot_ui_accepts_named_screens
```

Native screenshots inspected:

```bash
/tmp/mclone-world-list-ui-clean.png
/tmp/mclone-title-singleplayer-ui.png
```

The world-list capture rendered a clean, centered `SELECT WORLD` panel with
disabled catalog controls until Slice 5 supplies a live native catalog state.
The title capture rendered the title screen with `SINGLEPLAYER` as the first
button.

### Slice 5: Desktop Lifecycle Wiring

Status: first pass plus desktop persistence/delete smokes landed 2026-07-04.

Wire desktop flat end to end:

- boot to title/world list when requested
- create persistent local world through the catalog
- open existing persistent world by id
- quit-to-title shuts down local/remote runtime and then unlocks world CRUD
- delete disabled for active world and enabled after shutdown completes
- status/progress overlays for create/open/delete errors

Use the existing startup pump from tactical 101 for local world startup.

Validation:

- desktop app tests for create -> edit block -> quit -> open -> verify edit
- delete after quit removes the catalog entry and blocks reopen
- attempt to delete active world is rejected
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`

Recorded Slice 5 result:

- Added a desktop `--world-root PATH` catalog root with default app-data
  placement and kept `--world-dir PATH` as the direct-open developer bypass.
  `--transient` disables the menu-managed native catalog and preserves
  transient-only behavior.
- Wired `FlatClientDriver` to a native `NativeWorldCatalog` cache that converts
  `LocalWorldSummary` values into shared `WorldCatalogUiState` rows with stable
  UI row keys, selected row state, active-world marking, capability flags, and
  UI status messages.
- Replaced desktop placeholder handling for `OpenWorld`, `CreateCatalogWorld`,
  and `DeleteWorld`: create/open resolve catalog summaries and queue the
  existing non-blocking local startup pump with identity-bearing session
  descriptors; delete removes inactive worlds and surfaces active-world/storage
  errors in the world-list UI.
- Extended the desktop pending-start payload so `OpenLocalWorld { id }` can keep
  the shared request vocabulary small while carrying the resolved catalog
  summary descriptor through desktop lifecycle glue.
- Fixed the full-frame/headless render path to commit the live driver catalog
  state, so screenshots and desktop rendering show persistent catalog
  availability instead of the Slice 4 placeholder state.
- Added a desktop app persistence smoke covering catalog create, catalog-backed
  runtime start, debug block break, quit to title, catalog reopen, and
  verification that the block edit survives through the catalog world
  directory.
- Added a desktop app delete-after-quit smoke covering catalog create,
  catalog-backed runtime start, clean quit to title, title-side delete, catalog
  entry removal, world directory removal, and blocked reopen of the deleted UI
  row.
- Added a desktop app active-world delete rejection smoke covering
  catalog-backed runtime start, active row marking, rejected active save
  deletion, visible UI error status, and preservation of the live runtime,
  session descriptor, catalog row, and world directory.

Validation after Slice 5 desktop persistence/delete smokes on 2026-07-04:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_world_create_edit_quit_reopen_preserves_block_edit
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_world_delete_after_quit_removes_entry_and_blocks_reopen
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_world_delete_active_world_is_rejected_without_teardown
cargo test --manifest-path native/Cargo.toml -p mclone-native-client catalog_
cargo test --manifest-path native/Cargo.toml -p mclone-native-client cli_rejects_conflicting_world_storage_args
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo fmt --manifest-path native/Cargo.toml --all --check
git diff --check
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-world-list-slice5.png --screenshot-ui world-list --world-root /tmp/mclone-slice5-worlds-ui-20260704b --startup-wait none --width 1280 --height 720
```

Native screenshot inspected:

```bash
/tmp/mclone-world-list-slice5.png
```

The world-list capture rendered a persistent empty catalog state (`NO WORLDS
FOUND`) with `Create` enabled and `Open`/`Delete` disabled.

### Slice 6: Web IndexedDB Catalog

Status: first catalog-store/smoke pass landed 2026-07-04.

Add browser catalog support behind the same shared model:

- `worlds` object store
- list/create/open/delete bridge between Rust and TypeScript worker glue
- menu-driven Create/Open starts IndexedDB-backed local workers
- delete clears all stores for that world id

Validation:

- Playwright create/open/reload/delete probe
- block edit survives reload before delete
- deleted world no longer appears and has no chunk/entity records
- `pnpm native:web:build`
- `pnpm native:web:smoke`

Recorded Slice 6 first-pass result:

- Added a browser `worlds` IndexedDB object store alongside the existing
  chunk/entity stores and bumped the web world database schema to version 2.
- Centralized browser world-store schema setup and per-world chunk/entity
  cleanup in `mclone-web-world-catalog.ts` so the integrated-server worker and
  smoke page share the same store names, version, and record cleanup behavior.
- Added dependency-free browser catalog CRUD helpers that mirror the shared
  `LocalWorldSummary` shape, display-name slug/id rules, persistent
  IndexedDB backend label, duplicate id rejection, active-world delete
  rejection, last-played update on open, summary sorting, and delete cleanup.
- Extended the browser smoke page with an IndexedDB catalog probe covering
  create/list/open/delete, duplicate-id rejection, active-world delete
  rejection, and deletion clearing both chunk and entity stores for the world.
- Left menu-driven web Create/Open/Delete wiring pending; the Rust web UI still
  renders an empty/default catalog state until the next Slice 6 chunk feeds
  these browser summaries into `GameUiRenderState` and routes catalog actions.

Follow-up after tactical 141 Slice 3:

- Menu-driven native web Create/Open/Delete now uses the shared
  `FlatClientCatalogController`; IndexedDB promises remain web-local and feed
  controller completions back through wasm.
- `WebChunkRenderSession` now renders controller-owned catalog state and starts
  catalog-created/opened local worlds with `worldStorage=indexeddb` plus the
  selected/generated `worldId`.
- `pnpm native:web:catalog-smoke` now drives the native web Rust menu through
  create/open/delete, verifies active local `sessionWorldId` transitions, and
  confirms delete clears seeded chunk/entity IndexedDB records for the deleted
  world.
- Remaining browser persistence work is lifecycle depth, not menu policy:
  player/world metadata records, save/flush shutdown discipline, richer
  summaries, and folding catalog coverage into broader app-smoke once the
  unrelated block-interaction probe is stable.

Validation after Slice 6 first pass on 2026-07-04:

```bash
pnpm exec tsc --noEmit -p native/apps/mclone-web-client/tsconfig.json
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:smoke
```

Additional validation after tactical 141 Slice 3 follow-up on 2026-07-05:

```bash
pnpm native:web:typecheck
pnpm native:web:catalog-smoke
cargo fmt --manifest-path native/Cargo.toml --all --check
```

### Slice 7: Android And XR Adoption

Adopt the shared catalog and screens:

- flat Android app-private root
- Android XR app-private root
- desktop XR shared menu-panel world list/create/delete
- Android XR shared menu-panel world list/create/delete

Validation:

- flat Android AVD smoke: create world, place/break block, restart/open
- Android XR headset smoke: create/open existing local world from shared panel
- XR delete flow checked from title/no-active-world panel

### Slice 8: Polish And Later CRUD

Deferred until the MVP is stable:

- rename world
- duplicate/recreate from seed/settings
- backup/export
- world icons/screenshots
- richer compatibility/migration UI
- player position/world time summaries after player/world records land
- remote server-side world browser/admin API

## Post-UI Persistence Follow-Ups

These are not blockers for the first catalog/list/create/open/delete UI, but
they should be picked up after the basic world UI and lifecycle flow is usable.
They are carried here from the closeout of
[`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md)
so the old architecture tactical can stay closed.

### Follow-Up A: First Metadata/World Records

Add live metadata records that let the catalog summarize and resume worlds
without inferring facts from chunk/entity stores:

- persisted world display name, seed, created time, last played time
- schema/content version stamps
- last-opened platform/backend diagnostics where useful
- world game time and spawn position once the shared world record exists

Validation:

- create/open updates metadata timestamps without touching chunk data
- browser IndexedDB and native SQLite list the same summary fields through the
  shared catalog model
- incompatible metadata fails with a visible UI error, not transient fallback

### Follow-Up B: Player And Saved-Data Records

After the catalog UI can open named worlds, add the first non-chunk live record
families:

- player position/rotation, selected hotbar slot, and spawn/home fields that
  exist in the runtime
- world time/weather-ready slots as the world record grows
- saved-data key/value plumbing for future maps, counters, and global state
- UI summary fields that depend on those records, such as last player position
  or world time, only after the records are authoritative

Validation:

- quit/reopen resumes player position and selected slot in a local world
- world time resumes across desktop SQLite and browser IndexedDB
- remote sessions do not write accidental local player/world records

### Follow-Up C: Browser Save Path Cleanup

The browser now loads chunk/entity records through explicit external
request/completion records, but saves still flow through dirty-record arrays
attached to normal worker responses. Convert browser saves to the same external
storage discipline once the world UI path is stable:

- emit save requests through the shared persistence mailbox/backend shape
- let TypeScript acknowledge durable chunk/entity writes explicitly
- preserve pending-write visibility and revision precedence in Rust
- keep shutdown/flush semantics visible to the session coordinator

Validation:

- dirty block edit survives reload without relying on response-attached dirty
  arrays
- save acknowledgements gate unload/shutdown the same way on web and native
- failed IndexedDB write reports a visible storage error and does not pretend
  the world is saved

### Follow-Up D: Android App-Private Storage

Adopt the same catalog/session path on flat Android and Android XR:

- app-private world root for SQLite-backed local worlds
- lifecycle flush on pause/stop where practical
- validation that local and remote host modes remain separate
- shared UI panels for world list/create/delete on flat Android and XR

Validation:

- flat Android create/edit/restart/open smoke
- Android XR create/edit/restart/open headset smoke
- remote Android/XR sessions do not create local authority saves

### Follow-Up E: Entity Persistence Hardening

Chunk/entity record persistence exists, but generated-original entity semantics
are still a separate correctness slice:

- generated passive animal survives chunk unload/reload and process/page reload
- killed generated passive animal does not reappear from seed-time generation
- moved entity saves under the destination chunk
- empty entity chunk records suppress seed respawn ambiguity

Validation:

- desktop SQLite and browser IndexedDB entity reload smokes
- no resurrection after delete/kill across unload, page reload, and process
  restart where the platform supports it

## Guardrails

- Do not put catalog policy in `mclone-native-client`, web TypeScript, Android
  activity glue, or XR scene code.
- Do not make web IndexedDB semantics the shared model. IndexedDB is one
  adapter behind the catalog contract.
- Do not delete or mutate the active world container.
- Do not silently reset incompatible durable records. Cache reset may be
  explicit; durable data loss requires a user-facing reset/delete decision.
- Do not block the render/event loop on slow storage. Native may use helper
  tests that block, but the runtime path should report progress/status.
- Keep remote dedicated join separate from local persisted-world CRUD.

## Related

- [`094-runtime-world-teardown-and-new-world-menu.md`](094-runtime-world-teardown-and-new-world-menu.md)
- [`095-shared-session-coordinator.md`](095-shared-session-coordinator.md)
- [`101-create-world-chunk-progress-screen.md`](101-create-world-chunk-progress-screen.md)
- [`123-ui-v2-menu-rebuild.md`](123-ui-v2-menu-rebuild.md)
- [`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../loading-persistence.md`](../loading-persistence.md)
