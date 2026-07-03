# 133: Session Network Bus And Update Pacing

Status: active tracking doc; Slice 1 send-only high-frequency command policy
landed for local integrated play; Slice 2 runtime update pump budget is next
Workstream: shared native Rust app runtime, local integrated server runner,
native remote transport, web/WASM host convergence, Android XR frame pacing

## Impetus

The Quest render-distance-7 frame pacing work exposed an architectural problem,
not just a bad cap. The measured `commit_server_command_ms` tail was not really
command send time. The split in `130` showed the command send itself was tiny,
while the expensive part was immediate draining and applying of pending
server updates. The later actor-buffer fix in `131` removed the actor spike
family, leaving server-update application and dirty marking as one of the
remaining CPU-busy frame families.

The current local integrated path has a server thread and an update queue, but
`send_gameplay_command_timed` still does this on the app/render thread:

```text
send ClientCommand
drain all pending ServerUpdate frames
mark render dirty for those updates
apply the updates to ClientRuntime
```

That makes a high-frequency XR pose sync capable of absorbing a chunk streaming
burst. A bad frame can contain 15 `ChunkSnapshot` updates plus 15 `ChunkUnload`
updates inside the locomotion bucket. That shape is neither Java-like nor a good
native/network architecture.

The target architecture is documented in
[`../session-network-architecture.md`](../session-network-architecture.md). This
tactical tracks the implementation slices.

## Desired Direction

Replace request/response-style session helpers with a shared client session bus:

```text
outbound ClientCommand stream
inbound ServerUpdate stream
runtime update pump with priority and budget
```

The runtime remains frame-driven. Network IO, local integrated server runner
bridging, WebSocket callbacks, and future WebRTC data channels feed queues
behind a common policy boundary.

## Related Work

| Doc | Relationship |
| --- | --- |
| `062-shared-threading-topology.md` | Parent native-thread / web-worker topology. This tactical narrows the client/session bus part. |
| `085-web-host-mode-convergence.md` | Existing shared host-mode policy and web async adapter split. This tactical replaces request/response semantics without forcing native async. |
| `095-shared-session-coordinator.md` | Session lifecycle state remains separate from transport IO and update pacing. |
| `119-android-xr-live-streaming-frame-pacing.md` | Product-lane RD7/RD10 burst evidence and pacing goals. |
| `128-terrain-render-pipeline-coordination.md` | Terrain dirty-to-drawable bus that consumes update-driven dirty work after this pump. |
| `130-quest-thread-scheduling-and-streaming-tail-attribution.md` | Measured command split proving send was cheap and update apply/dirty marking was the tail. |
| `131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md` | Current next-step context after actor resource lifetime fix; points here for update pacing. |

## Current Code Facts

- `NativeIntegratedServerRunner` already has a command channel and update
  channel.
- `NativeIntegratedServerRunner::drain_updates` currently drains the whole
  update channel when asked.
- `LocalSingleViewSceneRuntime::send_gameplay_command_timed` sends a command,
  immediately drains all updates, and immediately applies them.
- `LocalSingleViewSceneRuntime::poll` also drains/applies updates, so there is
  already a better-owned runtime poll point.
- Native remote TCP still blocks the caller in `NativeClientSession::send_command`
  while reading one response batch.
- Web remote WebSocket is event-driven underneath, but the Rust-facing session
  is still shaped as command exchange.

## Slices

### Slice 0 - Documentation Baseline

- [x] Add the durable session/network architecture document.
- [x] Add this tactical tracking doc.
- [x] Keep `protocol.md` clear that request/response is current transport
  behavior, not the long-term session model.

### Slice 1 - Send-Only Policy For High-Frequency Commands

Goal: remove the accidental coupling causing the current Quest tail without
rewriting every transport.

- [x] Add an explicit command send policy in shared app runtime:
  `SendOnly` versus the default `DrainImmediately`.
- [x] Use `SendOnly` for XR camera/player pose sync and flat native camera pose
  sync.
- [x] Keep immediate drain only where synchronous feedback is genuinely required,
  such as startup/bootstrap or narrow commands whose caller depends on the
  result in the same function.
- [x] Preserve remote dedicated behavior initially because changing it requires a
  wire-protocol change; document that as temporary.
- [x] Add unit coverage proving `SendOnly` does not call `drain_updates`.
- [ ] Re-run the Quest RD7 settled-orbit lane. Success means the
  `commit_server_command` / locomotion-command update-apply tail collapses or
  moves into runtime poll attribution.

Expected result: the pose/movement path becomes cheap and predictable. If the
same cost simply moves to `runtime_poll`, continue immediately to Slice 2.

Recorded result:

- Added `GameplayCommandUpdatePolicy::{DrainImmediately, SendOnly}` in shared
  app runtime. Existing `send_gameplay_command*` helpers still use
  `DrainImmediately`, so old synchronous behavior remains the default.
