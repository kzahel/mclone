# 154: Client Ingress Adapter Cleanup

Status: active; drafted 2026-07-07 as the narrow follow-up to tactical
[`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md).
Slice 0 audit/classification completed 2026-07-07; Slice 1 helper quarantine
completed 2026-07-07; Slice 2 native remote wrapper sharing decision is next.
This tactical is cleanup and guardrail work only; it must not change runtime
behavior.

Workstream: native Rust shared session/runtime boundary, native web/WASM
adapters, native desktop/Android/Android XR remote adapters.

## Goal

Tactical 151 won the functional boundary: native local, native remote TCP, web
integrated inline/worker, and web remote WebSocket normal-frame paths all drain
through the shared `ClientConnection` interface and
`pump_client_connection_updates_report`.

This tactical makes that win harder to erode. It cleans up or quarantines the
remaining request/response-shaped helper names and duplicate-looking adapter
surfaces that are still valid but easy to mistake for the preferred runtime
path.

## Related Docs

| Doc | Relationship |
| --- | --- |
| [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md) | Completed the shared client ingress boundary and classified the remaining duplicate-looking hits that this cleanup starts from. |
| [`../session-network-architecture.md`](../session-network-architecture.md) | Owns the target topology: command enqueue and inbound update drain are separate operations, while platform async mechanics remain adapter-local. |
| [`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md) | Owns remaining server-push/protocol broadening. This tactical must not implement server-push or change wire semantics. |
| [`149-remote-contrast-accounting-honesty.md`](149-remote-contrast-accounting-honesty.md) | Historical remote contrast/accounting work that exposed the old send-side and readiness stalls before 151. |
| [`153-vanilla-shaped-chunk-pipeline-capacity.md`](153-vanilla-shaped-chunk-pipeline-capacity.md) | Local-integrated chunk pipeline capacity work. 154 must not block it; remote sessions remain on their existing fail-safe floors there. |
| [`145-native-rust-organization-refactor.md`](145-native-rust-organization-refactor.md) | Broader module-size/organization parent. 154 may feed small findings there, but should not become a general crate reorganization. |
| [`../native-web.md`](../native-web.md) | Web/WASM build and smoke validation context for any web adapter cleanup. |
| [`../platforms.md`](../platforms.md) | Current platform validation policy and platform ownership boundaries. |

## Current Facts

- The canonical shared runtime-facing path is
  `mclone-app-runtime/src/client_connection.rs`:
  `ClientConnection`, `QueuedServerUpdate`, drain result/metrics, drain mode,
  and `pump_client_connection_updates_report`.
- `mclone-app-runtime/src/local_single_view.rs` still contains the native
  local and remote adapter implementations plus the normal runtime call sites.
  That is acceptable after 151, but future edits should not reintroduce a
  host-specific pump.
- Desktop, Android, and Android XR remote wrappers hold
  `NativeClientIoSession`. They still expose methods named
  `drain_command_updates` / `try_drain_command_updates` because the shared
  `RemoteServerSession` trait predates the 151 naming cleanup.
- `mclone-net::NativeClientSession` remains a low-level compatibility/test
  helper and is still used by the standalone native remote-player visual smoke.
  It is not the normal native remote runtime path.
- Web integrated `exchange_command`, `try_recv_update`, and `drain_updates`
  helpers remain compatibility/probe affordances on
  `WebIntegratedServerRunner`. Normal web runtime frame polling goes through
  `WebRuntimeHost`'s `ClientConnection` implementation.
- Web remote still tracks `pending_response_batches`; that is adapter
  bookkeeping for the current one-response-per-command wire protocol, not proof
  that normal frame polling owns command exchange.

## Preferred Client Ingress Path

Future normal-frame client runtime work should start from these entry points:

- Shared contract and pump:
  `native/crates/mclone-app-runtime/src/client_connection.rs`
