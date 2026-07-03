# 133: Session Network Bus And Update Pacing

Status: active tracking doc; Slice 1 send-only high-frequency command policy
landed for local integrated play; direction revised 2026-07-03 to strict
receive-order update application (vanilla parity) with a frame-budget stall,
replacing the earlier priority-class plan; Slice 2 local integrated ordered
update pump landed; Slice 3A batched dirty intent and resident cache lookup
landed; Slice 3B local integrated producer-side decoded update queue landed;
Slice 3C local integrated chunk-interest unload hysteresis landed; Slice 3D
resident cached-section dirty flags landed; remote/web bus convergence and
broader terrain coordinator lifecycle remain active
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
- `NativeIntegratedServerRunner::try_recv_update` receives one decoded update
  envelope at a time and preserves queue depth/byte accounting. The runner
  still uses the shared update codec on the server thread to compute
  `encoded_len` for diagnostics, but the runtime pump no longer decodes local
  integrated update payloads. `drain_updates` remains as an unlimited
  compatibility helper.
- Local integrated play uses the shared player-chunk tracking policy with a
  one-chunk unload hysteresis margin for Java-shaped radii. The first view is
  exact, tiny debug views stay exact, and chunks just outside the accepted view
  can remain retained across boundary oscillation to avoid immediate unload /
  reload churn.
- `send_gameplay_command*` helpers default to `DrainImmediately`; Slice 1 added
  `SendOnly` and switched pose sync to it.
- `LocalSingleViewSceneRuntime::set_chunk_view` sends the view command only;
  resulting snapshots/unloads arrive through the runtime pump.
- `LocalSingleViewSceneRuntime::poll` applies updates in strict receive order
  under the default elapsed-time budget, while startup and `poll_until_idle`
  use an explicit unlimited pump.
- `EngineRenderSession::mark_server_update_render_dirty` now coalesces each
  server-update batch into unique dirty chunk neighborhoods and dirty section
  keys before touching render dirty state. `CachedTexturedRenderSections`
  also keeps a resident chunk-to-section-key index and per-section dirty flags,
  so resident dirty marking no longer scans all cached section keys or inserts
  resident keys into `dirty_sections`.
- Render dirty state still has `BTreeSet` fallback paths for nonresident/new
  snapshot chunks, removals, stale/nonresident sections, and
  `inflight_sections`. Resident cached sections use slot-local dirty booleans.
- `poll_until_idle` treats runner idle plus `update_queue_depth == 0` as done.
  Slice 2 must keep undrained updates observable (leave them in the channel)
  so idle and startup/bootstrap semantics stay correct.
- Native remote TCP still blocks the caller in `NativeClientSession::send_command`
  while reading one response batch, and it ignores the `SendOnly` policy. A
  one-time warning now surfaces that temporary limitation until Slice 4.
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
- A pure Slice 1 Quest RD7 baseline was not captured before Slice 2 landed.
  The post-Slice 2 Quest run below confirms the locomotion command drain/apply
  tail stayed collapsed.

### Slice 2 - Ordered Update Pump With Frame Budget

Goal: server updates apply at the owned per-frame pump, in strict receive
order, under an explicit budget; a burst stalls the queue across frames
instead of blowing one frame.

- [ ] Re-run the Quest RD7 settled-orbit lane on Slice 1 first (the unchecked
  item above) to baseline where the cost moved before changing the pump.
  Superseded in practice by the post-Slice 2 run recorded below.
- [x] Pump loop: while budget remains, receive the next update frame from the
  channel, decode, apply; stop when the budget is exhausted; always apply at
  least one pending update per pump so progress is guaranteed. Undrained
  frames stay in the channel so `update_queue_depth`, `poll_until_idle`, and
  bootstrap semantics keep working. Do not add a second runtime-side pending
  queue.
- [x] Budget definition: elapsed-time budget checked between updates (default
  ~2 ms, to be measured), with an unlimited setting for loading/bootstrap
  states and tests. A count-based budget (K≈4 chunk-scale updates per frame,
  per `131` Finding 3.1) is an acceptable first cut, but time is the target
  because update apply costs are heterogeneous.
