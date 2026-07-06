# 145: Native Rust Organization Refactor

Status: active parent; ORG-00 landed with current baseline validation blockers.
Opened 2026-07-06. This tactical is the current implementation tracker for
reducing oversized native Rust modules while preserving behavior, public
coherence, vanilla parity, platform boundaries, and benchmark performance.

Workstream: native Rust shared architecture plus platform adapter cleanup.
This is a successor checkpoint to
[`035-native-codebase-health-refactor-plan.md`](035-native-codebase-health-refactor-plan.md):
035 records earlier cleanup history and durable principles; this document owns
the July 2026 audit, the slice order, and the no-regression contract for the
current large-file pass.

## Purpose

The goal is to make the native Rust workspace feel like a maintainable
open-source Rust project: small modules with clear ownership, thin crate roots,
module-local tests, stable public re-exports, and platform adapter code that
does not hide shared engine policy.

The goal is not to rewrite behavior, introduce new abstractions for their own
sake, or win benchmarks. Every slice starts as a move-only or nearly move-only
refactor. A performance improvement is welcome only if it falls out of clearer
ownership and is validated separately; it is not part of this tactical's
success criteria.

## Current Audit Snapshot

Audit command shape:

```bash
find native -name '*.rs' -not -path '*/target/*' -not -path '*/vendor/*' -print0 \
  | xargs -0 wc -l
```

Current non-vendored Rust footprint:

- `242` Rust source files under `native/`.
- `211,946` total Rust LoC.
- `66` files above `1,000` LoC, totaling `157,160` LoC.
- Of those `66`, `16` are app files and `50` are shared crate files.

After discounting trailing inline `mod tests` blocks, the production footprint
still has:

- `45` production files above `1,000` LoC.
- `103,117` production LoC inside those oversized files.
- `3` production modules above `5,000` LoC.
- `7` production modules between `3,000` and `4,999` LoC.

Largest production-heavy files at audit time:

| File | Production LoC | Total LoC | Notes |
|---|---:|---:|---|
| `native/crates/mclone-xr-scene/src/lib.rs` | `7,351` | `9,032` | shared XR terrain scene, locomotion, UI panels, timing, render paths, automation, tests |
| `native/apps/mclone-android-xr-client/src/lib.rs` | `6,848` | `6,881` | Android activity glue, startup args, OpenXR frame loops, perf reporting, target acquisition |
| `native/apps/mclone-web-client/src/web_canvas.rs` | `6,307` | `6,377` | wasm exports, render compiler, web session, catalog bridge, JS codecs, canvas rendering |
| `native/crates/mclone-render-session/src/lib.rs` | `4,690` | `7,418` | mesh inputs, dirty/render-section sync, upload queues, engine camera, compile traits, tests |
| `native/crates/mclone-render/src/chunk.rs` | `3,838` | `4,781` | render views, culling, targets, GPU upload, draw resources, renderers |
| `native/apps/mclone-native-client/src/perf.rs` | `3,700` | `3,785` | desktop/headless perf reports and benchmark JSON |
| `native/crates/mclone-server/src/scheduler.rs` | `3,400` | `3,946` | scheduler, publication, persistence integration, fluid tick execution |
| `native/crates/mclone-ui/src/lib.rs` | `3,242` | `3,884` | primitives, widgets, state, debug overlays, flat HUD, legacy UI helpers |
| `native/apps/mclone-android-client/src/lib.rs` | `3,137` | `3,137` | flat Android app adapter |
| `native/crates/mclone-server/src/persistence.rs` | `3,035` | `3,925` | records, codecs, stores, mailbox, actor backends, SQLite |
| `native/crates/mclone-ui/src/v2.rs` | `2,987` | `4,447` | UI v2 layout, retained draw caches, HUD/loading surfaces, tests |

Large test-heavy files that should be handled differently:

| File | Production LoC | Test LoC | Recommendation |
|---|---:|---:|---|
| `native/crates/mclone-worldgen/src/levelgen.rs` | `39` | `3,960` | move tests out; production facade is already good |
| `native/crates/mclone-server/src/lib.rs` | `183` | `3,890` | move tests out; crate root should stay a facade |
| `native/crates/mclone-worldgen/src/feature.rs` | `292` | `3,377` | move tests out; production facade mostly good |
| `native/crates/mclone-server/src/integrated.rs` | `2,065` | `2,231` | split production later; tests can move first |

Crate/app footprint hotspots:

| Crate or app | Rust LoC | Files above 1k LoC |
|---|---:|---:|
| `native/crates/mclone-server` | `38,829` | `9` |
| `native/crates/mclone-worldgen` | `29,793` | `11` |
| `native/apps/mclone-native-client` | `22,090` | `10` |
| `native/crates/mclone-render` | `18,146` | `6` |
| `native/crates/mclone-app-runtime` | `15,135` | `7` |
| `native/apps/mclone-web-client` | `11,139` | `3` |
| `native/crates/mclone-xr-scene` | `9,032` | `1` |
| `native/crates/mclone-ui` | `8,331` | `2` |
| `native/apps/mclone-android-xr-client` | `7,442` | `1` |
| `native/crates/mclone-render-session` | `7,418` | `1` |

