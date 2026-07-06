# 133: Session Network Bus And Update Pacing

Status: active tracking doc; Slice 1 send-only high-frequency command policy
landed for local integrated play; direction revised 2026-07-03 to strict
receive-order update application (vanilla parity) with a frame-budget stall,
replacing the earlier priority-class plan; Slice 2 local integrated ordered
update pump landed; Slice 3A batched dirty intent and resident cache lookup
landed; Slice 3B local integrated producer-side decoded update queue landed;
Slice 3C local integrated chunk-interest unload hysteresis landed; Slice 3D
resident cached-section dirty flags landed; Slice 3E local chunk-view churn
validation landed; Slice 3F Quest-controlled chunk-view churn validation
landed and reproduced the unload client-apply tail; Slice 3G default
unload-count pump cap landed and reduced the Quest unload tail but did not
eliminate it; Slice 3H client entity-by-chunk unload index landed and cut the
Quest update-pump tail below 4 ms; Slice 3I batched packed-section patching
landed and improved the section-block client tail; Slice 3J deferred client
chunk snapshot payload drops landed and removed the Quest unload client-apply
tail while making cleanup cost explicit in poll diagnostics; Slice 3K native
deferred payload drop worker landed and moved that cleanup off the app frame;
remote `SendOnly` and response-readiness fixes landed under tactical 149;
focused native remote inbound-queue work is split to tactical 151; web bus
convergence and broader terrain coordinator lifecycle remain active
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
- The local `NativeSingleViewSessionRuntime::set_chunk_view` wrapper now
  preserves that deferred-update local path instead of routing through the
  default immediate-drain gameplay helper. Remote dedicated now honors
  `SendOnly` too, but still has a paired response-batch transport shape.
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
- Native remote TCP no longer drains remote `SendOnly` responses inside the
  command send path, and normal remote `poll()` first checks TCP readiness
  before draining. Once a response batch is ready, the runtime thread still
  reads and decodes the whole batch. Tactical 151 owns moving that ready-batch
  read/decode to a client IO actor and inbound queue.
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
  changes, interactions, and remote dedicated command exchange kept immediate
  behavior at the time this slice first landed.