- [x] Remove the immediate drain from `set_chunk_view`; view changes become
  send-only and their resulting updates arrive through the pump.
- [x] Surface (one-time log or diagnostics flag) when a `SendOnly` command is
  sent on a transport that ignores the policy (remote dedicated until
  Slice 4), so the asymmetry is visible instead of silent.
- [x] Diagnostics per pump: applied count, remaining queue depth, bytes queued,
  bytes applied, oldest applied update age, stall occurrences, and
  drain/decode/apply/dirty-mark time.
- [ ] Exact oldest pending update age. The current mpsc-backed runner exposes
  applied update age and queued bytes/depth, but cannot peek the oldest pending
  frame without a transport-side metadata queue.
- [x] Tests: receive order preserved when a stall splits a
  snapshot -> section-mutation -> unload sequence for one chunk across
  frames; a single over-budget update still applies (progress guarantee);
  `set_chunk_view` defers updates until poll; `poll_until_idle` still drains
  fully.
- [ ] Correction round-trip under `SendOnly` completes within a bounded number
  of pump passes.
- [x] Measure the same Quest RD7 lane with actors enabled.

Recorded result:

- Added `RuntimeUpdatePumpBudget::{MaxElapsed, Unlimited}` and local
  integrated polling via `poll_with_update_budget`.
- Added `NativeIntegratedServerRunner::try_recv_update`, per-frame queue byte
  accounting, queued-age reporting for applied updates, and a full-drain helper
  built from the same single-frame receive path.
- `LocalSingleViewStartupPump` and `poll_until_idle` use the unlimited budget;
  normal frame polling uses the default elapsed-time budget and always applies
  at least one pending update.
- `LocalSingleViewSceneRuntime::set_chunk_view` no longer drains or applies
  updates immediately.
- Remote dedicated logs once when `SendOnly` is requested on the still
  request/response-shaped transport.
- Diagnostics now expose update-pump stalls, queued bytes, applied bytes,
  oldest applied update age, and the existing drain/apply/dirty/client timing
  in flat perf and Android XR summaries.
- Added regression coverage:
  `split_update_application_preserves_snapshot_mutation_unload_order`,
  `local_set_chunk_view_defers_update_application_until_poll`, and
  `local_update_pump_applies_one_update_when_budget_is_exhausted`.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-server -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- `cargo test --manifest-path native/Cargo.toml` passed.
- Quest RD7 settled-orbit lane passed with actors enabled:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
  Summary artifacts:
  `/tmp/mclone-quest-openxr-perf-summary.txt` and
  `/tmp/mclone-quest-openxr-logcat.txt`.
- Quest result highlights:
  - `MCLONE_ANDROID_XR_PERF_LOCOMOTION_COMMAND`:
    `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`,
    `max_updates=0`.
  - `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX`:
    `poll_total_ms=3.439`, `drain_updates_ms=0.712`,
    `apply_updates_ms=3.425`, `dirty_mark_ms=1.963`,
    `client_apply_ms=3.402`, `updates=20`.
  - `MCLONE_ANDROID_XR_PERF_UPLOAD_MAX`:
    `update_pump_stalled=true`, `server_update_queue_depth=27`,
    `server_update_queue_bytes=893140`,
    `server_update_applied_bytes=748000`,
    `server_update_oldest_applied_age_ms=54.119`.
  - Overall frame pacing remains over budget:
    `app_work_avg_ms=14.281`, `app_work_p95_ms=16.478`,
    `headroom_avg_ms=-0.392`, `app_over_period_pct=57.8`.

Success means worst-frame snapshots no longer show multi-ms update apply or
dirty marking hidden inside locomotion or a single pump pass, runtime update
work is bounded and attributed, and no update is ever applied out of receive
order.

Slice 2 conclusion: the locomotion-command tail is gone and the update burst is
now visible under runtime poll, with queue stalls recorded. Runtime apply is
still too fat for Quest: a single pump pass can exceed the nominal budget
because the budget is checked between updates and per-update dirty/client apply
cost remains high. Continue directly to Slice 3.

