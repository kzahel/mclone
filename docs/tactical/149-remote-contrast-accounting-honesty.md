# 149: Remote/Dedicated Contrast Accounting Honesty

Status: active; Slice 1 `SendOnly` code, Quest evidence, the follow-up
interest-command isolation, and the remote response-readiness poll fix landed
2026-07-06. Tactical 151's Quest rebaseline proved the single-batch remote
drain/apply stall moved out of runtime `poll()`, and Slice 4A then removed the
send-side stall where `SendOnly` waited behind the response-paired IO actor.
Remote contrast is now blocked by Slice 2 host-mode-honest projection in this
tactical; web shared-ingress convergence remains tracked in tactical
[`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md).
Drafted 2026-07-06 as the gap-9 follow-on to tactical
[`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md).
Law doc: [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
(gap 9). Predecessor evidence:
[`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
("Attempt remote-dedicated RD5 chunk-view churn contrast" — recorded as a
probe bug / architecture gap, not a baseline).

Workstream: native Rust shared diagnostics/protocol contract. Goal: make the
local-integrated vs remote/dedicated comparison real. The original remote
transport blockers were fixed in Slice 1 and tactical 151: remote sends now
honor `GameplayCommandUpdatePolicy::SendOnly`, ready response-batch
read/decode is producer-side, and command enqueue no longer waits behind a
previous paired response read. The remaining blocker is report honesty: the
client summary still renders server/scheduler counters as zeros instead of
"these live on the remote host". Until that projection is fixed, every
"backpressure moved the cost off the headset" claim is unverifiable, and the
budget controller tactical
([`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md))
cannot use host mode as a controller input with a straight face.

## Sequencing

- Starts any time; no gate. Runs before tactical 150 opens policy levers
  (law-doc gap order: 9 before 10). 150's Slice 0 baseline work may proceed
  in parallel — nothing here changes local-integrated behavior.
- One slice per session; slices in order; the tactical 144 contract invariants
  apply (measurement before policy, one owner for accounting math, sans-I/O
  core, one schema / three sinks, conservation checks on new counters).

## Contract Notes For Implementing Agents

- **This tactical does not change pacing, admission, publication, upload, or
  budget policy.** The `SendOnly` fix in Slice 1 is a correctness fix to make
  the remote path do what the local path already does (defer update draining
  out of the command send), not a new policy.
- **No new accounting math outside `mclone-diagnostics`.** New counters are
  projections of existing report structs or additions to the shared schema
  with a version bump. Run the 144 tripwire greps before declaring any slice
  done.
- **Absence is a capability fact.** A counter that does not exist in a host
  mode is reported as unavailable, never as `0`. This is the same rule the
  GPU timestamp layer follows when `TIMESTAMP_QUERY` is missing.
- Quest rows are captured from one host (the Mac by default) with the device
  recorded, following the 144 host rules.

## Slice 1: Remote Session Honors `SendOnly`

Why: the recorded `5.7s` chunk-view command stall means remote play currently
blocks the XR frame loop on transport/update work that the local-integrated
path defers. That is both a real product bug and a measurement blocker — no
remote lane number is trustworthy while sends can stall the frame.

Deliverables:

- the remote/dedicated session command path honors
  `GameplayCommandUpdatePolicy::SendOnly`
  (`native/crates/mclone-app-runtime/src/lib.rs` defines the policy; the
  local path consumes it in
  `native/crates/mclone-app-runtime/src/local_single_view.rs`; the XR scene
  passes `SendOnly` in `native/crates/mclone-xr-scene/src/lib.rs`). Find the
  remote session adapter that drains/blocks inline and make it defer exactly
  like the local path;
- a shared regression test that sends a gameplay command through a
  remote-shaped session and asserts no update drain/apply happens inside the
  send call;
- command-send timing (`GameplayCommandTiming.send_ms`) for remote sessions
  visible in the shared report so a reintroduced stall is a report line, not
  an archaeology project.

Non-goals: no transport rewrite, no reordering of the update pump, no change
to local-integrated behavior (A/B: local lanes unchanged within noise).

Exit criteria:

- Quest remote churn run shows no command-send stalls over one frame period
  (was `5.7s`);