- Native local and native remote adapter implementations:
  `native/crates/mclone-app-runtime/src/local_single_view.rs`
- Native TCP IO actor:
  `native/crates/mclone-net/src/lib.rs` `NativeClientIoSession`
- Web inline/worker/remote host adapter:
  `native/apps/mclone-web-client/src/lib.rs` `WebRuntimeHost`
- Web remote WebSocket queued-update adapter:
  `native/apps/mclone-web-client/src/web_remote_session.rs`

If a change needs normal frame update ingress and does not use
`ClientConnection` plus `pump_client_connection_updates_report`, it needs a
written reason in the tactical that owns the change.

## Contract For Implementing Agents

- Refactor-only unless this tactical is explicitly revised. Preserve native
  local/remote behavior, web integrated/remote behavior, Android/Android XR
  behavior, startup/reconnect behavior, ordering, budgets, diagnostics meanings,
  and smoke command output.
- Do not implement server-push, WebRTC, compression, or wire protocol
  broadening here. Those belong to tactical 133.
- Do not move browser `web_sys`, JS promise, worker startup, WebSocket callback,
  Android activity, OpenXR session, or native socket ownership into shared
  app-runtime code. Platform lifecycle and physical transport setup stay in
  adapters.
- Prefer documentation, doc comments, narrow renames, and test/probe quarantine
  over structural moves. A rename is allowed only when the call graph is small
  enough to prove no behavior change.
- If a duplicate-looking helper must remain because it is compatibility,
  low-level protocol, startup/bootstrap, or smoke/probe code, make that role
  explicit at the declaration site and in this tactical's audit table.
- The blessed normal-frame path must remain easy to grep:
  `ClientConnection` plus `pump_client_connection_updates_report`.
- Any touched shared crate must still compile for `wasm32-unknown-unknown`.

## Slice 0: Audit And Guardrail Baseline

Status: completed 2026-07-07.

Goal: record the exact remaining old-shape affordances after 151 and decide
which are worth touching.

Deliverables:

- Run the duplicate-shape audit from 151 and classify every hit as one of:
  canonical shared path, normal adapter implementation, compatibility/probe,
  low-level protocol helper, startup/bootstrap, or test-only.
- Confirm the preferred-path note above still matches the code.
- Decide whether Slice 1 can stay limited to doc comments and declarations, or
  whether a small behavior-preserving rename is justified.

Validation:

```bash
rg -n "ClientConnection|QueuedServerUpdate|WebRuntimeHost|exchange_command|NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-web-client native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

Exit criteria: this tactical contains the current audit table and every
remaining old-shape hit has a target classification before any code edits.

Audit results:

| Hit group | Classification | Slice 0 decision |
| --- | --- | --- |
| `mclone-app-runtime/src/client_connection.rs` `ClientConnection`, `QueuedServerUpdate`, drain metrics/result/mode, and `pump_client_connection_updates_report` | canonical shared path | Keep as the preferred normal-frame ingress. |
| `mclone-app-runtime/src/local_single_view.rs` `LocalIntegratedConnection` and `RemoteDedicatedConnection` `ClientConnection` impls | normal adapter implementation | Keep. These are native local/remote adapters over the shared pump, not competing pumps. |
| `mclone-app-runtime/src/local_single_view.rs` `pending_response_batches` | normal adapter bookkeeping | Keep for current one-response-per-command wire pairing; Slice 1 should add a clarifying declaration comment. |
| `mclone-app-runtime/src/host_mode.rs` `RemoteServerSession::{drain_command_updates, try_drain_command_updates}` | normal remote adapter trait with old name | Keep names for now. Renaming would cross desktop/Android/Android XR and test helpers without improving behavior. Slice 1 can document the trait's post-151 role. |
| `mclone-net::NativeClientIoSession` and its drain methods | normal native TCP IO actor | Keep. This is the blessed native remote session used by app wrappers. |
| `mclone-net::NativeClientSession` and its drain methods | low-level protocol/compatibility helper | Keep but quarantine by comments/docs. Normal app runtimes must not use it. |
| `mclone-net` tests using `NativeClientSession` | test-only | Keep. They exercise low-level native transport behavior. |
| `native/apps/mclone-native-client/src/remote_session.rs` | normal desktop adapter | Keep. It wraps `NativeClientIoSession` and implements app-runtime's remote-session trait. |
| `native/apps/mclone-android-client/src/lib.rs` remote session wrapper | normal flat Android adapter | Keep. It wraps `NativeClientIoSession` and owns Android app lifecycle glue. |
| `native/apps/mclone-android-xr-client/src/lib.rs` remote session wrapper | normal Android XR adapter | Keep. It wraps `NativeClientIoSession` and owns XR/Android app lifecycle glue. |
| `native/apps/mclone-native-client/src/remote_player_visual_smoke.rs` `NativeClientSession` | smoke/probe helper | Keep. It is standalone visual-smoke support, not runtime ingress. |
| `native/apps/mclone-web-client/src/lib.rs` `WebRuntimeHost: ClientConnection` | normal web adapter implementation | Keep as the web inline/worker/remote host adapter over the shared pump. |
| `native/apps/mclone-web-client/src/web_remote_session.rs` `pending_response_batches` | normal web adapter bookkeeping | Keep for current WebSocket response-paired wire accounting; Slice 1 should add a clarifying declaration comment. |
| `native/apps/mclone-web-client/src/web_server_worker.rs` `exchange_command`, `try_recv_update`, and `drain_updates` | compatibility/probe helper | Keep but quarantine by comments/docs. Normal web runtime polling drains queued updates through `WebRuntimeHost`. |
| `pump_pending_remote_update_batches_report` | retired old pump | No hits. Keep this as the quick regression grep. |

Preferred path check: the "Preferred Client Ingress Path" section above still
matches the code. The shared pump is in `client_connection.rs`; native adapters
enter through `local_single_view.rs`; normal native remote apps hold
`NativeClientIoSession`; web inline/worker/remote hosts enter through
`WebRuntimeHost`.

Slice 1 direction: keep it to declaration comments and doc comments. Do not
rename `drain_command_updates`, `try_drain_command_updates`, or
`exchange_command` in Slice 1 unless a pre-edit call graph proves the rename is
small enough to stay mechanical.

Validation completed 2026-07-07:

```bash
rg -n "ClientConnection|QueuedServerUpdate|WebRuntimeHost|exchange_command|NativeClientSession|NativeClientIoSession|try_drain_command_updates|drain_command_updates|pending_response_batches|pump_pending_remote_update_batches_report" native/crates/mclone-app-runtime native/crates/mclone-net native/apps/mclone-web-client native/apps/mclone-native-client native/apps/mclone-android-client native/apps/mclone-android-xr-client || true
git diff --check
```

## Slice 1: Quarantine Compatibility And Probe Helpers

Status: completed 2026-07-07.

Goal: make non-normal-path helpers self-identifying.

Likely edits:

- Add doc comments or module-level notes to `mclone-net::NativeClientSession`
  explaining that normal native app runtimes use `NativeClientIoSession`.
- Add notes at web `exchange_command`, `try_recv_update`, and `drain_updates`
  declarations explaining that they are compatibility/probe helpers and not the
  normal web runtime frame path.
- Add a short comment near the native/web `pending_response_batches` fields
  saying they represent current response-paired wire bookkeeping and are not a
  runtime-thread socket ownership model.
- If names are changed, keep them narrowly scoped and update all call sites in
  the same commit. Do not rename public/probe APIs just for aesthetics if that
  obscures smoke scripts or historical evidence.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

Exit criteria: the old-shape helpers remain functional, but their declarations
make it clear they are not the preferred normal-frame ingress.

Results:

- Added a doc comment to `mclone-net::NativeClientSession` identifying it as a
  low-level request/response protocol helper for tests and standalone
  smoke/probe tools; normal app runtimes should use `NativeClientIoSession`.
- Added a compatibility note beside `NativeClientIoSession`'s old
  `drain_command_updates` names. The names remain because they describe the
  current response-paired wire protocol, while app runtimes still drain through
  the shared `ClientConnection` pump.
- Added a trait-level note to `RemoteDedicatedServerSession` explaining that it
  adapts response-paired transport sessions into the shared runtime pump.
- Added clarifying comments beside native and web `pending_response_batches`
  fields: these counters are wire bookkeeping, not runtime-thread socket
  ownership.
- Added comments to web integrated `exchange_command`, `try_recv_update`, and
  `drain_updates` identifying them as compatibility/probe or startup surfaces;
  normal web frame ingress remains `WebRuntimeHost`'s `ClientConnection`
  implementation.
- No renames and no behavior changes.

Validation completed 2026-07-07:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:smoke
git diff --check
```