### Slice 3 - Thin Apply And Source Pacing

Goal: make the apply step vanilla-thin so the budget rarely stalls, and stop
burst churn at the source. This is the actual fix; the Slice 2 budget is the
safety valve.

- [x] Slice 3A: batch server-update dirty intent before marking render state.
  Chunk snapshots/unloads expand to unique dirty chunk neighborhoods once per
  update frame, and section-block updates expand to unique dirty section keys
  once per update frame. Duplicate updates no longer multiply neighborhood
  fanout or section-revision bumps inside one pump application.
- [x] Slice 3A: maintain a resident chunk-to-section-key index in
  `CachedTexturedRenderSections`, and use it when collecting known resident
  keys for a dirty chunk. This removes the per-dirty-chunk full-cache scan
  while preserving the existing compile planner and cache lifecycle.
- [x] Slice 3D: resident-slot dirty marking for cached render sections. Flip
  state on resident render-section slots (vanilla `ViewArea.setDirty` shape:
  resident slot plus boolean) instead of inserting resident keys into
  `dirty_sections` per update. Keep fallback dirty sets for new snapshots before
  replica install, removals, nonresident/stale sections, and inflight tracking.
- [x] Slice 3B: move local integrated update payload decode off the render
  thread. The native runner queues decoded `ServerUpdate` envelopes and keeps
  encoded byte accounting as producer-side metadata.
- [ ] Move remote transport update payload decode off the render thread:
  remote transports decode on the session thread (Slice 4).
- [x] Interest hysteresis: verify the unload margin is wider than the load margin
  so a path oscillating near a chunk boundary does not thrash full row swaps
  (the measured 15-snapshot + 15-unload bursts). Vanilla keeps chunks resident
  beyond the strict view distance and unloads lazily.
- [ ] Keep apply attribution split by update kind (chunk snapshot, section
  mutation, unload) so per-update cost stays measurable.

Recorded Slice 3A result:

- Added `EngineServerUpdateDirtyBatch` so `mark_server_update_render_dirty`
  classifies a received update frame once, then applies unique dirty chunk and
  section marks through the existing render-session dirty contract.
- Added `CachedTexturedRenderSections::section_keys_by_chunk` plus indexed
  `contains_chunk` / `section_keys_for_chunk` resident lookups. Cache updates
  keep the index in sync during rebuild insertion and section/chunk removal.
- Added regression coverage for duplicate snapshot dirtying, duplicate
  section-update dirtying, and resident cache index removal.

Validation (Slice 3A):

- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit lane passed with actors enabled:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
  Summary artifacts:
  `/tmp/mclone-quest-openxr-perf-summary.txt` and
  `/tmp/mclone-quest-openxr-logcat.txt`.
- Quest result highlights compared with the post-Slice 2 run:
  - Locomotion command update drain remained collapsed:
    `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`,
    `max_updates=0`.
  - Runtime dirty marking improved but the pump is still too fat:
    `dirty_mark_ms=1.237` versus `1.963`, `client_apply_ms=3.184`
    versus `3.402`, with `updates=36`.
  - Runtime queue pressure improved:
    `server_update_queue_depth=13` versus `27`,
    `server_update_queue_bytes=319782` versus `893140`,
    `server_update_oldest_applied_age_ms=32.234` versus `54.119`.
  - Overall frame pacing improved but remains marginal:
    `app_work_avg_ms=13.788`, `app_work_p95_ms=15.803`,
    `headroom_avg_ms=0.101`, `app_over_period_pct=42.4`.

Recorded Slice 3B result:

- `NativeQueuedServerUpdate` now stores a decoded `ServerUpdate` plus
  `encoded_len`/`queued_at` metadata instead of an encoded frame. The native
  runner computes encoded length on the producer side for queue-byte
  diagnostics, then moves the owned update into the queue.
- `NativeIntegratedServerRunner::try_recv_update` no longer calls
  `decode_server_update`; it only updates queue counters and returns the
  decoded envelope.
