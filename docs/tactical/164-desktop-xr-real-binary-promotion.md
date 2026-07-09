# 164: Desktop XR Real-Binary Promotion

Status: active 2026-07-09; Slices 1 (module rename) + 2 (real `--desktop-xr`
verb, persistent default, `--no-window`) landed.

Workstream: native Rust desktop platform glue (`mclone-native-client`) plus the
shared XR frame interior it already drives. Desktop validation first; the frame
interior stays shared with Android/Quest XR, and headless/CI paths must survive
the promotion unchanged.

## Purpose

Promote desktop OpenXR from a frame-bounded "smoke" harness into a first-class,
persistent **desktop XR run mode**, without giving up the headless/bounded modes
that CI and the `native:xr:*` gates depend on.

The engine work is already done. Desktop XR is not a reduced or fake engine: the
`--xr-mclone-smoke` interior calls the same `mclone-xr-scene` terrain frame path
that Android/Quest XR uses (`docs/frame-pipeline-accounting.md`), and user
validation confirms it "works perfectly" with `--smoke mclone` on the Mac WiVRn
lane. What still reads as a harness is naming, defaults, lifecycle, and the lack
of any desktop presence. This slice closes that gap.

Scope for this doc is the "make it feel like a real binary" surface. The
**mirror view** (rendering the headset image into the companion window) is
explicitly deferred to a follow-up slice — see Non-Goals.

## Current State (why it reads as "smoke")

- **It is a CLI mode, not a separate binary.** Everything runs through
  `mclone-native-client --features xr`; the app arg selects the interior:
  `--xr-clear-smoke` (per-eye diagnostic clear) vs `--xr-mclone-smoke` (the real
  world). There is no `[[bin]]` and no first-class "run desktop XR" verb.
- **Defaults are validation defaults.** `DEFAULT_XR_CLEAR_SMOKE_FRAMES = 120`
  and `MAX_XR_SMOKE_FRAMES = 4096` in
  `native/apps/mclone-native-client/src/cli.rs:39-40`. A real session should run
  until the user quits; today the only unbounded path is `--xr-forever`
  (`cli.rs:1116-1118`), which cannot be combined with `--frames`
  (`cli.rs:1186-1187`).
- **The launch script can't run persistently.** `scripts/start-xr.sh` always
  injects `--frames "${frames}"` (lines 448/450) and defaults `--smoke clear`
  (line 267), so it cannot reach `--xr-forever` and defaults to the blue/green
  diagnostic clear, not the world. `--frames` is also capped at 4096 (~45-60s at
  headset frame rates), which is why "forever" is currently unreachable via the
  script.
- **Teardown is process-bounded.** `run_smoke_frames` deliberately
  `std::mem::forget`s the whole XR object graph after a clean shutdown
  (`desktop_xr.rs:636-647`) because some runtimes fault while destroying
  session-owned graphics handles after EXITING. Fine for a run-to-count harness;
  a persistent app needs a defined quit path.
- **No desktop presence.** Desktop XR spawns no window. The only display surface
  is the headset; the only ways to end a session are the headset system menu
  (OpenXR EXITING, handled at `desktop_xr.rs:501-504`, but only honored in
  the unbounded path), the frame limit, or Ctrl-C in the terminal. There is no
  taskbar/dock presence and no non-headset quit.
- **Module name signals "test."** The whole path lives in
  `native/apps/mclone-native-client/src/desktop_xr.rs`, dispatched as
  `Cli::XrClearSmoke` / `Cli::XrMcloneSmoke` in `main.rs:293-294`.

## Design Principle

One code path, three run intents, selected by flags — not three code paths:

```text
desktop XR frame loop (shared mclone-xr-scene interior)
  real mode      -> persistent, companion window, graceful quit   [new default verb]
  bounded smoke  -> --frames N, headless-capable, CI liveness gate [preserve]
  clear smoke    -> per-eye diagnostic clear, no world             [preserve]
```

Headless / no-window must remain an explicit, first-class option so bounded
smoke tests keep running without spawning a window. The window is a property of
the *real* run intent, not of desktop XR as such.

## Scope / Slices

Ordered so each slice lands independently and keeps `native:xr:*` green.

### Slice 1 — Rename/relocate the module for authority — LANDED 2026-07-09