## Target Shape

This tactical uses soft targets, not an arbitrary hard line-count rule:

- crate roots are mostly `mod` declarations, `pub use` re-exports, small
  shared helpers, and crate-level docs;
- hot-path modules normally stay below about `1,500` production LoC;
- ordinary modules normally stay below about `1,000` production LoC;
- no production module stays above `2,000` LoC unless the slice records an
  explicit exception such as generated/table-like data or reference-parity
  structure;
- tests live next to the module they validate, but not inside already-large
  production files;
- public paths remain stable through re-exports unless a slice explicitly
  records and validates a narrow API cleanup;
- module boundaries follow existing crate ownership first, Java 1.17.1
  reference boundaries second for vanilla/parity systems, and platform
  adapter boundaries for app crates;
- platform adapters own lifecycle, OS/browser/Android/OpenXR glue, and
  target acquisition only. They do not become the home for gameplay,
  rendering semantics, UI policy, persistence policy, or input semantics.

## Contract For Implementing Agents

Read this section and your slice section before writing code. If context is
compressed mid-task, re-read this section first.

Invariants, in priority order:

1. **Move-only first.** The default patch shape is new modules plus moved
   items, imports, and re-exports. Do not mix behavior changes into an
   organization slice.
2. **No performance policy changes.** Do not change frame budgets, admission
   limits, scheduling rules, worker counts, upload pacing, culling behavior, or
   renderer math. If a split exposes a bug, record it and fix it in a separate
   behavior slice.
3. **No cross-crate hot-path migration by default.** Splitting within a crate
   is the safe path. Moving hot code between crates can alter monomorphization,
   feature gates, visibility, and compile behavior; do it only if the slice
   explicitly says so.
4. **Stable public surface.** Preserve existing public paths with `pub use`
   during move-only splits. Visibility tightening is allowed only after tests
   pass and only when all external call sites are known.
5. **One owner per concept.** Do not create parallel helper modules with
   subtly different semantics. If two call sites share a helper only
   accidentally, keep it local.
6. **Reference parity stays legible.** For worldgen, simulation, vanilla visual
   behavior, and rendering semantics, read the corresponding
   `reference/minecraft-1.17.1/src/` source before choosing a boundary.
7. **Platform boundaries stay thin.** App splits should reveal glue, not
   legitimize app-owned policy. If a split finds shared behavior in an app
   crate, record the shared-owner follow-up instead of burying it deeper.
8. **One slice per session.** Do not start the next slice in the same run
   unless the user asks. Between slices, update the status table and let the
   user review.

A slice is done only when:

- its status row is updated;
- its deliverables and exit criteria are checked off;
- validation commands were run and results recorded in the slice's log;
- line-count tripwires were rerun and recorded if the slice changes module
  layout;
- any deferred exception is recorded under Open Questions or Follow-Up Queue.

## Baseline And Tripwire Commands

Run these in Slice ORG-00 and after any production organization slice.

Total oversized files:

```bash
find native -name '*.rs' -not -path '*/target/*' -not -path '*/vendor/*' -print0 \
  | xargs -0 wc -l \
  | awk '$2 != "total" && $1 ~ /^[0-9]+$/ && $1 >= 1000 {print $1, $2}' \
  | sort -nr
```

Production-sized files, discounting trailing inline `mod tests`:

```bash
for f in $(find native -name '*.rs' -not -path '*/target/*' -not -path '*/vendor/*'); do
  lines=$(wc -l < "$f" | tr -d ' ')
  testline=$(rg -n "^mod tests \\{" "$f" | head -1 | cut -d: -f1)
  if [ -n "$testline" ]; then prod=$((testline - 1)); else prod=$lines; fi
  if [ "$prod" -ge 1000 ]; then
    printf '%5d prod %5d total %s\n' "$prod" "$lines" "$f"
  fi
done | sort -nr
```

Per-crate/app footprint:

```bash
for d in native/crates/* native/apps/*; do
  [ -d "$d/src" ] || continue
  loc=$(find "$d/src" -name '*.rs' -not -path '*/target/*' -print0 \
    | xargs -0 wc -l 2>/dev/null \
    | awk '$2 != "total" && $1 ~ /^[0-9]+$/ {s += $1} END {print s + 0}')
  big=$(find "$d/src" -name '*.rs' -not -path '*/target/*' -print0 \
    | xargs -0 wc -l 2>/dev/null \
    | awk '$2 != "total" && $1 ~ /^[0-9]+$/ && $1 >= 1000 {n++} END {print n + 0}')
  printf '%6d %2d %s\n' "$loc" "$big" "$d"
done | sort -nr
```

Public surface inventory for files being split:

```bash
rg -n "^pub (struct|enum|trait|fn|type|const)|^pub use|^pub mod" \
  <files-being-split>
```

Broad app-policy smoke when touching app crates:

```bash
rg -n "GameUiAction::[A-Za-z]+(\\(_\\))? => \\{\\}" \
  native/apps native/crates/mclone-xr-scene/src
```

