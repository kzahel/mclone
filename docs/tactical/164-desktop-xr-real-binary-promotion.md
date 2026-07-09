# 164: Desktop XR Real-Binary Promotion

Status: proposed 2026-07-09.

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
  (`xr_clear_smoke.rs:636-647`) because some runtimes fault while destroying
  session-owned graphics handles after EXITING. Fine for a run-to-count harness;
  a persistent app needs a defined quit path.
- **No desktop presence.** Desktop XR spawns no window. The only display surface
  is the headset; the only ways to end a session are the headset system menu
  (OpenXR EXITING, handled at `xr_clear_smoke.rs:501-504`, but only honored in
  the unbounded path), the frame limit, or Ctrl-C in the terminal. There is no
  taskbar/dock presence and no non-headset quit.
- **Module name signals "test."** The whole path lives in
  `native/apps/mclone-native-client/src/xr_clear_smoke.rs`, dispatched as
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

### Slice 1 — Rename/relocate the module for authority

- Rename `native/apps/mclone-native-client/src/xr_clear_smoke.rs` →
  `desktop_xr.rs` and update `mod` + `use` sites (`main.rs:24`, dispatch at
  `main.rs:293-294`, and the feature-gated shims at `main.rs:364-382`).
- Keep the clear/mclone/real distinction as an internal enum
  (`DesktopXrSmoke` grows a `Real`/persistent variant, or is renamed to a
  `DesktopXrMode` covering all three intents).
- Pure rename/move slice: no behavior change, keeps the diff reviewable.

### Slice 2 — First-class CLI verb + real defaults

- Add a real run verb (e.g. `--desktop-xr`, or plain `--xr`) producing a new
  `Cli::DesktopXr` that defaults to **persistent** (unbounded) mclone play.
- Preserve the existing validation flags verbatim: `--xr-clear-smoke`,
  `--xr-mclone-smoke`, `--frames N` (still capped for CI), `--xr-forever`, plus
  the `--view-pose` / `--xr-underwater-mode` / `--xr-debug-ui` modifiers and
  their guards (`cli.rs:1180-1196`).
- Add an explicit **`--headless` / `--no-window`** flag (name TBD) so any real
  or smoke run can suppress the companion window. Headless is the default for
  the bounded smoke gates; window is the default for the real verb.
- Decide the `MAX_XR_SMOKE_FRAMES` story: keep the 4096 cap for the *smoke*
  frame budget (it is a CI bound), but the real verb is not frame-bounded at
  all, so the cap no longer limits real sessions.

### Slice 3 — Companion window + graceful quit

- Spawn a small 2D desktop window for the real verb (winit, app-local — this is
  legitimate desktop platform glue and stays in the app crate per the
  shared-first policy). Minimum contents: status/connection line and a **Close**
  control so the user can quit without the headset. Errors/handshake state can
  surface here too.
- Wire a defined quit path: window close request and headset-menu EXITING both
  drive the same graceful shutdown. In the persistent mode, treat headset-menu
  exit as a normal end (not the `bail!` used by the bounded path at
  `xr_clear_smoke.rs:498-500`).
- Revisit the `std::mem::forget` teardown (`xr_clear_smoke.rs:636-647`): keep it
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

- Current path: `native/apps/mclone-native-client/src/xr_clear_smoke.rs`
  (→ `desktop_xr.rs`), dispatch `main.rs:293-294`, feature shims
  `main.rs:364-382`, flags/guards `cli.rs`.
- Shared frame interior: `mclone-xr-scene`, `mclone-xr-host`;
  `docs/frame-pipeline-accounting.md` (desktop-XR host drives the same interior
  as Android XR).
- Launchers: `scripts/start-xr.sh`, `scripts/start-xr.ps1`,
  `scripts/start-xr.bat`, `scripts/start-desktop-xr.bat`; `package.json`
  `native:xr:*`.
- Prior desktop XR slices: `078` (clear smoke), `079` (mclone frame), `080`
  (startup view pose), `081` (controller actions), `082` (locomotion).
- Role framing: `tactical/153` §Desktop XR Role, `tactical/150` §Desktop XR Role.
