# 164: Desktop XR Real-Binary Promotion

Status: complete 2026-07-09; all four slices landed — 1 (module rename), 2 (real
`--desktop-xr` verb, persistent default, `--no-window`), 3 (companion window,
graceful quit), 4 (launcher persistent path + docs). Follow-up outside this doc:
the headset mirror into the companion window (see Non-Goals).

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

### Slice 3 — Companion window + graceful quit — LANDED 2026-07-09

- New app-local module `desktop_xr/companion_window.rs` owns a winit
  `EventLoop` + `Window` (`CompanionWindow`). This is legitimate desktop
  platform glue and stays in the app crate per the shared-first policy; no
  engine/gameplay/render policy moved into the app. The XR frame loop owns
  timing, so the window is drained non-blockingly with winit's
  `pump_app_events(Some(Duration::ZERO))` once per XR frame rather than running
  its own blocking event loop (the flat client uses `run_app`; the XR path
  cannot, since the XR frame loop is the driver).
- **Status surface is the window title** — decision: the live status line is
  carried on the window title (`"Desktop XR — connecting to headset…"` →
  `"… running · close this window to quit"` → `"… shutting down…"`), and the OS
  window-close control (title-bar close / Cmd-W) is the **Close** affordance.
  This gives a real desktop presence (dock/menu-bar entry as
  `mclone-native-client`) and a non-headset quit with **zero new dependencies
  and no renderer** — deliberately avoiding a second wgpu surface or a text
  renderer for what is only a status surface this slice. The window body is
  intentionally empty (default system background). Title updates are debounced
  (`set_status` no-ops when unchanged) so we do not thrash the OS title bar at
  headset frame rate.
- **Consumed the `window` bool** in `run_smoke_frames`: `want_window` is captured
  by borrow before the `mode` match moves `options`, and only
  `Real { window: true, .. }` builds a `CompanionWindow`. `--no-window` and both
  smoke/clear gates stay windowless, so the headless CI paths are byte-for-byte
  unchanged. If window creation fails it degrades to a windowless run with a
  printed note rather than aborting.
- **Graceful quit, one shutdown path:** the companion is pumped at the *top* of
  the frame loop (so a title-bar close quits even while still waiting for the
  headset session to reach READY, where the idle branch `continue`s). A window
  close sets `window_requested_exit` and `break`s; because `runtime_requested_exit`
  stays false, the loop falls into `shutdown_openxr_session` (app-driven
  `request_exit` → the STOPPING→IDLE→EXITING handshake). The headset-menu path
  (`OpenXrPollStatus::Exit` in the unbounded branch) already broke cleanly with
  `runtime_requested_exit = true` and skips the redundant request; Slice 3
  confirmed that and wired the window path to converge on the same teardown.
  The bounded/smoke `bail!("…requested exit before smoke completed")` remains
  guarded by `frame_limit.is_some()` and is untouched.
- **`std::mem::forget` teardown decision:** kept, with the comment rewritten to
  own the rationale. Both the real run and the bounded smokes exit the process
  immediately after `run_smoke_frames` returns, so a graceful EXITING handshake
  followed by process teardown is the defined quit path. We still `mem::forget`
  the XR object graph (eyes/stage/actions/graphics/terrain) rather than dropping
  it because the runtimes this path drives — the macOS WiVRn/Monado self-port and
  Windows VDXR — fault inside the runtime when session-owned graphics handles are
  destroyed after an app-requested EXITING. Real per-handle teardown stays
  deferred until a runtime is validated safe to drop post-EXITING; process exit
  reclaims the memory regardless. The completion label was corrected from
  `"…mclone smoke complete"` to `"…run complete"` for the real verb. The
  companion window is **not** forgotten — it drops normally.
- **Multiview/per-eye guardrail:** honored — no new per-view render path, no new
  renderer, no shader/uniform/pass. The window is a status surface only; the
  headset mirror stays a follow-up that must reuse an existing per-eye/multiview
  renderer (see the XR render-path guardrail in `CLAUDE.md`).
- **Validated (Mac WiVRn USB, headset connected):** `cargo check -p
  mclone-native-client` clean both with and without `--features xr`; all 181
  crate tests pass. Ran the real `--desktop-xr` verb persistently against the
  live WiVRn stack: the companion window appeared titled
  `"Desktop XR — running · close this window to quit"` while the shared mclone
  world rendered (10,625 real frames, 593 sections, 7 actors). Clicking the
  window Close control drove the graceful STOPPING→IDLE→EXITING shutdown and a
  clean `rc=0` — confirmed twice, including while the headset session was paused
  (headset disconnected). Observed that a headset disconnect does **not** end the
  persistent run: WiVRn holds the session at SYNCHRONIZED for reconnect, so the
  run correctly survives it. A *genuine* runtime-initiated EXITING (the physical
  headset system-menu quit) could not be synthesized remotely this session
  (client force-stop only pauses the session, and the button press is physical),
  but that branch is pre-existing and reaches the same `shutdown`-less clean
  break plus the teardown exercised end-to-end by the window path; a with-headset
  manual menu-quit check is the only residual runtime verification.