## Validation Policy

Every slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml
git diff --check
```

Shared crates that must compile for browser/WASM:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime --lib --target wasm32-unknown-unknown
cargo build --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
```

Hot server/worldgen/render slices use release A/B rows on the same host before
and after the slice. Treat a single-run delta above `3%` to `5%`, or any worse
frame-budget/dropped-frame tail, as an investigation trigger. Repeat before
calling it a regression or a win.

Server/worldgen hot slices:

```bash
pnpm native:worldgen:perf
pnpm native:scheduler-loading:perf
pnpm native:scheduler-loading:persisted-memory:perf
pnpm native:scheduler-loading:persisted-sqlite:perf
```

Render/render-session hot slices:

```bash
pnpm native:mesh-cpu:perf
pnpm native:gpu-upload:perf
pnpm native:movement-frame:perf
pnpm native:startup-streaming:persisted:perf
pnpm native:timedemo:perf
```

App/platform adapter slices:

```bash
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
pnpm native:web:app-smoke
```

Android/XR slices add the relevant device lane before closing the slice when
hardware is available:

```bash
pnpm native:android:avd-session-smoke
pnpm native:android-xr:session-smoke
pnpm native:android-xr:perf:orbit:rd7:metrics
```

Device/headset lanes may be marked blocked only when the blocker is recorded
with host, device, and command details. Do not replace a required device lane
with a desktop-only smoke silently.

## Slice Status

| Slice | Scope | Status |
|---|---|---|
| ORG-00: baseline and no-regression harness | inventory, line counts, benchmark baselines | landed 2026-07-06; baseline has current validation blockers |
| ORG-01: test relocation pass | move large inline tests out of production files | open |
| ORG-02: crate-root and facade cleanup | thin roots and module-local test facades | open |
| ORG-03: `mclone-render-session` split | render-section session ownership modules | open |
| ORG-04: `mclone-xr-scene` non-render split | options, timing, locomotion, tracking, UI math | open |
| ORG-05: `mclone-xr-scene` render/state split | terrain state, stereo/multiview render paths, overlay rendering | open |
| ORG-06: Android XR adapter split | activity/startup/frame-loop/perf/target modules | open |
| ORG-07: Web canvas adapter split | wasm exports, compiler session, render session, catalog bridge, JS codecs | open |
| ORG-08: chunk renderer split | view/target/culling/upload/draw-resource modules | open |
| ORG-09: UI crate split | primitives, widgets, state, v2 layout, HUD/loading draw lists | open |
| ORG-10: server scheduler/persistence split | scheduler submodules and persistence backend/codecs | open |
| ORG-11: worldgen and table exceptions | remaining large worldgen modules and explicit table/reference exceptions | open |
| ORG-12: secondary large-file cleanup | native-client, app-runtime, client, input, protocol, mesh follow-through | open |
| ORG-13: closeout audit | final counts, validation matrix, exception list | open |

## ORG-00: Baseline And No-Regression Harness

Why: every later slice needs an agreed baseline and a repeatable way to prove
"this was organization only."

Deliverables:

- run and record the three line-count tripwire commands;
- record the public-surface inventory for the top five production-heavy files;
- record current validation availability on this host: desktop, web, Android
  AVD, Quest/OpenXR, Windows-only;
- choose the release A/B lanes that will be reused for hot slices and record
  one clean baseline row for each;
- record the current git commit and whether the tree was dirty.

Non-goals:

- no production code changes;
- no module moves;
- no benchmark interpretation beyond baseline capture.

Exit criteria:

- this section contains the baseline counts and raw `/tmp` artifact paths for
  benchmark JSON or logs intentionally kept outside the repo;
