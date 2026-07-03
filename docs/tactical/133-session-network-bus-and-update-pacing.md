# 133: Session Network Bus And Update Pacing

Status: active tracking doc; Slice 1 send-only high-frequency command policy
landed for local integrated play; direction revised 2026-07-03 to strict
receive-order update application (vanilla parity) with a frame-budget stall,
replacing the earlier priority-class plan; Slice 2 ordered update pump is next
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
`send_gameplay_command_timed` still does this on the app/render thread (fixed
for pose sync by Slice 1's `SendOnly` policy):

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

2026-07-03 revision: a review of the 1.17.1 client against the original plan
showed vanilla has **no** client-side priority classes and **no** apply budget.
The client drains every queued packet task each frame in receive order
(`Minecraft.runTick` -> `runAllTasks()`), and that model works because packet
apply is thin (parse plus a resident-slot dirty boolean) and all expensive
mesh/upload work is pull-based behind bounded pools. Our measured spike is not
"too many updates per frame" in the vanilla sense — it is a fat apply step:
ordered-set dirty-mark churn (~0.13 ms per chunk-scale update) plus payload
decode on the render thread. The plan below targets the vanilla shape: keep
strict receive-order application, make apply thin, and add only a per-frame
budget stall as the recorded divergence. The priority/coalescing taxonomy is
parked for remote high-rate entity streams (Slice 6) and removed from the
local-play plan.

The target architecture is documented in
[`../session-network-architecture.md`](../session-network-architecture.md),
including the parity-versus-divergence record. This tactical tracks the
implementation slices.

## Desired Direction

Replace request/response-style session helpers with a shared client session bus:

```text
outbound ClientCommand stream
inbound ServerUpdate stream, decoded producer-side, strict receive order
runtime update pump applying in order under a per-frame budget stall
```

The runtime remains frame-driven. Network IO, local integrated server runner
bridging, WebSocket callbacks, and future WebRTC data channels feed the same
ordered queue behind a common policy boundary.

## Related Work

| Doc | Relationship |
| --- | --- |
| `062-shared-threading-topology.md` | Parent native-thread / web-worker topology. This tactical narrows the client/session bus part. |
| `085-web-host-mode-convergence.md` | Existing shared host-mode policy and web async adapter split. This tactical replaces request/response semantics without forcing native async. |
| `095-shared-session-coordinator.md` | Session lifecycle state remains separate from transport IO and update pacing. |
| `119-android-xr-live-streaming-frame-pacing.md` | Product-lane RD7/RD10 burst evidence and pacing goals. |
| `128-terrain-render-pipeline-coordination.md` | Terrain dirty-to-drawable bus that consumes update-driven dirty work after this pump. Its bounded compile/upload pipeline is the pull-based backpressure the vanilla model relies on, and its resident-lifecycle direction is where Slice 3's thin dirty marking lands. |
| `130-quest-thread-scheduling-and-streaming-tail-attribution.md` | Measured command split proving send was cheap and update apply/dirty marking was the tail. |
| `131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md` | Finding 3 records the dirty-mark cost diagnosis (ordered-set churn versus vanilla's resident-slot boolean), the K≈4 pacing suggestion, and the interest-hysteresis check adopted by Slice 3. |

## Current Code Facts

- `NativeIntegratedServerRunner` already has a command channel and update
  channel.
- `NativeIntegratedServerRunner::drain_updates` drains the whole update channel
  when asked and decodes every update frame on the calling (app/render)
  thread.
- `send_gameplay_command*` helpers default to `DrainImmediately`; Slice 1 added
  `SendOnly` and switched pose sync to it.
- `LocalSingleViewSceneRuntime::set_chunk_view` still sends the view command
  and immediately drains/applies all pending updates on the caller thread.
  After Slice 1 this is the last opportunistic drain in the movement/interest
  path, and it fires exactly on chunk-boundary crossings where bursts exist.
- `LocalSingleViewSceneRuntime::poll` drains/applies updates and runs every
  frame in all three client lanes (desktop `poll_runtime`, XR
  `poll_runtime_and_upload`, perf harness), so the owned pump point exists.
- `poll_until_idle` treats runner idle plus `update_queue_depth == 0` as done.
  Slice 2 must keep undrained updates observable (leave them in the channel)
  so idle and startup/bootstrap semantics stay correct.
- Native remote TCP still blocks the caller in `NativeClientSession::send_command`
  while reading one response batch, and it ignores the `SendOnly` policy
  (documented temporary limitation until Slice 4).
- Web remote WebSocket is event-driven underneath, but the Rust-facing session
  is still shaped as command exchange.

## Slices

### Slice 0 - Documentation Baseline

- [x] Add the durable session/network architecture document.
- [x] Add this tactical tracking doc.
- [x] Keep `protocol.md` clear that request/response is current transport
  behavior, not the long-term session model.
- [x] 2026-07-03: revise the architecture doc and this tactical to the
  vanilla ordered-stream model; record parity versus divergence explicitly.

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
Also expect part of the old tail to reappear under `commit_interest`
attribution on chunk-boundary frames until Slice 2 moves the `set_chunk_view`
drain into the pump.

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
- Behavior note: position corrections triggered by a `SendOnly` pose are now
  detected at the next runtime poll instead of inside the send call — one
  frame later, which matches the vanilla client-thread latency.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime local_send_only_command_defers_update_application_until_poll -- --nocapture`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit perf has not been rerun yet for this slice.

### Slice 2 - Ordered Update Pump With Frame Budget

Goal: server updates apply at the owned per-frame pump, in strict receive
order, under an explicit budget; a burst stalls the queue across frames
instead of blowing one frame.

- [ ] Re-run the Quest RD7 settled-orbit lane on Slice 1 first (the unchecked
  item above) to baseline where the cost moved before changing the pump.
- [ ] Pump loop: while budget remains, receive the next update frame from the
  channel, decode, apply; stop when the budget is exhausted; always apply at
  least one pending update per pump so progress is guaranteed. Undrained
  frames stay in the channel so `update_queue_depth`, `poll_until_idle`, and
  bootstrap semantics keep working. Do not add a second runtime-side pending
  queue.
- [ ] Budget definition: elapsed-time budget checked between updates (default
  ~2 ms, to be measured), with an unlimited setting for loading/bootstrap
  states and tests. A count-based budget (K≈4 chunk-scale updates per frame,
  per `131` Finding 3.1) is an acceptable first cut, but time is the target
  because update apply costs are heterogeneous.
- [ ] Remove the immediate drain from `set_chunk_view`; view changes become
  send-only and their resulting updates arrive through the pump.
- [ ] Surface (one-time log or diagnostics flag) when a `SendOnly` command is
  sent on a transport that ignores the policy (remote dedicated until
  Slice 4), so the asymmetry is visible instead of silent.
- [ ] Diagnostics per pump: applied count, remaining queue depth, oldest
  queued update age, bytes queued, stall occurrences, and
  drain/decode/apply/dirty-mark time.
- [ ] Tests: receive order preserved when a stall splits a
  snapshot -> section-mutation -> unload sequence for one chunk across
  frames; correction round-trip under `SendOnly` completes within a bounded
  number of pump passes; a single over-budget update still applies (progress
  guarantee); `poll_until_idle` still drains fully.
- [ ] Measure the same Quest RD7 lane with actors enabled.

Success means worst-frame snapshots no longer show multi-ms update apply or
dirty marking hidden inside locomotion or a single pump pass, runtime update
work is bounded and attributed, and no update is ever applied out of receive
order.

### Slice 3 - Thin Apply And Source Pacing

Goal: make the apply step vanilla-thin so the budget rarely stalls, and stop
burst churn at the source. This is the actual fix; the Slice 2 budget is the
safety valve.

- Resident-slot dirty marking: flip state on resident render-section slots
  (vanilla `ViewArea.setDirty` shape — array index plus boolean store) instead
  of ordered-set insertion per update. This is the `131` Finding 3.3 diagnosis
  and should land as part of, or aligned with, `128`'s resident-lifecycle
  direction — not a separate renderer shortcut.
- Move update payload decode off the render thread: the integrated runner
  publishes decoded updates; remote transports decode on the session thread
  (Slice 4).
- Interest hysteresis: verify the unload margin is wider than the load margin
  so a path oscillating near a chunk boundary does not thrash full row swaps
  (the measured 15-snapshot + 15-unload bursts). Vanilla keeps chunks resident
  beyond the strict view distance and unloads lazily.
- Keep apply attribution split by update kind (chunk snapshot, section
  mutation, unload) so per-update cost stays measurable.

Success means a chunk-scale update applies at parse-plus-flag-flip cost (well
below the current ~0.13 ms), the default budget absorbs a 15+15 burst within a
frame or two of stalling at most, and boundary oscillation no longer reloads
and unloads the same chunk row.

### Slice 4 - Native Remote TCP Session Actor

Goal: make native remote dedicated follow the same client bus shape.

- Move `TcpStream` ownership into a dedicated network/session thread; that
  thread owns payload decode and pushes decoded updates to the inbound queue
  in receive order.
- App/runtime enqueues commands and drains decoded updates through the shared
  bus.
- Keep blocking socket IO inside the network thread for the first pass.
- Keep the existing TCP frame codec initially.
- Remove the temporary Slice 1 limitation where remote dedicated ignores the
  `SendOnly` policy.
- Decide whether the dedicated server needs a server-push wire mode before
  remote clients can receive updates without sending commands.

Success means desktop/Android native remote paths no longer block the app frame
inside command exchange.

### Slice 5 - Web Bus Convergence

Goal: preserve browser async mechanics while sharing the same policy.

- WebSocket `message` callbacks push decoded updates to the inbound queue in
  receive order.
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
- Evaluate whether high-rate entity/player snapshots need a
  superseding/coalescing lane or a future WebRTC data channel. This is where
  the parked priority taxonomy may return; any coalescing lane must define
  spawn/despawn ordering rules first, and it remains remote-only — local play
  keeps the strict ordered stream.
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
- locomotion command total/send (drain/apply should stay zero after Slice 1),
- runtime poll drain/decode/apply/dirty/client-apply,
- inbound update queue depth, oldest queued update age, and bytes queued,
- updates applied versus deferred per pump, and stall occurrences,
- chunk snapshot/unload counts applied and deferred,
- render dirty chunk/section counts before and after the update pump.

## Guardrails

- Do not make native app/runtime code async just to match web.
- Do not keep a `send command -> wait for response -> apply response` API as the
  shared engine shape.
- Do not reorder or coalesce inbound updates for local play. The only
  divergence from vanilla's drain-all model is the budget stall, which changes
  when updates apply, never their order or content. A critical fast-lane for
  corrections/lifecycle is the recorded escalation path and requires measured
  correction-latency evidence first.
- Do not coalesce future predicted movement command records unless that lane's
  replay semantics explicitly allow it.
- Keep the apply step thin. New per-update work at the pump (payload decode,
  allocation, ordered-set churn) is a regression smell; heavy work belongs in
  the pull-based terrain pipeline (`128`).
- Do not let the pump silently stop running in any client lane; queue age
  diagnostics exist so starvation is detectable.
- Do not bypass the shared client replica and render dirty path for local
  integrated play.
- Do not solve Quest frame pacing with XR-only update semantics.
