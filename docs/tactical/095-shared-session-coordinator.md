# 095: Shared Session Coordinator

Status: active; Slices 1-4a landed on 2026-06-27. `mclone-app-runtime` now owns
the platform-neutral session request/state vocabulary and shared start-result
boundary, including local-world start, remote-join start, active session
descriptors, pending starts, failure state, status text, and success/failure
transition policy. Desktop flat consumes that coordinator for initial startup,
queued New World creation, and the first Join Remote menu flow. Web/WASM now
uses the same coordinator vocabulary for initial local-worker and remote
WebSocket starts and publishes shared session state to the browser diagnostics
surface. Dynamic web menu-driven New World/Join Remote replacement, manual
remote address entry, and XR/Android coordinator adoption remain follow-ups.

## Purpose

World creation is one way to enter a play session. Joining a network host is
another. Those paths should not become separate desktop/web/XR implementations
with private lifecycle state. The common part is session coordination:

1. what the user requested (`NewLocalWorld`, `JoinRemote`, future P2P/session
   host),
2. whether that request is pending, starting, active, failed, or cleared,
3. what status/error the UI should show,
4. when an existing session must be torn down before the next one starts.

The platform-specific part remains outside the coordinator: `winit` windows,
browser workers/WebSockets, OpenXR sessions/swapchains, Android lifecycle, and
the concrete runtime factory that turns a request into a live renderable
session.

## Java Shape

Minecraft Java does not treat "create a world" and "join a server" as unrelated
application states.

- Local create/load enters through `Minecraft.createLevel(...)`,
  `Minecraft.loadLevel(...)`, and `Minecraft.doLoadLevel(...)`
  (`reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java:1807`,
  `:1811`, `:1832`). That path clears the current level and starts an
  integrated server-backed client connection.
- Remote join enters through `ConnectScreen.startConnecting(...)`, which also
  clears the current level before opening a network connection
  (`reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/ConnectScreen.java:47`).
- Both paths converge on client-world installation through
  `ClientPacketListener.handleLogin(...)`, which creates the `ClientLevel` and
  calls `Minecraft.setLevel(...)`
  (`reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java:331`,
  `:352`; `Minecraft.java:2017`).
- Disconnect/teardown is centralized through `Minecraft.clearLevel(...)`
  (`Minecraft.java:2036`, `:2040`).

The native shape should mirror that coordination: local integrated, remote
dedicated, and future P2P enter through a shared session request/state model,
then platform adapters perform the concrete construction and transport work.

## Target Shape

- `mclone-app-runtime` owns the session lifecycle vocabulary and simple state
  machine: no session, starting request, active descriptor, failed request.
- App crates provide payloads/factories for their platform. A desktop pending
  start may carry `SceneOptions`; web may carry async worker/socket setup; XR
  may carry OpenXR-scene startup data. The request/status semantics stay shared.
- UI surfaces render status from the shared state rather than inventing
  per-platform "creating world" or "connecting" flags.
- Local integrated, remote dedicated, and future P2P/session-host requests can
  share menu intent, lifecycle, teardown, status, and error policy even though
  their transport adapters differ.
- The coordinator must not own platform resources. It should not know about
  `wgpu::Surface`, `winit`, OpenXR swapchains, browser `WebSocket`, workers, or
  Android activity lifecycle.

## Implementation Slices

### Slice 1 - Shared Request/State Vocabulary

- [x] Add `mclone_app_runtime::session` with:
  - `GameSessionCoordinator<P>`
  - `GameSessionState`
  - `SessionStartRequest`
  - `PendingSessionStart<P>`
  - `ActiveSessionDescriptor`
  - `RemoteSessionEndpoint`
  - `SessionFailure`
  - `SessionStatus`
- [x] Support `NewLocalWorld { seed }`, `JoinRemote { endpoint }`, and an
  explicit `Unknown` fallback for defensive failure reporting.
- [x] Provide shared status text for "Creating world..." and "Connecting...".
- [x] Refactor desktop flat to use the coordinator for:
  - initial world auto-start status/failure,
  - queued New World creation,
  - loading/error overlay derivation,
  - pending start completion after a visible status frame,
  - clearing inactive session status when navigating away.
- [x] Keep concrete runtime rebuild in the desktop app for now; only the
  coordination state moved common.

Recorded Slice 1 result:

- Desktop `ChunkApp` now owns `GameSessionCoordinator<PendingWindowSessionStart>`
  instead of app-local `pending_world_start` and `world_status` fields.
- New World creation records a shared `SessionStartRequest::NewLocalWorld`, then
  consumes a `PendingSessionStart` on the next presented/skipped frame.
- Startup maps `SceneOptions.remote_addr` to `JoinRemote` and local scenes to
  `NewLocalWorld`, so the state vocabulary already covers both entry modes.
- The native client still only implements queued local-world creation from the
  menu. Remote join request creation is represented but not yet menu-driven.