## Slice 2: Native Remote Wrapper Sharing Decision

Goal: decide whether the desktop, Android, and Android XR remote wrappers have
enough duplicated adapter code to justify one shared native remote adapter.

This is a decision slice first, not a promised refactor. The platform crates
should continue to own launch arguments, Android activity state, OpenXR session
state, socket endpoint selection, and lifecycle wiring. A shared helper is only
worth adding if it removes real duplicated `NativeClientIoSession` glue without
pulling platform lifecycle into app-runtime.

Deliverables:

- Compare the desktop, flat Android, and Android XR `NativeClientIoSession`
  wrappers.
- Record one explicit decision: no action because duplication is small and
  clearer in platform adapters; a shared helper with exact crate/module owner
  and minimal methods; or deferral to tactical 145 because the cleanup is really
  broader organization work.
- If a helper is added, keep it behavior-preserving and leave platform startup
  ownership in the app crates.

Validation if code changes:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
git diff --check
```

Exit criteria: either a small shared helper exists with green validation, or
the tactical records why the platform-local wrappers are intentionally left
alone.

## Deferred Work

These are still valuable but intentionally not in the first cleanup pass:

- **Server-push/protocol broadening:** native TCP and WebSocket server paths
  still emit one response batch per command. Tactical 133 Slice 6 owns deciding
  whether and how remote updates arrive without a paired command response.
- **Hard queue caps/disconnect policy:** 151 exposed queue depth and bytes.
  Hard caps need a clear disconnect/error path and should land as a separate
  runtime policy slice.
- **Full diagnostic vocabulary unification:** depth, byte, age, read/decode,
  pending-response, and transport-drained fields are close enough for current
  evidence. A full naming/shape cleanup should wait until it has a concrete
  consumer.
- **Startup/reconnect behavioral rewrite:** startup and reconnect may
  intentionally use blocking or unlimited drains. This tactical can audit and
  document them, but must not change those semantics casually.
- **WebRTC, compression, or binary protocol redesign:** separate transport
  tacticals only.
- **Terrain throughput/capacity:** tactical 153 and tactical 128 own chunk
  pipeline capacity, dirty-to-drawable lifecycle, mesh admission, and upload
  pacing.
- **General crate/module reorganization:** tactical 145 owns broad file/crate
  organization. 154 should only move code when it directly protects the shared
  ingress boundary.

## Closeout Criteria

- Future agents can identify the preferred normal-frame ingress path without
  reading the whole 151 history.
- Every remaining `exchange_command`, `NativeClientSession`,
  `drain_command_updates`, `try_drain_command_updates`, and
  `pending_response_batches` hit is either renamed, documented, or explicitly
  classified.
- No normal-frame runtime path has a host-specific command-response update pump.
- Server-push/protocol broadening remains clearly owned by tactical 133.
- Tactical README and any affected parent docs point at this tactical as a
  cleanup guardrail, not as a prerequisite for 153.