- Renamed `native/apps/mclone-native-client/src/xr_clear_smoke.rs` →
  `desktop_xr.rs` and the sibling `xr_clear_smoke/` graphics dir
  (`graphics_metal.rs`, `graphics_vulkan.rs`) → `desktop_xr/`, via `git mv` to
  preserve history. Updated the three `main.rs` sites: `mod desktop_xr;`
  (line 24) and the two feature-gated call sites `desktop_xr::run` /
  `desktop_xr::run_mclone` (lines 366/376).
- Pure rename/move: no behavior change. The `--xr-clear-smoke` /
  `--xr-mclone-smoke` CLI flags, the `Cli::XrClearSmoke` variant, and the
  `xr_clear_smoke` boolean in `cli.rs` are intentionally unchanged — flag/verb
  work is Slice 2. The internal `DesktopXrSmoke` enum keeps its clear/mclone
  shape here; the `Real`/persistent variant lands in Slice 2.
- Verified: `cargo check -p mclone-native-client --features xr` clean (only the
  pre-existing `DESKTOP_LOCAL_ARG_FLAGS` dead-code warning), and the 14
  `tests::cli_xr` tests pass.
- Deferred to Slice 4: living docs still referencing the old path
  (`docs/frame-pipeline-accounting.md:202`, `tactical/150:386`) get updated with
  the rest of the doc pass. Historical tactical records (081, 083, 086, 130,
  144, 145, 158) are left as-is.

### Slice 2 — First-class CLI verb + real defaults — LANDED 2026-07-09

- Added the `--desktop-xr` run verb producing a new `Cli::DesktopXr { options,
  window }`. It renders the same mclone world as `--xr-mclone-smoke` but is
  **persistent by default** (unbounded frames): only an explicit `--frames N`
  bounds it; `--xr-forever` and the no-flag default both leave it unbounded.
  The verb reuses `XrMcloneSmokeOptions` for the shared world content rather
  than duplicating a struct.
- Internal enum `DesktopXrSmoke` → `DesktopXrMode` with a new `Real { options,
  window }` variant beside `Clear`/`Mclone`; `desktop_xr::run_desktop` is the
  new entry. The `Real` arm renders the identical world path as `Mclone` and
  threads `window` through to `run_smoke_frames` for Slice 3 to consume (it is a
  deliberate no-op here, `let _ = window;`).
- Companion-window flag: chose **`--no-window`** (not a bare `--headless`) to
  avoid collision with the existing `--headless-clear` / `--headless-dual-view`
  headless *modes*. It suppresses the Slice 3 window; `window` defaults to
  `true` for the real verb. `--no-window` requires `--desktop-xr` (the smoke
  gates are already windowless, so it is meaningless there).
- Preserved every validation flag verbatim: `--xr-clear-smoke`,
  `--xr-mclone-smoke`, `--frames N` (still capped at `MAX_XR_SMOKE_FRAMES` =
  4096 for the smoke frame budget), `--xr-forever`. The `--frames` /
  `--xr-forever` / `--view-pose` / `--xr-underwater-mode` / `--xr-debug-ui`
  guards now also accept `--desktop-xr` (messages updated); the combined-mode /
  window-report / menu / startup-wait guards now also reject combining
  `--desktop-xr` with headless/perf modes. The "XR smoke modes cannot be
  combined…" message became "XR modes…" since the real verb is not a smoke.
- `MAX_XR_SMOKE_FRAMES` (4096) stays as the *smoke* CI bound. The real verb is
  not frame-bounded, so that cap no longer limits real sessions — persistent is
  the default and `--frames N` is opt-in.
- Registered `--desktop-xr` and `--no-window` in `DESKTOP_LOCAL_ARG_FLAGS` and
  refreshed the `--help` usage. `main.rs` gained the `Cli::DesktopXr` dispatch
  plus feature-gated shims (`--features xr` → `desktop_xr::run_desktop`;
  otherwise the friendly "rebuild with `--features xr`" bail).
- Verified: `cargo check -p mclone-native-client` clean both with and without
  `--features xr`; all crate tests pass (181), including 8 new `tests::cli_xr`
  cases (persistent+window default, `--no-window`, `--frames` bound,
  `--xr-forever`, mclone modifiers, and the three rejection guards) and the
  `desktop_cli_flags_are_classified_as_shared_or_desktop_local` scan test. Ran
  the non-xr binary to confirm `--desktop-xr` bails with the rebuild hint and
  `--no-window` alone hits its guard.