- local-integrated RD5 churn A/B unchanged within recorded noise;
- regression test in place and failing when the fix is reverted (verify once).

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
pnpm native:android-xr:apk
git diff --check
```

Plus one local-integrated and one remote (`--adb-reverse --start-server`)
Quest churn run recorded in this section.

2026-07-06 Slice 1 implementation status:

- Split native TCP remote sessions into explicit command-send and
  command-response-drain operations while keeping the old request/response
  helper as a convenience wrapper.
- Updated the shared `RemoteDedicatedServerSession` contract plus desktop,
  Android, Android XR, and XR placeholder adapters to expose split send/drain.
- Remote `GameplayCommandUpdatePolicy::SendOnly` now sends the command,
  records a deferred command exchange, and leaves the response batch for the
  remote update pump instead of draining/applying it inside the send call.
  `DrainImmediately` keeps the previous synchronous behavior and first drains
  any older pending responses to preserve protocol order.
- Remote `poll()` now drains pending response batches, applies updates, and
  records the same shared `RuntimePollDiagnostics` update/apply timing fields
  used by local-integrated polling where the current request/response protocol
  has those facts.
- Added a shared app-runtime regression,
  `host_mode::tests::remote_dedicated_send_only_defers_update_drain_and_apply`,
  that fails if a remote-shaped send-only command drains/applies the scripted
  update inside the send call.

Validation run on 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
pnpm native:android-xr:apk
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client
git diff --check
```

Results: all passed. `cargo check -p mclone-android-xr-client` reported the
existing `ANDROID_REMOTE_ADDR_NONE_SENTINEL` /
`normalize_android_legacy_remote_addr` dead-code warnings only.

144 tripwire greps on 2026-07-06:

- accounting math outside `mclone-diagnostics`: 1 existing hit
  (`native/crates/mclone-worldgen/src/levelgen/tests/terrain.rs`, test helper
  `percentile`);
- clock reads inside `mclone-diagnostics` core outside `clock.rs`: 0 hits;
- GPU timestamp sites: 1 existing shared-render hit
  (`native/crates/mclone-render/src/gpu_timestamps.rs`).

Quest churn evidence captured 2026-07-06 on Meta Quest 3
`2G0YC1ZF93041Z`, APK built from `5d367b90`:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-sendonly-local-20260706-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-sendonly-local-20260706-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse \
  --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-sendonly-remote-20260706-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-sendonly-remote-20260706-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-churn-rd5-sendonly-remote-20260706-server.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Summary:

| Mode | Settle | FPS | Runtime skipped | App avg | App p50 | App p95 | App p99 | App max | Headroom avg | App over-period | Over 2x / 4x |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| local integrated | `32.349s` | `68.41` | `0` | `9.859ms` | `9.852ms` | `17.470ms` | `19.217ms` | `21.028ms` | `+4.030ms` | `953 / 3079` (`31.0%`) | `2 / 0` |
| remote dedicated, `--adb-reverse --start-server` | `5.005s` | `37.90` | `0` | `21.710ms` | `9.656ms` | `24.180ms` | `28.821ms` | `5887.223ms` | `-7.821ms` | `651 / 1706` (`38.2%`) | `31 / 16` |

Local integrated still ran without skipped runtime frames and with app-work max
near the 2026-07-05 row (`21.028ms` vs `21.738ms`), but average/p95 were
higher on this run. Treat that as enough to prove no catastrophic local A/B
regression from the `SendOnly` fix, not as a fresh local performance baseline.

Remote result:

- The original `SendOnly` command-send/update-drain stall is fixed in the
  report surface: no `remote dedicated session still ignores SendOnly` warning,
  and `MCLONE_ANDROID_XR_PERF_LOCOMOTION_COMMAND` reported
  `max_send_ms=0.000`, `max_drain_updates_ms=0.000`,
  `max_apply_updates_ms=0.000`, and zero updates.
- A different remote frame-loop stall remains: worst frame `142` had
  `app_work_ms=5887.223`, `locomotion_ms=5875.345`, and
  `commit_interest_ms=5875.280`, while command timing stayed zero. This means
  the old transport/update-drain archaeology problem is gone, but the remote
  churn lane still cannot be used as a "server work moved off headset" contrast
  row.
- Remote client reports still render server/scheduler counters as zeros
  (`MCLONE_ANDROID_XR_PERF_QUEUE_MAX` all zero; `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX`
  update counters all zero), and the dedicated server log only records startup
  (`mclone dedicated server listening ...`) with no labeled server-side
  summary. That is the intended Slice 2 gap.

2026-07-06 interest-command isolation follow-up:

- Added policy-aware, timed interest/chunk-view updates in
  `NativeSingleViewSceneRuntime`: recurring `set_chunk_view` /
  `set_interest_center` calls now send their command with
  `GameplayCommandUpdatePolicy::SendOnly` and expose the command timing.
- XR locomotion records the interest command separately from player-pose
  sync, and Android XR emits
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` plus matching
  per-worst-frame markers.
- The Quest validation summary extractor now retains the locomotion and
  worst-frame command markers instead of requiring a logcat dig for this
  specific isolation.

Validation run on 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
pnpm native:android-xr:apk
```

