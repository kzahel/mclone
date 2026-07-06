# 149: Remote/Dedicated Contrast Accounting Honesty

Status: active; Slice 1 code landed 2026-07-06, Quest churn evidence pending
(no ADB device attached during implementation). Drafted 2026-07-06 as the
gap-9 follow-on to tactical
[`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md).
Law doc: [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
(gap 9). Predecessor evidence:
[`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
("Attempt remote-dedicated RD5 chunk-view churn contrast" — recorded as a
probe bug / architecture gap, not a baseline).

Workstream: native Rust shared diagnostics/protocol contract. Goal: make the
local-integrated vs remote/dedicated comparison real. Today the remote lane
cannot be interpreted: the remote session path ignores
`GameplayCommandUpdatePolicy::SendOnly` (a recorded `5.7s` chunk-view command
stall on the XR frame loop), and the client summary renders server/scheduler
counters as zeros instead of "these live on the remote host". Until this is
fixed, every "backpressure moved the cost off the headset" claim is
unverifiable, and the budget controller tactical
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

Quest churn evidence: not captured in this session because `adb devices`
reported no attached device. Remaining Slice 1 exit work is to run the
local-integrated RD5 churn and remote `--adb-reverse --start-server` RD5 churn
on a Quest and record whether command-send stalls stay under one frame period
and local-integrated A/B remains within noise.

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
- Remote transport performance work (batching, compression, WebSocket/web
  convergence) — tactical
  [`133-session-network-bus-and-update-pacing.md`](133-session-network-bus-and-update-pacing.md).
- Multiplayer correctness/authority work.

## Open Questions

- (record here during implementation)
