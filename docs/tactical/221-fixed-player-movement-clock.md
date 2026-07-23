# 221: Fixed Player Movement Clock

Status: complete 2026-07-23.

Topic: client-prediction

## Motivation

Local walking, flying, and no-clip movement were integrated once per render
frame. Desktop, flat Android, and browser adapters also capped the elapsed time
they passed to movement at 50 ms. A visible 100 ms frame therefore simulated
only half of its elapsed movement, making low-frame-rate play slow. Walking's
vanilla-style impulse/drag recurrence also used `dt * 20` as an approximation,
so merely removing the host caps would not make different rates equivalent.

The requested direction is a real high-rate player lane rather than another
architecture note:

- default local-player movement to 60 Hz;
- do not force player movement to the 20 Hz world/AI cadence;
- keep input receipt, player simulation, pose publication, and rendering as
  separate clocks;
- make the behavior shared across desktop, browser, Android, and XR;
- preserve a clean path to sequenced client prediction and authoritative host
  replay.

## Landed Contract

`mclone-scene` owns `PlayerMovementCadenceConfig` and one movement state per
local participant. The default is 60 Hz with at most 12 admitted catch-up steps
per presentation callback. Hosts supply uncapped visible elapsed time and the
latest semantic input; they do not choose simulation `dt`. Hidden/background
transitions reset elapsed state, so time spent suspended is never replayed.

The movement owner:

- turns elapsed time into fixed `1/60` quanta with integer nanosecond
  accumulation;
- preserves ordinary movement through presentation rates down to 5 FPS;
- records and reports work dropped after a longer pathological stall;
- retains a jump edge observed between movement quanta;
- applies look immediately at presentation rate while applying body
  translation at the fixed rate;
- interpolates the rendered eye between the previous and current fixed poses;
- snaps on lifecycle changes, teleports, authoritative corrections, warm-world
  swaps, and movement/collision-mode changes.

Desktop keyboard/mouse and native touch changes arrive as platform events.
Desktop/Android controller collectors and the browser Gamepad API produce
snapshots near presentation boundaries. The fixed movement clock consumes the
most recently observed semantic state; it does not pretend that snapshot-only
APIs were polled while the presentation thread was stalled.

XR tracked head/hand poses and view matrices remain tied to the OpenXR frame,
as they should. Only the player body locomotion is fixed-rate. Camera look and
smooth-turn deltas remain presentation-rate inputs and are applied once, not
once per catch-up quantum.

## Movement Math

Walking and water movement retain the existing 20 Hz vanilla recurrence as
their reference behavior, but use a fractional affine iterate for smaller fixed
steps. Three unobstructed 60 Hz steps therefore compose to one 20 Hz step for
position and velocity. Collision is intentionally tested at every 60 Hz
quantum. Fly/no-clip movement already uses blocks per second and now preserves
sub-unit analog stick magnitude rather than normalizing every non-zero vector
to full throttle.

## Clock Separation

The default lanes after this slice are:

| Lane | Default | Owner |
|---|---:|---|
| Local player movement | 60 Hz | `mclone-scene` |
| Player pose publication | at most 20 Hz | `mclone-scene` |
| Integrated host/world/physics | 20/20/60 Hz | server cadence profile |
| AI decisions | gameplay policy, currently world-tick-shaped | server |
| Look and XR tracking | presentation frame | scene/platform input boundary |
| Rendering/interpolation | presentation frame | render host |

Changing the server cadence does not implicitly change local movement or
publication. `PlayerMovementCadenceConfig` is explicit scene profile data so a
later profile can choose another clean quantum without app-local policy.

## Preserved Follow-up

This is the first client movement-clock slice, not completed shooter-style
netcode. The clock currently consumes the latest semantic state during bounded
catch-up. It retains jump edges, but cannot reconstruct the exact timestamps of
arbitrary held-axis changes inside a long presentation interval.

The next authoritative-prediction step is to materialize each fixed quantum as
a sequenced command record, queue records in input-time order, make the host
drain the same records, acknowledge the last applied sequence, and replay
unacknowledged commands after correction. Browser worker ownership can then run
that same command clock continuously without inventing a browser-only movement
model.

## Validation

Automated validation covered scheduler counts at 60, 10, and 5 FPS, bounded
long-stall dropping, jump-edge retention, interpolation, 20-versus-60 Hz free
walking equivalence, analog no-clip throttle, and structural rejection of the
old desktop/Android elapsed caps.

The following boundaries passed:

- `cargo test -p mclone-client --lib`;
- `cargo test -p mclone-render-session`;
- `cargo test -p mclone-scene --no-default-features`;
- `cargo check -p mclone-native-client --features xr`;
- `cargo check --target wasm32-unknown-unknown -p mclone-web-client`;
- `cargo check -p mclone-android-client -p mclone-android-xr-client`;
- `pnpm native:thin-adapters:purity`;
- flat Android and Quest XR APK builds;
- the headed-Wayland browser movement performance smoke, which crossed six
  chunks while compiling and reported a 60 Hz player movement rate with no
  dropped steps;
- desktop offscreen and XR emulation production smokes.

The browser WebGPU probe, movement capture, desktop offscreen capture, and XR
emulation capture were inspected and contained valid rendered pixels. A full
workspace test run passed the movement-related crates and reached one failure
in the concurrently modified Mclone-overworld baked-liquid stencil test; that
unrelated work is outside this tactical and commit.