- Launcher passthrough was **not** added (kept for Slice 4 per scope); the run
  was driven by bringing the WiVRn host/adb/Quest client up manually and
  launching the binary directly with the WiVRn runtime env.

### Slice 4 — Launcher + docs — LANDED 2026-07-09

- **`scripts/start-xr.sh`**: added `--desktop-xr` (and `--no-window`) script
  flags selecting the real verb. The real path emits `--desktop-xr` and does
  **not** inject `--frames` (persistent by default); an explicit `--frames N`
  still bounds a validation run, and `--no-window` passes through. Guards mirror
  the CLI: `--desktop-xr` cannot combine with `--smoke` (it always renders the
  mclone world), and `--no-window` requires `--desktop-xr`. The bounded smoke
  path (`--smoke clear|mclone --frames N`) is untouched, so the `native:xr:*` CI
  gates keep their exact args. `--wivrn-usb --desktop-xr` brings the WiVRn host
  up and the cleanup trap tears it down on quit.
  - Decision: I did **not** flip the global `--smoke clear` default to `mclone`.
    "Default the interactive launcher to mclone" is satisfied by `--desktop-xr`
    always rendering the world; flipping the smoke default would silently change
    the CI gates that call `start-xr.sh` without `--smoke`. The interactive real
    run is `--desktop-xr`; smoke defaults stay conservative.
- **`scripts/start-xr.ps1`**: added `-DesktopXr` / `-NoWindow` switches with the
  same semantics (no `--frames` unless `-Frames` explicit; guards for
  `-DesktopXr`+`-Smoke` and `-NoWindow` without `-DesktopXr`).
- **`scripts/start-xr.bat`**: added `--desktop-xr`/`--no-window` (and PascalCase
  aliases) passthrough to the PowerShell launcher, plus help/example lines.
- **`scripts/start-desktop-xr.bat`**: the Windows interactive launcher now calls
  `--vdxr --desktop-xr` instead of `--vdxr --mclone --forever`, so it uses the
  real verb (companion window + non-headset quit) rather than the windowless
  persistent smoke.
- **`package.json`**: added `native:xr:mac:wivrn:desktop`
  (`start-xr.sh --runtime wivrn --wivrn-usb --desktop-xr`) and a generic
  `native:xr:desktop`; the existing `native:xr:windows:interactive`
  (`start-desktop-xr.bat`) now drives the real verb. All bounded `native:xr:*`
  gates are unchanged.
- **Docs**: `docs/platforms.md` Desktop OpenXR row now describes the two run
  intents (real persistent `--desktop-xr` run vs the retained bounded
  smoke/headless gates) and the single graceful quit, and the validation block
  gained the real-run commands. The CLI `--help` string (already accurate from
  Slice 2) gained the quit-path sentence. The living cross-refs to the old
  module path were repointed: `docs/frame-pipeline-accounting.md` →
  `desktop_xr.rs`/`run_smoke_frames`, `tactical/150` §386 →
  `desktop_xr::run_mclone` / `run_desktop`. Historical tactical records
  (081/083/086/130/144/145/158) are intentionally left as-is.
- **Validated (Mac WiVRn USB, headset connected):** `bash -n start-xr.sh` clean;
  the new guards reject `--desktop-xr --smoke mclone` and `--no-window` alone;
  `start-xr.sh --desktop-xr --check-only` still just checks. End-to-end via the
  launcher: `start-xr.sh --wivrn-usb --desktop-xr --frames 120` printed
  "Starting mclone desktop XR run (bounded to 120 frames)", rendered the world
  (165 sections, 2 actors), hit EXITING and "run complete"; and
  `start-xr.sh --wivrn-usb --desktop-xr` (no `--frames`) ran unbounded with the
  companion window up and the WiVRn host reaped by the trap on exit. The Windows
  launcher edits (`start-xr.ps1`/`.bat`/`start-desktop-xr.bat`) are mechanical
  mirrors and were **not** run this session (no Windows host); a Windows VDXR
  pass via `native:xr:windows:interactive` is the residual check.

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
