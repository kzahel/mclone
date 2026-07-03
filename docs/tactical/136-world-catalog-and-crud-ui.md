# 136: World Catalog And CRUD UI

Status: proposed; owns follow-up persistence lifecycle work from closed
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
- `095-shared-session-coordinator.md` owns shared `NewLocalWorld { seed }` and
  `JoinRemote { endpoint }` request state across desktop, web, Android, and XR.
- `101-create-world-chunk-progress-screen.md` owns the shared local startup
  progress/pump path.
- `123-ui-v2-menu-rebuild.md` gives the shared UI path retained layout and
  hit-testing for menu screens.
- `134-shared-persistence-architecture.md` closed the first shared persistence
  architecture pass: host-owned `WorldStore`, native threaded SQLite,
  dedicated/desktop local world-dir wiring, browser IndexedDB chunk/entity
  records, autosave/reload, and browser IndexedDB async load-miss handling.

Gaps:

- There is no shared world catalog.
- `SessionStartRequest` and `ActiveSessionDescriptor` identify a local session
  only by seed, not by save id, display name, world root, or storage backend.
- `mclone-native-client --world-dir PATH` and web
  `worldStorage=indexeddb&worldId=...` are startup/query selectors, not in-game
  UI.
- Native SQLite has a metadata table, but no shared world-summary record family
  is used by UI.
- Web IndexedDB has chunk/entity stores keyed by `worldId`, but no visible
  browser world list/create/delete UI.
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

### Slice 2: Session Requests Carry Local World Identity

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

### Slice 3: Native Catalog Backend

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
- restart/open test proves a block edit survives through catalog open
- delete removes the directory only when no session is active

### Slice 4: Shared UI Screens

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

### Slice 5: Desktop Lifecycle Wiring

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

### Slice 6: Web IndexedDB Catalog

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