- Simulation, physics, and command-produced update vectors are moved into the
  queue instead of cloned through a caller-side decode path.
- Added regression coverage:
  `native_runner_publish_updates_queues_decoded_updates_with_encoded_byte_accounting`.

Validation (Slice 3B):

- `cargo test --manifest-path native/Cargo.toml -p mclone-server -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit lane passed with actors enabled:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
  Summary artifacts:
  `/tmp/mclone-quest-openxr-perf-summary.txt` and
  `/tmp/mclone-quest-openxr-logcat.txt`.
- Quest result highlights compared with the post-Slice 3A run:
  - Locomotion command update drain remained collapsed:
    `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`,
    `max_updates=0`.
  - Runtime receive/decode drain tail collapsed:
    `drain_updates_ms=0.094` versus `8.294`, and
    `poll_total_ms=3.263` versus `8.320`.
  - Apply remains dominated by client state mutation and dirty marking:
    `apply_updates_ms=3.253`, `dirty_mark_ms=1.231`,
    `client_apply_ms=3.232`, with `updates=37`.
  - Queue pressure improved again:
    `server_update_queue_depth=3` versus `13`,
    `server_update_queue_bytes=191` versus `319782`,
    `server_update_oldest_applied_age_ms=31.586` versus `32.234`.
  - Overall frame pacing improved slightly but remains marginal:
    `app_work_avg_ms=13.751`, `app_work_p95_ms=15.864`,
    `headroom_avg_ms=0.138`, `app_over_period_pct=41.1`.

Recorded Slice 3C result:

- `PlayerChunkTrackingPolicy` now has an explicit
  `unload_hysteresis_chunks` knob. The default dedicated policy keeps exact
  view-diff behavior, while `NativeIntegratedServerRunnerConfig` opts local
  integrated play into a one-chunk unload margin on top of the Java-max policy.
- Hysteresis applies only for normal Java-shaped radii
  (`chunk_tracking_radius >= 3`). Radius 0/1 debug and smoke views still unload
  exactly, preserving existing narrow tests and capture lanes.
- The accepted view remains the strict requested/clamped target. The
  per-player `visible_chunks` set now means "chunks the player may still hold
  locally"; under hysteresis it can retain chunks just outside the accepted
  target, so aggregate player tickets keep that trailing ring resident.
- Added regression coverage:
  `unload_hysteresis_keeps_tiny_debug_views_exact`,
  `unload_hysteresis_retains_chunks_inside_unload_margin`, and an integrated
  runner config assertion that local integrated tracking enables the one-chunk
  margin.

Validation (Slice 3C):

- `cargo test --manifest-path native/Cargo.toml -p mclone-server -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit lane passed with actors enabled:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
  Summary artifacts:
  `/tmp/mclone-quest-openxr-perf-summary.txt` and
  `/tmp/mclone-quest-openxr-logcat.txt`.
- Quest result highlights compared with the post-Slice 3B run:
  - Locomotion command update drain remained collapsed:
    `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`,
    `max_updates=0`.
  - Runtime pump tail improved again:
    `poll_total_ms=1.722` versus `3.263`,
    `drain_updates_ms=0.050` versus `0.094`,
    `apply_updates_ms=1.712` versus `3.253`,
    `client_apply_ms=1.690` versus `3.232`.
    `dirty_mark_ms=1.299` remains the resident-slot dirty-marking item.
  - Queue pressure cleared on this lane:
    `update_pump_stalled=false`, `server_update_queue_depth=0`,
    `server_update_queue_bytes=0`,
    `server_update_oldest_applied_age_ms=22.503`.
  - Source-side retention is visible in diagnostics:
    `player_visible_chunks=256` for the RD7 settled-orbit path, while the
    strict accepted target remains the Java-shaped view radius.
  - Overall frame pacing is still not solved and was noisier/worse in this
    run:
    `app_work_avg_ms=14.816`, `app_work_p95_ms=17.560`,
    `headroom_avg_ms=-0.927`, `app_over_period_pct=65.6`. The worst frames
    are now dominated by terrain render/upload/poll-wait work rather than a
    hidden command-drain path.