- Local integrated `SendOnly` sends the `ClientCommand` to the integrated
  server runner without calling `drain_updates` or applying pending
  `ServerUpdate`s.
- XR camera/player pose sync and flat native camera pose sync now use
  `SendOnly`.
- Correction acknowledgement/resync, startup/bootstrap, explicit chunk-view
  changes, interactions, and remote dedicated command exchange keep immediate
  behavior for now.
- Remote dedicated ignores the policy temporarily because the current native
  TCP/WebSocket transport is still request/response-shaped. Slice 4/Slice 6
  track the real session actor and server-push broadening needed to remove
  that limitation.
- Added regression coverage:
  `local_send_only_command_defers_update_application_until_poll`.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime local_send_only_command_defers_update_application_until_poll -- --nocapture`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit perf has not been rerun yet for this slice.

### Slice 2 - Runtime Update Pump Budget

Goal: server updates are applied at one owned frame point with priority and
budget.

- Add a pending inbound update queue in the runtime if the drain point can
  produce more updates than the frame should apply.
- Classify updates by coarse priority:
  - critical ordered: disconnect/session/corrections,
  - gameplay ordered,
  - realtime superseding entity/player transforms,
  - world streaming chunks/unloads,
  - section/block mutations.
- Apply critical and small gameplay updates promptly.
- Apply chunk snapshots/unloads under an explicit per-frame budget.
- Preserve per-chunk ordering where chunk snapshot/unload order matters.
- Add diagnostics for drained, queued, applied, deferred, and coalesced updates
  by class.
- Measure the same Quest RD7 lane with actors enabled.

Success means worst-frame snapshots no longer show multi-ms update apply or
dirty marking hidden inside locomotion, and runtime update work is bounded and
attributed.

### Slice 3 - Dirty Marking Cost Units

Goal: make world streaming budget match render work rather than raw update
count.

- Split dirty marking attribution by update kind.
- Count chunk-neighborhood dirty marks, section dirty marks, and render-section
  keys touched.
- Consider applying chunk snapshot/unload dirty marking incrementally if a
  single update can still mark too much.
- Feed the dirty-to-drawable coordination work in `128`, not a separate
  renderer shortcut.

Success means a budget of K updates cannot still hide unbounded dirty-set churn.

### Slice 4 - Native Remote TCP Session Actor

Goal: make native remote dedicated follow the same client bus shape.

- Move `TcpStream` ownership into a dedicated network/session thread.
- App/runtime enqueues commands and drains decoded updates through the shared
  bus.
- Keep blocking socket IO inside the network thread for the first pass.
- Keep the existing TCP frame codec initially.
- Decide whether the dedicated server needs a server-push wire mode before
  remote clients can receive updates without sending commands.

Success means desktop/Android native remote paths no longer block the app frame
inside command exchange.

### Slice 5 - Web Bus Convergence

Goal: preserve browser async mechanics while sharing the same policy.

- WebSocket `message` callbacks push decoded update frames to the inbound queue.
- Worker-integrated local play publishes updates through the same policy.
- Browser app drains updates in the normal runtime frame/update pump.
- Keep JS promises, `web_sys::WebSocket`, worker startup, and browser lifecycle
  in the web app crate.

Success means web-specific async remains a transport adapter detail, not a
different engine/runtime model.

### Slice 6 - Server-Push Protocol Broadening

Goal: make remote dedicated fully event/update-stream shaped.

- Extend native TCP and WebSocket server paths so updates can arrive without a
  paired command response.
- Keep reliable ordered delivery for core world/gameplay updates.
- Evaluate whether high-rate entity/player snapshots need a superseding lane
  or future WebRTC data channel.
- Preserve the logical `ClientCommand` / `ServerUpdate` protocol model.

This is not required for the first local integrated Quest fix, but the shared
bus should be designed so this does not require another client-runtime rewrite.

## Measurement Plan

Primary lane:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-settled-orbit \
  --perf-orbit-speed 4.3 \
  --perf-metrics \
  --wait-seconds 270 \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 7 \
  --day-time 6000 \
  --freeze-time
```

Track:

- Meta dropped frames,
- `app_work_avg_ms`, p95, and app-over-period,
- locomotion command total/send/drain/apply/dirty/client-apply,
- runtime poll drain/apply/dirty/client-apply,
- inbound update queue depth by class,
- chunk snapshot/unload counts applied and deferred,
- render dirty chunk/section counts before and after update pump.

## Guardrails

- Do not make native app/runtime code async just to match web.
- Do not keep a `send command -> wait for response -> apply response` API as the
  shared engine shape.
- Do not coalesce future predicted movement command records unless that lane's
  replay semantics explicitly allow it.
- Do not let chunk streaming starve player corrections, lifecycle events, or
  interaction feedback.
- Do not bypass the shared client replica and render dirty path for local
  integrated play.
- Do not solve Quest frame pacing with XR-only update semantics.