Validation after Slice 1 on 2026-06-27:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
git diff --check
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot C:\tmp\mclone-shared-session-coordinator-new-world.png --screenshot-ui new-world --width 960 --height 540 --seed 12345
```

The New World screenshot was inspected and showed the menu rendered correctly
without blank output or overlapping UI.

### Slice 2 - Shared Session Factory Boundary

- [x] Define the smallest common boundary between a session request and a live
  platform session. Likely shape: common request/descriptor/result types in
  `mclone-app-runtime`, with platform-specific factories implemented in app
  crates.
- [x] Keep synchronous desktop construction and async browser construction
  behind different adapters rather than forcing one async model everywhere.
- [ ] Move common teardown-before-start and failure-status policy behind this
  boundary once two platforms consume it.

Recorded Slice 2 result:

- Added `StartedGameSession<S>` and `SessionStartResult<S>` to
  `mclone_app_runtime::session`, so platform factories can return a live
  session payload plus the common `ActiveSessionDescriptor`.
- Added `SessionStartRequest::active_descriptor()` and
  `default_failure_message()` so local-world and remote-join requests derive
  common success/failure facts without desktop-only matching.
- Added `GameSessionCoordinator::apply_start_result(...)` and
  `start_pending_with(...)` to centralize the transition from pending/starting
  to active or failed.
- Refactored desktop startup and queued New World creation to construct a
  `SessionStartResult<()>` through a desktop-local runtime factory, then apply
  the common result to the coordinator. The `()` payload means the live runtime
  is already installed into the desktop app; future adapters can use a concrete
  session payload when that is a better ownership shape.
- Kept browser async and XR platform mechanics untouched. No common async trait
  or platform resource ownership was introduced.

Validation after Slice 2 on 2026-06-27:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
git diff --check
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot C:\tmp\mclone-session-result-boundary-new-world.png --screenshot-ui new-world --width 960 --height 540 --seed 12345
```

The New World screenshot was inspected and still rendered correctly after the
shared result-boundary refactor.

### Slice 3 - Join Remote Menu Flow

- [x] Add a UI action/screen path for joining a remote host without baking it
  into desktop-only code.
- [x] Route the request through `SessionStartRequest::JoinRemote`.
- [x] Reuse existing remote-dedicated transport/session code for the desktop
  factory, and keep the web factory on WebSocket/worker mechanics.

Recorded Slice 3 result:

- Added shared `GameScreen::JoinRemote`, `GameUiAction::OpenJoinRemote`, and
  `GameUiAction::JoinRemote` in `mclone-ui`.
- Added a Title-screen Join Remote entry and a first Join Remote screen with a
  displayed endpoint plus Connect/Back buttons. The endpoint uses the existing
  `--remote-addr` value when present and otherwise defaults to
  `127.0.0.1:25565`. Manual text entry is still deferred.
- Desktop `ChunkApp` now queues Join Remote through
  `SessionStartRequest::JoinRemote { endpoint }`, uses the same pending loading
  frame and shared `SessionStartResult` transition as New World, and preserves
  the Join Remote screen plus endpoint on connection failure.
- Web, flat Android, and XR shared UI consumers now compile with the new action
  and screen labels but intentionally ignore remote-join execution until their
  coordinator-adoption slices.
- Added screenshot support for `--screenshot-ui join-remote`.

Validation after Slice 3 on 2026-06-27:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot C:\tmp\mclone-join-remote-menu.png --screenshot-ui join-remote --width 960 --height 540
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot C:\tmp\mclone-title-join-remote-entry.png --screenshot-ui title --width 960 --height 540
```

The Join Remote and updated Title screenshots were inspected and rendered
without blank output or overlapping UI.

### Slice 4 - Platform Adoption

- [x] Web/WASM consumes the common coordinator for initial local integrated and
  remote WebSocket starts while preserving browser async startup.
- [ ] Web/WASM routes menu-driven New World and Join Remote actions through an
  async restart/reconnect factory instead of only publishing the UI intent.
- [ ] Flat Android consumes the common coordinator for local and remote starts.
- [ ] Desktop XR and Android XR consume the same request/status model while
  keeping OpenXR scene/session ownership in XR adapters.

Recorded Slice 4a result:

- `WebChunkRenderSession` now owns a `GameSessionCoordinator<()>` and initializes
  it from `SessionStartRequest::NewLocalWorld` for local-worker startup or
  `SessionStartRequest::JoinRemote` for remote WebSocket startup.
- Web UI status reports now include shared session fields:
  `sessionState`, `sessionKind`, `sessionSeed`, `sessionRemoteEndpoint`,
  `sessionStatusVisible`, `sessionStatusOk`, and `sessionStatusMessage`.
- The browser app mirrors those fields into `globalThis.__mcloneWebApp.state`,
  so smoke tests and diagnostics see the same common session state as Rust.
- The browser app-loop smoke now asserts that local-worker mode publishes an
  active `localWorld` session and remote WebSocket mode publishes an active
  `remote` session with the requested endpoint.
- The Playwright native UI probe now clicks shared-menu button centers derived
  from the Rust GUI scale rules and exercises the current Title -> New World ->
  Create World intent flow. Actual web runtime teardown/replacement from that
  menu action is intentionally still deferred to the next web slice.

Validation after Slice 4a on 2026-06-27:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
pnpm native:web:typecheck
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:web:app-smoke
pnpm native:web:remote-smoke
git diff --check
```

The web native UI screenshot at `/tmp/mclone-native-web-ui-canvas.png` was
inspected after the remote WebSocket smoke and showed the shared title UI
rendering nonblank without central menu overlap.

## Open Questions

- Do we want `ActiveSessionDescriptor` to grow world identity/save-slot facts
  before persistence lands, or keep it seed/endpoint-only until the world list
  exists?
- Should "New World" immediately clear an active session before the loading
  frame, or should the old world remain visible until the replacement is ready?
  Current desktop behavior favors the visible loading frame and then swaps.
- How much retry policy belongs in the coordinator versus transport adapters?
  Initial answer: coordinator records failure and request identity; transport
  adapters own reconnect/retry mechanics.