Recorded Slice 3D result:

- `CachedTexturedRenderSections` now stores resident section slots with mesh
  payload plus a dirty flag. Resident snapshot and section-block-update dirty
  marks flip that slot flag instead of inserting the section key into
  `RenderSectionDirtyState::dirty_sections`.
- Dirty work classification merges resident dirty flags with fallback dirty
  chunks/sections, and ready-plan application clears accepted resident flags
  while keeping deferred resident sections dirty in place.
- Forced dirty chunks preserve new snapshot behavior when a snapshot dirty mark
  arrives before the client replica installs the chunk. Unknown neighborhood
  chunks without resident cache entries no longer seed empty dirty work.
- Runtime diagnostics count resident dirty flags alongside fallback dirty sets
  so existing pending-work counters remain comparable.
- Added regression coverage:
  `engine_render_session_marks_resident_snapshot_sections_dirty_without_dirty_chunk_set`,
  `engine_render_session_marks_resident_section_updates_dirty_without_dirty_section_set`,
  and the remote-session reconnect resync path that depends on forced snapshot
  chunk dirtying.

Validation (Slice 3D):

- `cargo test --manifest-path native/Cargo.toml -p mclone-render-session -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Quest RD7 settled-orbit lane passed with actors enabled:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
  Summary artifacts:
  `/tmp/mclone-quest-openxr-perf-summary.txt` and
  `/tmp/mclone-quest-openxr-logcat.txt`.
- Quest result highlights compared with the post-Slice 3C run:
  - Resident dirty marking collapsed the measured runtime dirty-seed tail:
    `max_runtime_dirty_seed_ms=0.001`, down from the prior
    `dirty_mark_ms=1.299` item.
  - Runtime apply is thinner but not free:
    `apply_updates_ms=1.998`, `client_apply_ms=1.514`,
    `poll_total_ms=2.026`, with `updates=31`.
  - Source/update pressure is mostly controlled but not zero:
    `update_pump_stalled=true` once, `server_update_queue_depth=9`,
    `server_update_queue_bytes=483148`,
    `server_update_oldest_applied_age_ms=34.689`.
  - Overall frame pacing remains unsolved and is now dominated by terrain
    render/upload/poll-wait and draw work:
    `app_work_avg_ms=15.070`, `app_work_p95_ms=17.759`,
    `headroom_avg_ms=-1.181`, `app_over_period_pct=75.3`,
    `submitted_fps=65.34`.

Slice 3 conclusion: the local integrated update pump now follows the intended
thin-apply shape for resident dirty marking: producer-side decode, strict
receive-order application, resident flag flips, and bounded compile/upload
pull-work. Remaining frame misses are no longer explained by hidden command
drain, render-thread decode, full-cache dirty scans, or resident dirty-set
insertion. The next local Quest work should move back to the terrain
coordinator/render cost tracks (`128`, `119`, `117`), while this tactical's
remaining bus work is remote TCP and web convergence.

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

## Closeout Notes / Deferred Valuable Work

Before closing this tactical or moving the local integrated pacing work out to
another doc, explicitly decide where these remaining valuable items live:

- Terrain coordinator lifecycle follow-up: resident cached-section dirty flags
  are in place, but fallback dirty sets, `inflight_sections`, deferred
  ready-plan materialization, compiler handoff, and upload/publication
  ownership remain valuable `128` work before the dirty-to-drawable pipeline is
  truly Java-shaped.
- Exact oldest pending update age: current diagnostics expose applied update
  age and queued bytes/depth, but the mpsc-backed runner still cannot peek the
  oldest pending update without a transport-side metadata queue.
- Remote TCP session actor: native remote still needs a real inbound update
  bus and producer-side decode so `SendOnly` works outside local integrated
  play.
- Web bus convergence: browser WebSocket/worker callbacks should feed the same
  ordered inbound queue behind the shared policy.
- Correction latency fast-lane: keep this only as the recorded escalation path
  if measured queue age proves corrections/lifecycle updates need it. Do not
  introduce chunk-stream priority classes for local play.

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