- status row updated.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:perf:smoke
pnpm native:web:build
git diff --check
```

Log:

- Landed 2026-07-06 as a documentation/baseline slice. No production code
  changed.

Host and git state:

- Host: `kmacbook`, Darwin `25.5.0`, macOS `26.5.1` build `25F80`,
  `arm64`, Apple M4 Pro.
- Toolchain: Node `v25.8.2`, pnpm `9.15.1`, Cargo `1.92.0`, Rust
  `1.92.0`.
- Baseline commit reported by commands: `c0b80603`.
- `git_dirty=true` for benchmark JSON because this tactical file and the
  tactical README index were in progress.

Raw inventory artifacts:

- `/tmp/mclone-org145-oversized-files.txt`
- `/tmp/mclone-org145-production-oversized-files.txt`
- `/tmp/mclone-org145-crate-footprint.txt`
- `/tmp/mclone-org145-top5-public-surface.txt`
- `/tmp/mclone-org145-app-policy-smoke.txt`

Line-count baseline:

| Inventory | Count |
|---|---:|
| Non-vendored Rust files above `1,000` LoC | `66` |
| Production files above `1,000` LoC after discounting trailing `mod tests` | `45` |
| Crate/app footprint rows recorded | `27` |
| Top-five public surface rows recorded | `235` |
| App-policy inert-arm smoke hits | `0` |

Largest production files remain:

| File | Production LoC | Total LoC |
|---|---:|---:|
| `native/crates/mclone-xr-scene/src/lib.rs` | `7,351` | `9,032` |
| `native/apps/mclone-android-xr-client/src/lib.rs` | `6,848` | `6,881` |
| `native/apps/mclone-web-client/src/web_canvas.rs` | `6,307` | `6,377` |
| `native/crates/mclone-render-session/src/lib.rs` | `4,690` | `7,418` |
| `native/crates/mclone-render/src/chunk.rs` | `3,838` | `4,781` |

Top-five public surface counts:

| File | Public rows |
|---|---:|
| `native/crates/mclone-render-session/src/lib.rs` | `107` |
| `native/crates/mclone-xr-scene/src/lib.rs` | `79` |
| `native/crates/mclone-render/src/chunk.rs` | `31` |
| `native/apps/mclone-web-client/src/web_canvas.rs` | `17` |
| `native/apps/mclone-android-xr-client/src/lib.rs` | `1` |

Validation availability:

- Desktop/native test and headless/perf lanes are available on this Mac.
- `pnpm host:check` reports Chrome and Playwright installed; browser WebGPU
  capture is a candidate lane, with the usual macOS Chrome/ANGLE Metal note.
- Android SDK tools are installed: `adb`, `emulator`, `avdmanager`, and
  `sdkmanager`.
- AVDs available: `jstorrent-dev`, `jstorrent-playstore`, and
  `jstorrent-tablet`.
- `adb devices` reported no attached devices, so Quest/physical-device lanes
  were not available in this slice.
- `pnpm native:xr:check` passed on the Mac WiVRn OpenXR path. Windows native
  desktop-XR validation was not run on this Mac host and remains a host-switch
  lane.

Selected release A/B baseline rows:

| Lane | Artifact | Headline |
|---|---|---|
| `pnpm --silent native:worldgen:perf` | `/tmp/mclone-org145-worldgen-perf.json` | radius `1`, target `9`; surface `11.911ms`; features cold `70.075ms` / `128.434` chunks/sec; features warm `17.315ms` / `519.793` chunks/sec |
| `pnpm --silent native:scheduler-loading:persisted-sqlite:perf` | `/tmp/mclone-org145-scheduler-loading-persisted-sqlite-perf.json` | RD10 persisted SQLite reload; prewarm ready `13.324s`, settled `15.409s`; reload ready `0.575s`, settled `0.668s`; `529` target chunks, `625` stored records, `35.5 MB` SQLite |
| `pnpm --silent native:mesh-cpu:perf` | `/tmp/mclone-org145-mesh-cpu-perf.json` | RD10 persisted mesh CPU; `8,464` sections, `2,789` non-empty; mesh build `10.885s`; `777.607` sections/sec |
| `pnpm --silent native:gpu-upload:perf` | `/tmp/mclone-org145-gpu-upload-perf.json` | RD10 prebuilt mesh upload; mesh build `11.022s`; section upload `137.970ms`; uploaded `360.4 MB` |
| `pnpm --silent native:startup-streaming:persisted:perf` | `/tmp/mclone-org145-startup-streaming-persisted-perf.json` | RD10 persisted startup; playable `112.923ms`; full view `1.022s`; render quiescent `25.155s`; p95 frame `9.901ms`; max frame `55.859ms`; `37` over-budget frames |
| `pnpm --silent native:timedemo:perf` | `/tmp/mclone-org145-timedemo-perf.json` | RD5 timedemo, `240` frames; scene build `17.359s`; average frame `4.022ms`; max frame `34.445ms`; average drawn sections `138.717` |

Validation results:

```text
cargo fmt --manifest-path native/Cargo.toml --all --check
PASS

pnpm native:perf:smoke
PASS

git diff --check
PASS

pnpm native:xr:check
PASS
```

Current baseline blockers, reproduced without code changes:

```text
cargo test --manifest-path native/Cargo.toml
FAIL

Reproduced in isolation:
- cargo test --manifest-path native/Cargo.toml -p mclone-server --lib \
    runner::native::tests::native_runner_reports_native_thread_kind_and_queue_depths
  Fails at crates/mclone-server/src/runner.rs:1378:
  expected thread kind None, got MessageTransfer.

- cargo test --manifest-path native/Cargo.toml -p mclone-server --lib \
    tests::generated_origin_chunk_block_light_strict_matches_scheduler_light_oracle_fixture
  Fails at crates/mclone-server/src/lib.rs:1509:
  block light mismatch at section 5 byte 1499, expected 0x00, got 0x01.

pnpm native:web:build
FAIL