Results: all passed. `cargo check -p mclone-android-xr-client` still reported
only the existing `ANDROID_REMOTE_ADDR_NONE_SENTINEL` /
`normalize_android_legacy_remote_addr` dead-code warnings.

Quest remote churn evidence captured 2026-07-06 on Meta Quest 3
`2G0YC1ZF93041Z` from the pre-commit worktree after the interest-command
isolation change:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-interest-sendonly-remote-20260706-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-interest-sendonly-remote-20260706-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-churn-rd5-interest-sendonly-remote-20260706-server.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Result:

- The interest command is now isolated and cheap:
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` in logcat reported
  `max_total_ms=0.287`, `max_send_ms=0.279`,
  `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`, and zero
  updates.
- The previous remote `commit_interest` frame-loop stall is gone:
  `MCLONE_ANDROID_XR_PERF_STAGES` reported `max_locomotion_ms=0.337`, and
  worst frame `141` had `locomotion_ms=0.151` with
  `commit_interest_ms=0.111`.
- The remaining remote stall is in runtime polling/update draining, not in
  locomotion or command send: worst frame `141` had
  `app_work_ms=6007.791` / `render_mclone_frame_ms=6007.728`, while
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX` reported
  `poll_total_ms=6000.083`, `drain_updates_ms=5990.776`,
  `apply_updates_ms=9.303`, `updates=402`, `snapshot_updates=169`,
  `section_updates=61`, and `unload_updates=169`.

2026-07-06 remote response-readiness follow-up:

- Added `NativeClientSession::try_drain_command_updates`, which uses a
  nonblocking TCP readiness probe before attempting the existing blocking
  response-batch read.
- Extended `RemoteDedicatedServerSession` with `try_drain_command_updates`
  and wired the native desktop, Android, and Android XR adapters through it.
- Remote render-frame `poll()` now drains only ready response batches.
  Synchronous startup, `DrainImmediately`, and `poll_until_idle` keep blocking
  drains so initialization and protocol ordering stay unchanged.
- `RuntimePollDiagnostics` now carries remote pending-response depth through
  `server_update_queue_depth`; the Android XR perf markers therefore report
  a real remote client-side pending queue instead of rendering that fact as
  zero.
- Added regressions:
  `mclone_net::tests::native_tcp_client_try_drain_does_not_wait_for_delayed_response`
  and
  `local_single_view::tests::remote_poll_skips_pending_response_until_ready`.

Validation run on 2026-07-06:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
pnpm native:android-xr:apk
```

Results: all passed. `cargo check -p mclone-android-xr-client` still reported
only the existing `ANDROID_REMOTE_ADDR_NONE_SENTINEL` /
`normalize_android_legacy_remote_addr` dead-code warnings.

Quest remote churn evidence captured 2026-07-06 on Meta Quest 3
`2G0YC1ZF93041Z` from the pre-commit worktree after the response-readiness
change:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build --skip-assets \
  --adb-reverse --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-remote-ready-drain-20260706-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-remote-ready-drain-20260706-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-churn-rd5-remote-ready-drain-20260706-server.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Result:

- The previous multi-second wait-for-server poll stall is gone:
  `app_work_max_ms` dropped from `6007.791` to `268.630`, and
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX poll_total_ms` dropped from
  `6000.083` to `262.168`.
- Remote pending-response depth is now visible:
  `MCLONE_ANDROID_XR_PERF_QUEUE_MAX server_update_q=2` and
  `MCLONE_ANDROID_XR_PERF_UPLOAD_MAX server_update_queue_depth=2`.
- Locomotion/interest remains isolated:
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND` reported
  `max_total_ms=0.242`, `max_send_ms=0.232`,
  `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`, and zero
  updates.
- Before tactical 151, the remaining remote stall was one ready response batch
  being drained and applied in a single frame: worst frame `2332` had
  `app_work_ms=268.630`, `render_mclone_frame_ms=268.406`, and
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX` reported
  `drain_updates_ms=253.930`, `apply_updates_ms=9.344`, `updates=402`,
  `snapshot_updates=169`, `section_updates=61`, and
  `unload_updates=169`.

2026-07-06 tactical 151 Slice 4 rebaseline:

- The ready-batch runtime drain moved off the headset frame. The same Quest RD5
  remote churn lane reported `poll_total_ms=2.127`, `drain_updates_ms=0.148`,
  `apply_updates_ms=2.112`, and `producer_read_ms=5766.112` in
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX`.
- The remote lane is still not interpretable enough for durable contrast rows:
  the worst frame moved to `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND`
  with `max_total_ms=2770.917`, `max_send_ms=2770.910`,
  `max_drain_updates_ms=0.000`, `max_apply_updates_ms=0.000`, and zero
  updates. That means command send can still wait for the IO actor to finish a
  previous response read before it writes/acks the next `SendOnly` command.

2026-07-06 tactical 151 Slice 4A rebaseline:

- Remote command enqueue now returns without waiting behind the previous paired
  response read. The same Quest RD5 remote churn lane reported
  `MCLONE_ANDROID_XR_PERF_LOCOMOTION_INTEREST_COMMAND max_total_ms=0.061`,
  `max_send_ms=0.021`, `max_drain_updates_ms=0.000`,
  `max_apply_updates_ms=0.000`, and zero updates.
- Runtime receive/apply remained in the intended producer/apply shape:
  `MCLONE_ANDROID_XR_PERF_RUNTIME_MAX poll_total_ms=2.134`,
  `drain_updates_ms=0.132`, `apply_updates_ms=2.091`,
  `producer_read_ms=6021.649`, and `producer_decode_ms=13.114`.
- The remote lane is now clean enough to resume Slice 2 host-mode-honest report
  projection. The remaining accounting problem is presentation of unavailable
  remote-host counters as zeros, not a known headset-frame transport stall.

Next implementation step before durable contrast rows: do Slice 2's
host-mode-honest report projection so remote client/server halves stop
presenting unavailable counters as zeros.

## Slice 2: Host-Mode-Honest Report Projection

Why: gap 9's core ask. The report schema must say which stages ran on this
device and which ran on the remote host, so the same report shape can be read
for both host modes without lying.

Deliverables:

- host-mode capability projection in the shared schema (schema version bump
  in `mclone-diagnostics`): local-integrated sessions render the peer-thread
  and host-scheduler panels as today; remote sessions render them as
  explicitly unavailable-on-this-device, and instead surface the
  client-side lane that remote play actually pays:
  inbound queue depth/bytes, oldest-applied-update age, decode/apply elapsed,
  update-pump stalls (these already exist in `RuntimeUpdatePumpReport` /
  `RuntimePollTiming` — this is projection, not new math);
- Quest perf markers include the client network/update/apply counters in
  remote mode (additive markers only; existing `MCLONE_ANDROID_XR_PERF_*`
  keys preserved);
- the dedicated server process emits its own scheduler/publication summary
  (tick time, publish counts, pending publication, queue depths) as a
  labeled server-side report, so the contrast has both halves — each measured
  where it runs;
- conservation checks active on any new queue counters.

Non-goals: no cross-process clock alignment (the law doc's clock-domain rule
stands — the two halves are aligned structurally by lane and wall-clock-ish
sample windows, not by a shared epoch); no new transport diagnostics protocol
messages unless a named report field needs one (record it if so).

Exit criteria:

- one desktop remote-loopback run and one Quest remote run produce client
  reports with populated network/update panels and explicitly-unavailable
  server panels (no silent zeros);
- the dedicated server run produces its own labeled summary;
- 144 tripwire greps clean.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-diagnostics
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:accounting:smoke
pnpm native:startup-streaming:smoke
git diff --check
```

## Slice 3: Capture The Contrast Rows

Why: the whole point — durable local-vs-remote rows that later policy work
can cite instead of re-deriving.

Deliverables:

- Quest RD5 chunk-view churn contrast, same device, same session
  configuration, two runs each:
  - local integrated (existing lane:
    `android-xr/validate-quest-openxr.sh --perf-chunk-view-churn ...`);
  - remote dedicated via `--adb-reverse --start-server`;
  recorded in
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  with app-work/headroom, update queue depth/age, upload/accept maxima, and
  the server-side summary from the host process;
- one desktop-shaped remote contrast (native client against a loopback
  dedicated server) recorded in
  [`../performance-records.md`](../performance-records.md). If the desktop
  startup-streaming probe cannot run in remote-host mode, record that as the
  gap and capture a manual-session report excerpt instead — do not build a
  new permanent lane just for this row;
- a short interpretation note in tactical 142's stage-ownership section:
  which costs measurably left the headset in remote mode, which stayed, and
  which changed shape (e.g. publication burst -> network burst).

Exit criteria:

- both contrast row sets recorded with host, device, commit, and lane args;
- the 142 open checkbox "Fix remote-dedicated churn accounting before using
  remote play as the comparison" is checked with a pointer here.

Validation: the capture commands above, plus `git diff --check`.

## Out Of Scope

- Any budget/admission/publication policy change — tactical
  [`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md).
- Web shared-ingress convergence and broader remote transport cleanup —
  tactical
  [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md);
  native remote receive/decode and command-send decoupling completed there.
- Broader remote transport performance work (server push, batching,
  compression, WebSocket/web convergence) — tactical
  [`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md).
- Multiplayer correctness/authority work.

## Open Questions

- (record here during implementation)