- Historical note: remote dedicated ignored the policy temporarily because the
  native TCP/WebSocket transport was still request/response-shaped. Tactical
  149 later removed the native remote `SendOnly` limitation and added
  TCP-readiness polling. Tactical 151 owns the remaining native remote
  ready-batch read/decode split; Slice 6 below still owns future server-push
  broadening.
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
  sent on a transport that cannot honor the policy, so the asymmetry is visible
  instead of silent. Historical remote dedicated limitation removed by
  tactical 149.
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
- Remote dedicated originally logged once when `SendOnly` was requested on the
  still request/response-shaped transport. Tactical 149 later replaced that
  limitation with split send/drain behavior and readiness-aware remote polling.
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
- [x] Keep apply attribution split by update kind (chunk snapshot, section
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

Recorded Slice 3E result:

- Added deterministic local chunk-view churn validation:
  `local_chunk_view_churn_attributes_unload_apply`.
- The test uses the real local integrated server runner and runtime update pump:
  start with a render-distance-1 chunk window, jump the chunk view far enough to
  eliminate overlap, pump until the runner is idle, and aggregate the normal
  per-poll update-apply diagnostics.
- The validation asserts that the old 3x3 chunk window is fully unloaded, the
  new center is loaded, unload updates are counted, mixed update batches are not
  produced, and the unload timing category is populated.
- The churn path also produced section block updates in the focused run, so the
  useful invariant is not "unload-only"; it is "unloads stay separately
  attributed while other ordered updates may coexist."

Validation (Slice 3E):

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime local_chunk_view_churn_attributes_unload_apply -- --nocapture`
  passed.
- Focused local result:
  `polls=1085`, `changed_polls=17`, `snapshots=9`,
  `section_updates=21`, `unloads=9`, `other=35`,
  `unload_apply_ms=0.125`, `unload_dirty_ms=0.073`,
  `unload_client_ms=0.051`, `max_queue_depth_before_poll=11`.

### Slice 3F - Quest-Controlled Chunk-View Churn Lane

Goal: reproduce or clear the intermittent Quest unload/update-apply tail under a
controlled chunk-interest workload, without headset locomotion noise.

- [x] Fix the local `NativeSingleViewSessionRuntime::set_chunk_view` wrapper so
  XR/local session interest changes use the local send-only chunk-view path and
  leave resulting updates for the normal runtime pump.
- [x] Add Android XR startup/probe flags:
  `--perf-chunk-view-churn`, `--perf-churn-interval-seconds`, and
  `--perf-churn-offset-chunks`.
- [x] Add shared XR scene automation that keeps input stationary, then toggles
  the runtime interest center during the timed sample.
- [x] Add Quest validator support and marker checks for
  `mode=chunk-view-churn` plus `MCLONE_ANDROID_XR_CHUNK_VIEW_CHURN`.
- [x] Validate on headset with the current `2 / 16 / 64` measurement lane.

Recorded Slice 3F result:

- Quest chunk-view churn lane passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-churn-summary.txt --log /tmp/mclone-quest-openxr-churn-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
- The app settled first, then alternated chunk interest between `(0, 0)` and
  `(16, 0)` every 3 seconds:
  `MCLONE_ANDROID_XR_PERF_SETTLED mode=chunk-view-churn
  settle_seconds=35.365`, followed by repeated
  `MCLONE_ANDROID_XR_CHUNK_VIEW_CHURN` markers.
- Frame result:
  `sample_seconds=45.011`, `submitted_fps=70.36`, `runtime_fps=70.36`,
  `app_work_avg_ms=8.048`, `app_work_p95_ms=16.164`,
  `app_work_p99_ms=18.531`, `app_over_period_pct=12.2`,
  `frame_max_ms=41.432`.
- Update result:
  `max_runtime_poll_ms=20.654`, `apply_updates_ms=20.605`,
  `dirty_mark_ms=1.880`, `client_apply_ms=19.323`, `updates=254`,
  `unload_updates=204`, `server_update_queue_depth=418`,
  `server_update_queue_bytes=11151295`,
  `server_update_oldest_applied_age_ms=124.335`.
- Specific update attribution:
  `unload_ms=20.605`, `unload_dirty_ms=1.711`,
  `unload_client_ms=19.323`, with no mixed-update batches.
- Terrain pipeline pressure was also visible:
  `upload_limited=true`, `accept_limited=true`, `backpressured=true`,
  `deferred_sections=3328`, `ready_sections=2432`,
  `queued_upload_removed_sections=1920`.

Validation (Slice 3F):

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime native_single_view_session_runtime_local_interest_change_defers_updates_until_poll -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene --lib`
  passed.
- `cargo ndk -t arm64-v8a --platform 28 check --package mclone-android-xr-client --lib`
  passed.
- `bash -n android-xr/validate-quest-openxr.sh` passed.
- Quest validation command above passed and wrote:
  `/tmp/mclone-quest-openxr-churn-summary.txt` and
  `/tmp/mclone-quest-openxr-churn-logcat.txt`.

### Slice 3G - Default Unload Count Cap

Goal: make the frame update-pump budget meaningful for ordered unload bursts
without reordering updates or changing startup/bootstrap drains.

- [x] Add a default frame-pump cap of `16` chunk-unload updates in addition to
  the existing 2 ms elapsed budget.
- [x] Preserve `Unlimited` for startup/idle drains and preserve explicit
  `MaxElapsed` semantics for focused tests.
- [x] Keep receive order: the pump only stops between already ordered
  `ServerUpdate` records.

Recorded Slice 3G result:

- Quest chunk-view churn lane passed again with the same command shape, writing
  `/tmp/mclone-quest-openxr-churn-unload-cap-summary.txt` and
  `/tmp/mclone-quest-openxr-churn-unload-cap-logcat.txt`.
- The cap is visible in the summary maxima: `unload_updates` dropped from `204`
  in Slice 3F to `16`.
- The runtime/update max improved but still missed the 72 Hz budget:
  `max_runtime_poll_ms=8.694`, `apply_updates_ms=8.676`,
  `client_apply_ms=8.325`, down from `20.654`, `20.605`, and `19.323`.
- Overall frame pacing was roughly unchanged:
  `app_work_avg_ms=8.054`, `app_work_p95_ms=16.603`,
  `app_over_period_pct=13.1`, `frame_max_ms=35.774`.
- Queue age rose while the burst was spread across more frames:
  `server_update_queue_depth=414`,
  `server_update_oldest_applied_age_ms=235.992`.
- Terrain upload/ready pressure remained active:
  `upload_limited=true`, `accept_limited=true`, `backpressured=true`,
  `deferred_sections=3328`, `queued_upload_removed_sections=1408`,
  `max_runtime_upload_apply_ms=21.669`.

Validation (Slice 3G):

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo ndk -t arm64-v8a --platform 28 check --package mclone-android-xr-client --lib`
  passed.
- Quest validation command passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-churn-unload-cap-summary.txt --log /tmp/mclone-quest-openxr-churn-unload-cap-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.

### Slice 3H - Client Entity-By-Chunk Unload Index

Goal: remove the obvious per-unload client-replica scan while preserving the
same `ServerUpdate::ChunkUnload` semantics.

- [x] Add a `ClientRuntime` entity chunk index (`EntityId -> ChunkPos` and
  `ChunkPos -> EntityId set`) so chunk unload removes only entities in the
  unloaded chunk instead of retaining/scanning the entire entity map for every
  unload update.
- [x] Keep the index synchronized on entity snapshot insert/replace, entity
  movement updates, explicit entity removal, and replica clear.
- [x] Extend the client lifecycle test so an entity moved across a chunk
  boundary survives unloading the old chunk and is removed by unloading the new
  chunk.

Recorded Slice 3H result:

- Quest chunk-view churn lane passed again with the same command shape, writing
  `/tmp/mclone-quest-openxr-churn-entity-index-summary.txt` and
  `/tmp/mclone-quest-openxr-churn-entity-index-logcat.txt`.
- The capped update-pump max improved from Slice 3G's
  `max_runtime_poll_ms=8.694`, `apply_updates_ms=8.676`,
  `client_apply_ms=8.325` to `3.925`, `3.913`, and `3.882 ms`.
- The unload category improved but is not zero:
  `unload_ms=3.614`, `unload_dirty_ms=1.580`,
  `unload_client_ms=3.208`, with `unload_updates=16`.
- The max update category moved to section block updates:
  `section_ms=3.913`, `section_dirty_ms=0.068`,
  `section_client_ms=3.882`, with `section_updates=13`.
- Queue age remained high because the cap still spreads the churn burst across
  frames: `server_update_queue_depth=406`,
  `server_update_oldest_applied_age_ms=227.549`.
- Worst-frame attribution no longer points at update apply. The top sampled
  frames had zero update-apply work and were dominated by terrain poll wait,
  eye encode/submit, and upload apply.
- Terrain upload/ready pressure remained active but less extreme than Slice 3G:
  `upload_limited=true`, `accept_limited=true`, `backpressured=true`,
  `deferred_sections=3008`, `queued_upload_removed_sections=1408`,
  `max_runtime_upload_apply_ms=5.080`.

Validation (Slice 3H):

- `cargo test --manifest-path native/Cargo.toml -p mclone-client -- --nocapture`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed.
- `cargo ndk -t arm64-v8a --platform 28 check --package mclone-android-xr-client --lib`
  passed.
- Quest validation command passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-churn-entity-index-summary.txt --log /tmp/mclone-quest-openxr-churn-entity-index-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.

### Slice 3I - Batched Section Block Patching

Goal: make `SectionBlockUpdates` client apply thin by avoiding repeated
unpack/repack work for multiple block changes in the same packed section.

- [x] Add `ChunkSnapshot::patch_section_blocks`, preserving ordered per-block
  semantics while unpacking the target packed section once and repacking it
  once after all changes.
- [x] Keep `patch_section_block` as the single-block compatibility wrapper.
- [x] Route `ClientRuntime::apply_section_block_updates` through the batch API.
- [x] Route the server snapshot replay helper through the same batch API so
  shared snapshot semantics stay aligned.
- [x] Cover multi-block batches, no-op batches, all-air removal, and duplicate
  same-position update ordering in core tests; extend the client section-update
  test to apply multiple updates from one packet.

Validation (Slice 3I):

- `cargo test --manifest-path native/Cargo.toml -p mclone-core -p mclone-client -p mclone-server -- --nocapture`
  passed.
- `cargo ndk -t arm64-v8a --platform 28 check --package mclone-android-xr-client --lib`
  passed.
- `cargo test --manifest-path native/Cargo.toml -p mclone-core -p mclone-client -p mclone-server -p mclone-app-runtime -- --nocapture`
  ran but app-runtime's textured local-single-view tests failed before this
  patch's runtime path could be measured because the current asset tree cannot
  load `minecraft:cactus[age=0]` (`age=0` blockstate variant missing). That
  known blocker is also recorded in `136-world-catalog-and-crud-ui.md`.
- Quest chunk-view churn was not rerun for Slice 3I because the same textured
  asset load failure blocks the runtime startup path needed for measurement.
- Follow-up validation after the asset-load fix passed
  `cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-mesh -p mclone-app-runtime -- --nocapture`.
  The fix keeps simulation-facing `age=0` states for cactus/sugar cane while
  allowing known non-model block properties to resolve through vanilla's empty
  blockstate model variant.
- Follow-up Quest chunk-view churn validation passed with the same command
  shape, writing `/tmp/mclone-quest-openxr-churn-batched-patches-summary.txt`
  and `/tmp/mclone-quest-openxr-churn-batched-patches-logcat.txt`.

Recorded Slice 3I result:

- The section-block client tail improved as intended:
  `section_ms=2.073`, `section_dirty_ms=0.126`,
  `section_client_ms=1.997`, down from Slice 3H's `3.913`, `0.068`, and
  `3.882`.
- Overall update-pump max did not improve because the max bucket moved back to
  unload client apply: `max_runtime_poll_ms=6.551`,
  `apply_updates_ms=6.531`, `client_apply_ms=6.158`,
  `unload_ms=6.531`, `unload_dirty_ms=1.750`,
  `unload_client_ms=6.158`, with `unload_updates=16`.
- Worst-frame attribution still did not point at update apply. The top sampled
  frames had zero or tiny update-apply work and were dominated by terrain poll
  wait, eye encode/submit, and upload apply.
- Queue age remained high while the unload cap spread churn across frames:
  `server_update_queue_depth=421`,
  `server_update_oldest_applied_age_ms=241.515`.
- Terrain upload pressure remained active:
  `upload_limited=true`, `accept_limited=true`, `backpressured=true`,
  `queued_upload_removed_sections=1504`, `max_runtime_upload_apply_ms=6.008`.

### Slice 3J - Deferred Client Chunk Snapshot Payload Drops

Goal: keep chunk unload and replacement semantics immediate while moving large
old `ChunkSnapshot` payload destruction out of the client update-apply bucket.

- [x] Queue removed/replaced `ChunkSnapshot`s inside `ClientRuntime` instead of
  dropping their section/light/biome payloads inline during
  `ChunkUnload`/replacement apply.
- [x] Drain deferred snapshot payload items from the normal local integrated
  poll path with an explicit 16-item per-poll budget.
- [x] Keep `chunk_snapshot(pos)` and `loaded_chunk_count()` semantics immediate:
  unloaded chunks disappear from the client replica before their old payloads
  are cleaned up.
- [x] Make `poll_until_idle` wait for the deferred cleanup queue so tests and
  startup helpers do not leave retained old snapshot memory behind.
- [x] Surface `deferred_chunk_drop_ms`, drained item count, and backlog items
  through runtime/XR/Quest perf diagnostics.

Validation (Slice 3J):

- `cargo test --manifest-path native/Cargo.toml -p mclone-client -- --nocapture`
  passed (`89` tests).
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed (`73` tests). The local churn probe reported `9` unloads,
  `max_unload_client_ms=0.008`, `deferred_chunk_drop_items=73`,
  `max_deferred_chunk_drop_ms=0.013`, and zero deferred backlog at settle.
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene` passed.
- Quest chunk-view churn validation passed with the same `2 / 16 / 64` lane,
  writing
  `/tmp/mclone-quest-openxr-churn-deferred-drop-items-16-summary.txt` and
  `/tmp/mclone-quest-openxr-churn-deferred-drop-items-16-logcat.txt`.

Recorded Slice 3J result:

- The unload client-apply tail collapsed:
  `unload_ms=1.158`, `unload_dirty_ms=1.145`,
  `unload_client_ms=0.021`, down from Slice 3I's `6.531`, `1.750`, and
  `6.158`.
- The remaining update-apply max is now section block client application:
  `section_ms=2.062`, `section_dirty_ms=0.074`,
  `section_client_ms=1.996`. Overall update apply is
  `apply_updates_ms=2.066`, `dirty_mark_ms=1.883`,
  `client_apply_ms=1.997`.
- Deferred cleanup is now visible and bounded separately from apply, but this
  still left cleanup inside the app-frame poll:
  `deferred_chunk_drop_ms=3.011`,
  `deferred_chunk_drop_items=16`,
  `deferred_chunk_drop_backlog_items=3078`, and
  `max_runtime_poll_ms=3.018`.
- Queue age remained in the same rough band under the churn lane:
  `server_update_queue_depth=406`,
  `server_update_oldest_applied_age_ms=228.713`.
- Worst sampled frames still did not point at update apply. Top frames had
  zero or small update-apply work and were dominated by terrain poll wait, eye
  encode/submit, and upload apply.
- Terrain upload pressure remained active:
  `upload_limited=true`, `accept_limited=true`, `backpressured=true`,
  `queued_upload_removed_sections=1408`, `max_runtime_upload_apply_ms=5.217`.

### Slice 3K - Native Deferred Payload Drop Worker

Goal: keep deferred old-snapshot payload destruction out of the app/render
frame while retaining the immediate client-replica unload semantics from Slice
3J.

- [x] Keep `ClientRuntime` deterministic by continuing to own the removed
  snapshot queue and vector-level drop-item accounting.
- [x] Add a whole-snapshot handoff API so native runtimes can move old
  `ChunkSnapshot` payload ownership without freeing it on the app frame.
- [x] Add a native `LocalSingleViewSceneRuntime` drop worker that receives
  handed-off snapshots and frees them on a named background thread.
- [x] Report combined client-queue plus worker-pending backlog through the
  existing `deferred_chunk_drop_backlog_items` diagnostic.
- [x] Make `poll_until_idle` wait for the combined backlog so tests and startup
  helpers do not leave background cleanup pending.

Validation (Slice 3K):

- `cargo test --manifest-path native/Cargo.toml -p mclone-client -- --nocapture`
  passed (`90` tests).
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture`
  passed (`78` tests). The local churn probe reported `9` unloads,
  `deferred_chunk_drop_items=137`, `max_deferred_chunk_drop_ms=0.016`,
  `max_deferred_chunk_drop_backlog_items=137`, and zero combined backlog at
  settle.
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene` passed.
- Quest chunk-view churn validation passed with the same `2 / 16 / 64` lane,
  writing
  `/tmp/mclone-quest-openxr-churn-deferred-drop-worker-handoff-summary.txt`
  and
  `/tmp/mclone-quest-openxr-churn-deferred-drop-worker-handoff-logcat.txt`.

Recorded Slice 3K result:

- The deferred cleanup tail left the app-frame poll:
  `deferred_chunk_drop_ms=0.131`,
  `deferred_chunk_drop_items=39`,
  `deferred_chunk_drop_backlog_items=5994`, and
  `max_runtime_poll_ms=2.279`.
- Update apply stayed in the intended thin-apply band for this run:
  `apply_updates_ms=2.183`, `dirty_mark_ms=2.114`,
  `client_apply_ms=1.986`; unload apply was
  `unload_ms=1.581`, `unload_dirty_ms=1.321`,
  `unload_client_ms=0.256`.
- Queue age remained in the same rough band under the churn lane:
  `server_update_queue_depth=406`,
  `server_update_oldest_applied_age_ms=235.040`.
- Worst sampled frames again had zero or tiny update-apply work and were
  dominated by terrain upload/poll-wait shape, not deferred cleanup or update
  application.
- Terrain upload pressure remains the next dominant local integrated follow-up:
  `max_runtime_upload_apply_ms=14.021`,
  `max_runtime_gpu_upload_ms=15.643`, `upload_limited=true`,
  `accept_limited=true`, `backpressured=true`,
  `queued_upload_removed_sections=1408`.
- Ready/prepared churn was no longer the top bucket in this run:
  `max_runtime_ready_sections_ms=1.148`,
  `ready_set_backpressured=72`,
  `ready_set_backpressured_skipped=253`,
  `prepared_rebuild_max_ms=1.246`, and
  `prepared_rebuild_backpressured_dirty=2`.

Slice 3 conclusion: the local integrated update pump now follows the intended
thin-apply shape for resident dirty marking: producer-side decode, strict
receive-order application, resident flag flips, off-frame old-payload cleanup,
and bounded compile/upload pull-work. Remaining frame misses are no longer
explained by hidden command drain, render-thread decode, full-cache dirty
scans, resident dirty-set insertion, entity cleanup scans, old chunk snapshot
destruction in the unload apply bucket, or deferred cleanup inside the app-frame
poll. Keep receive order; any additional divergence should stop between
ordered update records or make oversized lifecycle batches splittable instead
of reordering them. This tactical's remaining bus work is still remote TCP and
web convergence. Local integrated performance follow-up should move primarily
back to the terrain dirty-to-drawable coordinator (`128`): upload apply,
ready-section admission/publication, per-eye encode/submit, and sustained
worker-backlog monitoring if movement patterns get more aggressive than the
current churn lane.

### Slice 4 - Native Remote TCP Session Actor

Status: superseded by focused tactical
[`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md)
after the 2026-07-06 remote `SendOnly` and TCP-readiness fixes in tactical
149 exposed the remaining ready-batch drain/decode stall.

Goal: make native remote dedicated follow the same client bus shape.

- Move `TcpStream` ownership into a dedicated network/session thread; that
  thread owns payload decode and pushes decoded updates to the inbound queue
  in receive order.
- App/runtime enqueues commands and drains decoded updates through the shared
  bus.
- Keep blocking socket IO inside the network thread for the first pass.
- Keep the existing TCP frame codec initially.
- Preserve tactical 149's remote `SendOnly` behavior and remove the remaining
  ready-batch read/decode work from the runtime frame.
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
- Deferred payload cleanup outside native local integrated play: Slice 3K adds
  a native local-runtime worker handoff. When remote TCP and web bus convergence
  resume, decide whether those paths need the same old-snapshot drop handoff or
  whether their unload/update cadence keeps inline/fallback cleanup acceptable.
- Exact oldest pending update age: current diagnostics expose applied update
  age and queued bytes/depth, but the mpsc-backed runner still cannot peek the
  oldest pending update without a transport-side metadata queue.
- Oversized lifecycle/update batches: Quest churn reproduced a single
  update-pump frame with `204` unload updates and `19.323 ms` unload
  client-apply. A default count cap now limits the normal frame pump to `16`
  unload updates, the client entity-by-chunk index removed the O(unloads *
  live entities) cleanup scan, and deferred old snapshot payload cleanup moved
  the capped unload client tail to `0.021 ms`. Deferred cleanup is now explicit
  poll work (`3.011 ms` max at a 16-item budget in the latest Quest churn
  run). Keep this note as the easy-to-find place to retune the cleanup budget
  or split payload items further if sustained movement shows memory/backlog
  pressure.
- Remote TCP session actor: native remote still needs a real inbound update
  bus and producer-side decode so ready response batches do not read/decode on
  the runtime frame. This is now tracked in tactical 151.
- Web bus convergence: browser WebSocket/worker callbacks should feed the same
  ordered inbound queue behind the shared policy.
- Correction latency fast-lane: keep this only as the recorded escalation path
  if measured queue age proves corrections/lifecycle updates need it. Do not
  introduce chunk-stream priority classes for local play.

## Measurement Plan

Product movement lane:

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

Controlled unload/update-pump repro lane:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
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