Fails in native/crates/mclone-render/src/gpu_timestamps.rs:443:
use of unresolved module or unlinked crate `log` for `log::warn!`.
```

Interpretation for later organization slices:

- These blockers predate any ORG-01 refactor and must not be attributed to test
  relocation.
- ORG-01 may still proceed as a move-only documentation/code-organization
  slice, but it must rerun and record these exact failing commands. Any new or
  different failure is a regression.

## ORG-01: Test Relocation Pass

Why: many "large files" are large because tests live inside otherwise-thin
facades. Moving tests first lowers review noise with almost no runtime risk.

Primary targets:

- `native/crates/mclone-worldgen/src/levelgen.rs`
- `native/crates/mclone-worldgen/src/feature.rs`
- `native/crates/mclone-server/src/lib.rs`
- `native/crates/mclone-server/src/integrated.rs`
- `native/crates/mclone-render-session/src/lib.rs`
- `native/crates/mclone-ui/src/v2.rs`
- `native/apps/mclone-native-client/src/main.rs`
- `native/apps/mclone-native-client/src/app.rs`
- `native/apps/mclone-native-client/src/scene_runtime.rs`

Deliverables:

- replace large inline `mod tests` blocks with `#[cfg(test)] mod tests;` or
  module-specific `tests/` submodules;
- keep tests next to the code they validate, not in broad integration buckets;
- preserve access to private internals using sibling module layout, not by
  widening production visibility unless unavoidable;
- update any `include_str!` relative paths;
- rerun line-count tripwires and record deltas.

Non-goals:

- no production logic moves except the tiny `mod tests;` declaration;
- no test rewrites beyond path/import repair.

Exit criteria:

- test-heavy facades are no longer counted as large production files;
- no new public items were added only for tests.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml
git diff --check
```

Log:

- Pending.

## ORG-02: Crate-Root And Facade Cleanup

Why: after tests move out, crate roots and facade modules should be boring and
predictable.

Primary targets:

- `native/crates/mclone-server/src/lib.rs`
- `native/crates/mclone-worldgen/src/levelgen.rs`
- `native/crates/mclone-worldgen/src/feature.rs`
- `native/crates/mclone-mesh/src/lib.rs`
- `native/crates/mclone-ui/src/lib.rs`
- `native/apps/mclone-native-client/src/main.rs`

Deliverables:

- make roots and facades consist mostly of `mod`, `pub mod`, `pub use`, crate
  attributes, and short shared helpers;
- move helper functions out of roots when they clearly belong to an existing
  module;
- keep public paths stable through re-exports;
- record any root helper that intentionally remains and why.

Non-goals:

- no deep production decomposition yet;
- no public API cleanup that forces call-site churn across crates.

Exit criteria:

- each touched root has a short "why this remains here" shape that is obvious
  from module names and re-exports;
- no touched root is above `1,000` production LoC unless explicitly recorded.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml
cargo build --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Log:

- Pending.

## ORG-03: `mclone-render-session` Split

Why: `mclone-render-session` is a shared boundary crate and currently combines
mesh extraction, render-section dirty state, ready planning, upload queues,
camera/player movement, server-update dirty classification, compile traits,
packed reports, and tests in one file.

Proposed module shape:

- `mesh_inputs`: snapshot/client to mesh input conversion and actor instance
  extraction;
- `section_cache`: cached textured render-section state and cache updates;
- `upload`: upload queue/coordinator, frame policy, drain reports;
- `dirty`: dirty-state tracking and neighbor readiness helpers;
- `ready_plan`: ready render-section planning and sync plan preparation;
- `compile_queue`: queued compile requests/results, compile traits, packed
  report encoding;
- `camera`: engine camera input, movement modes, pose sync, render pose
  conversion;
- `server_updates`: dirty classification for `ServerUpdate`s;
- `session`: `EngineRenderSession` and `RenderSectionSession`;
- `tests`: module-local tests from ORG-01.

Deliverables:

- split within the crate only;
- preserve existing `mclone_render_session::...` public paths by re-exporting
  from `lib.rs`;
- keep hot compile/render data structs monomorphized as before;
- record any public item whose visibility is tightened.

Non-goals:

- no render compile policy changes;
- no movement/camera behavior changes;
- no worker topology changes.

Exit criteria:

- `lib.rs` becomes a facade;
- no new public path is required by downstream crates;
- release A/B render-session lanes are within recorded noise.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml
pnpm native:mesh-cpu:perf
pnpm native:startup-streaming:persisted:perf
pnpm native:timedemo:perf
pnpm native:web:build
git diff --check
```

Log:

- Pending.

## ORG-04: `mclone-xr-scene` Non-Render Split

Why: the XR scene crate is the largest production module. Start with logic
that is easiest to isolate without touching renderer hot paths.

Proposed module shape:

- `options`: defaults, parsing-facing data, validation helpers;
- `timing`: frame summaries, render/upload/locomotion timing structs and
  accumulation helpers;
- `tracking`: `XrTrackingOrigin`, `XrStageToWorld`, view/pose transforms;
- `locomotion`: controller locomotion input, snap turn, automated movement,
  hand-push mapping;
- `teleport`: blink teleport state, intent, preview line helpers;
- `ui_panels`: menu/game UI panel geometry, pointer hit testing, controller
  rays, panel anchors;
- `comfort`: head comfort state, fade overlays, underwater overlay helpers;
- `session`: local/remote session helpers and XR game UI projection helpers.

Deliverables:

- move non-render helpers and data first;
- keep `XrMcloneTerrainState` render methods in place until ORG-05;
- preserve public exports from `lib.rs`;
- rerun XR unit tests and desktop compile.

Non-goals:

- no render-frame method decomposition yet;
- no OpenXR app changes;
- no locomotion tuning changes.

Exit criteria:

- non-render pure/helper regions are no longer buried below the terrain state
  implementation;
- no public XR helper path breaks downstream crates.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml
pnpm native:xr:check
pnpm native:web:build
git diff --check
```

Log:

- Pending.

## ORG-05: `mclone-xr-scene` Render And State Split

Why: after ORG-04, split the hot and platform-visible XR terrain scene paths
with strong validation.

Proposed module shape:

- `state`: `XrMcloneTerrainState`, startup state, pending session start, upload
  summaries;
- `runtime`: local single-view runtime startup and poll/upload coordination;
- `render_frame`: public frame entry points and shared frame summary assembly;
- `stereo`: per-eye terrain render path;
- `multiview`: full-frame multiview terrain render path;
- `overlays`: sky/actors/world GUI/selection/screen-effect overlay routing;
- `upload`: section upload application and upload diagnostics;
- `actors`: current actor instance and actor-light helpers.

Deliverables:

- split render paths without changing per-eye vs multiview behavior;
- keep per-view uniform ownership invariants intact;
- record an explicit exception if any XR-visible render feature is available
  in one path but not the other;
- preserve public entry points used by desktop XR and Android XR apps.

Non-goals:

- no new renderer features;
- no multiview default-policy changes;
- no upload budget changes.

Exit criteria:

- both stereo and multiview render paths are legible from module names;
- Android XR app builds without adapting to private implementation details;
- device validation is complete or recorded as blocked.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml
pnpm native:xr:check
pnpm native:android-xr:apk
pnpm native:android-xr:session-smoke
pnpm native:android-xr:perf:orbit:rd7:metrics
git diff --check
```

Log:

- Pending.

## ORG-06: Android XR Adapter Split

Why: `mclone-android-xr-client/src/lib.rs` is almost entirely production app
adapter code. It should reveal Android glue, startup parsing, frame loops,
perf reporting, and target acquisition as separate modules.

Proposed module shape:

- `android_glue`: logger, asset root, world root, activity failure reporting,
  property/argv handling, event polling;
- `startup`: CLI/property parsing, startup scene defaults, perf mode parsing,
  render-scale validation;
- `runtime`: runtime asset loading, scene/runtime construction, host options;
- `frame_loop`: main OpenXR mclone frame loop;
- `proof_loops`: clear/multiview/terrain proof and perf loops;
- `perf_report`: frame accounting, marker-line logging, summary formatting;
- `targets`: eye/stereo target acquisition and release helpers;
- existing `graphics_vulkan` and `perf_metrics` stay separate.

Deliverables:

- split inside the app crate only;
- keep Android-specific unsafe allowances tightly scoped to modules that call
  Android/OpenXR APIs;
- do not move shared XR scene behavior into the app;
- preserve existing validator marker strings and command-line/property
  behavior.

Non-goals:

- no Quest performance tuning;
- no OpenXR session ownership changes;
- no manifest/package changes unless required by module paths.

Exit criteria:

- `lib.rs` is a thin Android entry/facade;
- `MCLONE_ANDROID_XR_PERF_*` marker keys are preserved;
- device validation is complete or recorded as blocked.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-android-xr-client
pnpm native:android-xr:apk
pnpm native:android-xr:session-smoke
pnpm native:android-xr:perf:orbit:rd7:metrics
git diff --check
```

Log:

- Pending.

## ORG-07: Web Canvas Adapter Split

Why: `web_canvas.rs` mixes wasm exports, render compiler session, web render
session, local catalog bridge, JS codecs, input mappings, and canvas rendering.

Proposed module shape:

- `exports`: `#[wasm_bindgen]` exported functions and thin dispatch only;
- `compiler_session`: `WebRenderCompilerSession`, shared arena, in-flight
  compile tracking;
- `chunk_session`: `WebChunkRenderSession` and frame stepping;
- `catalog_bridge`: local world catalog request/response preparation;
- `startup_query`: query parsing and startup options JS conversion;
- `js_codec`: `JsValue` getters/setters and serde-ish conversion helpers;
- `render_helpers`: canvas/context setup, target sections, camera JS state,
  atlas upload;
- `input`: GUI key mapping, touch/control labels, movement mode mapping;
- `debug_mesh`: test/debug mesh helpers.

Deliverables:

- keep all wasm exports stable;
- keep TypeScript glue behavior unchanged;
- do not add app-local policy; policy should stay in `mclone-app-runtime`,
  `mclone-ui`, `mclone-input`, or shared render crates;
- preserve worker/shared-memory behavior exactly.

Non-goals:

- no JS/TS rewrite;
- no web worker lifecycle changes;
- no WASM threading changes.

Exit criteria:

- `web_canvas.rs` is a facade/export layer or is replaced by a module tree;
- `native:web:app-smoke` and `native:web:catalog-smoke` pass;
- browser-visible exported function names remain unchanged.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:build
pnpm native:web:typecheck
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
git diff --check
```

Log:

- Pending.

## ORG-08: Chunk Renderer Split

Why: `mclone-render/src/chunk.rs` is a hot shared renderer file containing
multiple stable concepts. Split after render-session/XR scene so call sites
are already cleaner.

Proposed module shape:

- `chunk/view`: `ChunkCamera`, `PerspectiveRenderPose`, `ChunkRenderView`,
  projection validation;
- `chunk/target`: `ChunkRenderTarget`, `ChunkMultiviewRenderTarget`, depth
  targets;
- `chunk/culling`: visibility stats, cull scratch, frustum, stereo draw masks,
  prepared records;
- `chunk/upload`: GPU mesh structs, vertex/index byte conversion, atlas upload,
  upload timing;
- `chunk/draw_resources`: section draw resources and cache application;
- `chunk/pipeline`: render pipeline creation and blend/depth state;
- `chunk/renderer`: flat/textured/single-view/multiview renderer types.

Deliverables:

- preserve public `mclone_render::chunk::...` paths through re-exports;
- keep WGSL/pipeline behavior unchanged;
- avoid changing GPU buffer layout, uniform bytes, culling order, or sort keys;
- record public surface before and after.

Non-goals:

- no draw batching or greedy meshing;
- no multiview behavior changes;
- no color/fog/lighting changes.

Exit criteria:

- no render behavior changes in screenshot/offscreen smokes;
- render benchmark lanes are within recorded noise.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml
pnpm native:desktop-offscreen:smoke
pnpm native:mesh-cpu:perf
pnpm native:gpu-upload:perf
pnpm native:movement-frame:perf
pnpm native:timedemo:perf
pnpm native:web:build
git diff --check
```

Log:

- Pending.

## ORG-09: UI Crate Split

Why: `mclone-ui` has two large files and is becoming the shared UI policy and
draw-list owner for flat, web, Android, XR, and offscreen lanes.

Proposed module shape:

- `geometry`: `GuiScale`, `Point`, `Rect`, `Color`, clipping, UVs;
- `draw_list`: `GuiDrawCommand`, `GuiDrawList`, retained draw-list stats;
- `widgets`: button, checkbox, slider, cycle button, interaction primitives;
- `state`: `GameScreen`, `GameUiRenderState`, options/catalog UI state,
  actions;
- `v2/surface`: `UiSurface`, frame state, revisions, widget IDs;
- `v2/layouts`: title/world-list/create/delete/new/join/pause/options/help
  layouts;
- `v2/hud`: hotbar/status/prompt/debug retained HUD surfaces;
- `v2/loading`: loading progress draw lists and retained loading surface;
- `palette`: block palette layout and tooltip rendering.

Deliverables:

- keep public `mclone_ui::...` exports stable;
- keep UI v2 action behavior unchanged;
- preserve flat/XR/web draw-list consumers;
- move tests beside the modules they validate.

Non-goals:

- no UI redesign;
- no new screens;
- no input policy changes.

Exit criteria:

- `lib.rs` and `v2.rs` no longer carry unrelated UI domains;
- UI tests and web catalog smoke pass.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml
pnpm native:desktop-offscreen:smoke
pnpm native:web:catalog-smoke
git diff --check
```

Log:

- Pending.

## ORG-10: Server Scheduler And Persistence Split

Why: server files are large because they own real coupled systems. Split them
only after test relocation and with Java/server architecture in hand.

Proposed scheduler module shape:

- `scheduler/core`: `ChunkScheduler` type and high-level poll/tick flow;
- `scheduler/metrics`: metrics and event/report data;
- `scheduler/tickets`: interest and ticket reconciliation glue, if not already
  adequately covered by `distance_manager`;
- `scheduler/publication`: completed worldgen/light publication;
- `scheduler/persistence`: load/save request integration;
- `scheduler/fluid_ticks`: scheduler-coupled fluid tick queue and mutation
  application;
- `scheduler/jobs`: feature job creation, priority, status paths;
- `scheduler/tests`: module-local scheduler tests.

Proposed persistence module shape:

- `persistence/records`: chunk/entity/player/world record data;
- `persistence/codec`: binary encode/decode helpers;
- `persistence/stores`: null, memory, snapshot, filesystem, SQLite stores;
- `persistence/actor`: synchronous/external/threaded actor internals;
- `persistence/mailbox`: public `PersistenceMailbox` facade;
- `persistence/tests`: module-local persistence tests.

Deliverables:

- read relevant Java `server/level/*` and persistence-adjacent reference
  sources before finalizing scheduler boundaries;
- split within `mclone-server`;
- preserve public `mclone_server::...` exports;
- keep store behavior and scheduled tick persistence byte-for-byte compatible
  unless a separate migration tactical exists.

Non-goals:

- no scheduler policy changes;
- no save format changes;
- no lighting or fluid behavior changes.

Exit criteria:

- scheduler and persistence internals are navigable by subsystem;
- persisted-memory and persisted-SQLite perf lanes are within recorded noise;
- fixture/oracle-backed server tests still pass.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml
pnpm native:scheduler-loading:perf
pnpm native:scheduler-loading:persisted-memory:perf
pnpm native:scheduler-loading:persisted-sqlite:perf
pnpm native:movement:smoke
git diff --check
```

Log:

- Pending.

## ORG-11: Worldgen And Table Exceptions

Why: worldgen remains one of the largest shared crates, but some size is
intentional: reference-parity code and tables can be clearer when kept close
to the Java shape.

Primary targets:

- `native/crates/mclone-worldgen/src/biome.rs`
- `native/crates/mclone-worldgen/src/feature/tables.rs`
- `native/crates/mclone-worldgen/src/feature/tree.rs`
- `native/crates/mclone-worldgen/src/feature/configured.rs`
- `native/crates/mclone-worldgen/src/carver.rs`
- `native/crates/mclone-worldgen/src/noise.rs`
- `native/crates/mclone-worldgen/src/surface.rs`
- `native/crates/mclone-worldgen/src/block.rs`

Deliverables:

- classify each remaining large worldgen file as split, table/reference
  exception, or needs a future parity tactical;
- for split files, follow Java 1.17.1 package boundaries first;
- for exception files, add a short module-level comment explaining why size is
  intentional and what would justify revisiting it;
- do not port disabled Caves & Cliffs Part 1 paths.

Non-goals:

- no new worldgen parity port;
- no fixture updates unless import paths change;
- no table regeneration.

Exit criteria:

- no unclassified worldgen production file remains above `1,500` LoC;
- all explicit exceptions are documented in this section's log.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml
pnpm native:worldgen:perf
pnpm native:scheduler-loading:perf
git diff --check
```

Log:

- Pending.

## ORG-12: Secondary Large-File Cleanup

Why: after the highest-risk modules are split, clean up the remaining
production files above the soft threshold without losing momentum to tiny
cosmetic moves.

Candidate targets:

- `native/apps/mclone-native-client/src/perf.rs`
- `native/apps/mclone-native-client/src/flat_client_driver.rs`
- `native/apps/mclone-native-client/src/cli.rs`
- `native/apps/mclone-native-client/src/headless.rs`
- `native/apps/mclone-native-client/src/xr_clear_smoke.rs`
- `native/crates/mclone-app-runtime/src/lib.rs`
- `native/crates/mclone-app-runtime/src/local_single_view.rs`
- `native/crates/mclone-app-runtime/src/frame_render.rs`
- `native/crates/mclone-client/src/player.rs`
- `native/crates/mclone-client/src/teleport.rs`
- `native/crates/mclone-input/src/lib.rs`
- `native/crates/mclone-protocol/src/lib.rs`
- `native/crates/mclone-mesh/src/builder.rs`

Deliverables:

- re-run production line-count inventory and choose a batch of closely related
  targets;
- split only when a domain boundary is clear;
- prefer consolidating shared benchmark/report helpers into the existing
  diagnostics/perf owner rather than creating app-local duplicates;
- record any file intentionally left above threshold.

Non-goals:

- no new feature work;
- no "one file per type" churn where the existing file is cohesive.

Exit criteria:

- all remaining production files above `1,500` LoC are either split or
  explicitly justified;
- app-local code growth is not hiding shared policy.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml
pnpm native:perf:smoke
pnpm native:web:build
git diff --check
```

Log:

- Pending.

## ORG-13: Closeout Audit

Why: close the tactical only after the codebase shape, tests, and performance
evidence line up.

Deliverables:

- rerun all baseline and tripwire commands;
- record final total LoC, files over `1,000`, production files over `1,000`,
  and explicit exception list;
- run the broad validation matrix;
- update `035-native-codebase-health-refactor-plan.md` with a pointer to this
  tactical's closeout result if needed;
- update this status to closed only after validation is recorded.

Closeout validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml
pnpm native:perf:smoke
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
pnpm native:web:app-smoke
```

Optional hardware/device closeout, if available:

```bash
pnpm native:android:avd-session-smoke
pnpm native:android-xr:session-smoke
pnpm native:android-xr:perf:orbit:rd7:metrics
```

Exit criteria:

- no unexplained benchmark regression;
- no unexplained device validation regression;
- all exceptions are documented;
- status table shows every slice landed, intentionally skipped, or superseded.

Log:

- Pending.

## Open Questions

- Should this tactical target a strict maximum production file size for new
  code after closeout, or keep the current soft threshold plus exception model?
- Should `cargo clippy` become a formal gate after the organization pass, or
  should it remain separate until existing warnings are audited?
- Should benchmark/report helper cleanup land under this tactical's ORG-12, or
  under a diagnostics-specific successor to tactical 144?

## Follow-Up Queue

- Add a lightweight repository policy check for production files above the soft
  threshold once the exception list is stable.
- Consider a short `docs/native-rust-organization.md` durable guide after the
  tactical closes, if the module conventions prove useful beyond this cleanup.