### Slice 3 — Companion window + graceful quit

- Spawn a small 2D desktop window for the real verb (winit, app-local — this is
  legitimate desktop platform glue and stays in the app crate per the
  shared-first policy). Minimum contents: status/connection line and a **Close**
  control so the user can quit without the headset. Errors/handshake state can
  surface here too.
- Wire a defined quit path: window close request and headset-menu EXITING both
  drive the same graceful shutdown. In the persistent mode, treat headset-menu
  exit as a normal end (not the `bail!` used by the bounded path in
  `desktop_xr.rs` `run_smoke_frames`). The `Real` arm already threads a `window`
  bool into `run_smoke_frames`; consume it here to build/skip the window.
- Revisit the `std::mem::forget` teardown in `desktop_xr.rs` `run_smoke_frames`: keep it
  only where a runtime genuinely faults on handle destruction; otherwise perform
  a real teardown for the persistent path, or document per-runtime why the
  forget-on-exit shape stays. Process exit on quit is acceptable as an interim.
- **Multiview/per-eye guardrail:** this slice adds no new per-view render path.
  The companion window starts as a status surface only; a mirror image is
  Slice 4 / follow-up and must reuse an existing renderer, not a new per-eye
  path (see the XR render-path guardrail in `CLAUDE.md`).

### Slice 4 — Launcher + docs

- `scripts/start-xr.sh`: add a persistent/real launch path that does **not**
  inject `--frames` (so `--xr-forever` / the real verb is reachable), default
  the interactive launcher to `--smoke mclone`, and add a `--headless` / window
  passthrough. Keep the bounded `--frames N` path for CI. Mirror the equivalent
  on `scripts/start-xr.ps1` / `start-xr.bat` / `start-desktop-xr.bat` and add a
  `pnpm` script for the real run alongside the existing `native:xr:*` gates.
- Docs: update `docs/platforms.md` (the Desktop OpenXR row, line ~20, currently
  "active XR lane" described only via smoke gates) to describe the real desktop
  XR run mode plus the retained smoke/headless gates; refresh the CLI usage
  strings (`cli.rs:1880-1881`); and update this doc + the README index as slices
  land.

## Non-Goals / Follow-ups

- **Mirror view** — rendering the headset image (or a spectator view) into the
  companion window. Deferred to a follow-up slice; must route through an
  existing renderer with both per-eye and multiview paths, never a new
  per-eye-only path.
- **Android/Quest packaging** — unchanged. Quest standalone stays the numeric
  authority (`tactical/153`, `150`); desktop XR remains the behavior reference
  and now also a usable desktop client. The bounded smoke stays the per-promotion
  "rot insurance" gate.
- **New gameplay** — no new engine/gameplay behavior; this is naming, defaults,
  lifecycle, and desktop presence only.

## Validation

- `pnpm native:xr:check` and the bounded `--frames` gates must stay green
  (headless, no window) after every slice — this is the regression bar that the
  promotion must not break.
- Real verb, Mac WiVRn: `scripts/start-xr.sh` persistent path renders the mclone
  world, the companion window appears with a working Close control, and quitting
  from both the window and the headset menu shuts down cleanly.
- Windows VDXR: equivalent persistent launch via `start-desktop-xr.bat`.
- Per the native validation policy, capture the companion window and (later) the
  headset frame at the first drawable milestone of each visual slice, saved to
  `/tmp`, not the repo.

## References

- Current path: `native/apps/mclone-native-client/src/desktop_xr.rs` (+
  `desktop_xr/graphics_{metal,vulkan}.rs`), dispatch `main.rs:293-294`, feature
  shims `main.rs:364-382`, flags/guards `cli.rs`.
- Shared frame interior: `mclone-xr-scene`, `mclone-xr-host`;
  `docs/frame-pipeline-accounting.md` (desktop-XR host drives the same interior
  as Android XR).
- Launchers: `scripts/start-xr.sh`, `scripts/start-xr.ps1`,
  `scripts/start-xr.bat`, `scripts/start-desktop-xr.bat`; `package.json`
  `native:xr:*`.
- Prior desktop XR slices: `078` (clear smoke), `079` (mclone frame), `080`
  (startup view pose), `081` (controller actions), `082` (locomotion).
- Role framing: `tactical/153` §Desktop XR Role, `tactical/150` §Desktop XR Role.
