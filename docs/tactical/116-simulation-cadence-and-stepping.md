# 116: Simulation Cadence and Stepping

Status: active; initial architecture note recorded, 60 Hz physics substeps landed
inside the existing 20 Hz server tick, the first shared server cadence primitive
landed, cadence configs now reject uneven fractional lane pacing by default, and
the native server runner now consumes separate gameplay and physics lane work
while preserving default 20 Hz gameplay behavior. Native local play now exposes
a developer cadence profile option that derives the runner host interval from
the selected host rate, and the native runner can apply a new cadence profile
while running. The shared in-game options UI now exposes local integrated-server
cadence controls for host, world tick, and physics rates; native flat local
captures validate the settings screen at the default 20/20/60 profile. Tactical
221 separately lands the first 60 Hz client-player movement lane; it does not
change the default 20 Hz world/gameplay lane or 20 Hz pose-publication lane.

## Purpose

Keep Minecraft-compatible gameplay timing where it matters without letting one
global 20 Hz tick become the permanent cadence for physics, AI, networking, and
render presentation.

Minecraft Java's simple 20 TPS model is useful for vanilla parity, scheduled
block/fluid behavior, deterministic chunk work, and gameplay rules. It is a poor
fit for rigid-body physics, high-speed collisions, camera/render smoothness, and
eventual AI or networking policies that naturally want different cadences.

The target architecture is a configurable cadence profile with named fixed-rate
lanes. A higher host cadence must not automatically make every lane run more
often, but the gameplay lane itself is also configurable: 20 Hz is the
Minecraft-like default, not an architectural ceiling.

## Cadence Lanes

- Host/server pump: configurable long-term, with 20/30/60 Hz as plausible
  modes. This owns wall-clock pacing and command intake, not gameplay policy.
- Gameplay lane: default 20 Hz for the Minecraft-like profile. Scheduled block
  ticks, fluid ticks, random ticks, entity age, and parity-sensitive world rules
  belong here. Custom/high-fidelity profiles may raise this lane, such as a
  60 Hz gameplay server, as an explicit gameplay-policy choice.
- Physics lane: default 60 Hz for the first Rapier-backed implementation, with
  lower-CPU 30 Hz and higher-fidelity 120 Hz profiles available for local
  integrated experiments. Physics can substep inside a lower-rate host tick or
  run once per host frame in a matching host mode.
- AI lane: lower-frequency decision cadence, likely 5-10 Hz for expensive
  thinking, with movement intents fed into the normal authoritative simulation.
- Networking lane: snapshot and event publication rates are selected per entity
  class and connection mode. Network publication rate is not the same thing as
  internal simulation rate.
- Client render lane: frame-rate driven. Rendering consumes presentation state,
  interpolation, and later prediction/extrapolation rather than driving
  authoritative simulation.

## Invariants

- Do not use a single global `tick` value to mean all subsystem time.
- The gameplay lane remains 20 Hz by default for the Minecraft-like profile, but
  it is configurable as part of the cadence profile.
- Default supported cadence profiles should use clean integer relationships:
  lower-rate lanes divide the host rate, and higher-rate lanes are integer
  substeps of each host frame.
- Uneven fractional lane pacing, such as 20 Hz gameplay on a 30 Hz host, should
  be rejected unless we explicitly add a future advanced fractional scheduler.
- Physics substeps may improve authoritative simulation quality without
  increasing network snapshot frequency.
- Client prediction or local render sampling is allowed only behind shared
  command/state contracts. It must not create a local-only architecture fork.
- Web/WASM remains part of the target shape: browser worker timing should map to
  the same lanes rather than becoming a reduced single-threaded design.

## Landed Slices

For the debug physics experiment, keep the server runner at 20 Hz and step
Rapier three times at 1/60 second per server tick in the default profile.
Alternate clean cadence profiles can run physics on host frames where gameplay
does not advance.

The expected benefit is better collision/contact integration and more stable
settling. Visual smoothness for authoritative updates now depends on the host
cadence, entity publication policy, and the current actor interpolation model.

- Added `SimulationCadence` in `mclone-server` as the first host/lane stepping
  primitive. It maps one configurable host frame into fixed-rate gameplay and
  physics lane work without changing live runner behavior yet.
- Initial tested mappings for the first cadence primitive:
  - 20 Hz host: one 20 Hz gameplay tick and three 60 Hz physics steps per host
    frame.
  - 60 Hz host: one 60 Hz physics step per host frame and one 20 Hz gameplay
    tick every third host frame.
  - 30 Hz host: deterministic fractional gameplay cadence with two 60 Hz physics
    steps per host frame. This was useful for proving deterministic scheduling,
    but it is no longer the desired default validation shape.
- The elapsed-time wrapper accumulates partial host frames and caps catch-up
  work so a long frame cannot force unlimited simulation work in one pump.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server cadence -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-server
```

- Routed the native integrated server runner through `SimulationCadence` for the
  full gameplay tick lane. The default native runner cadence remains 20 Hz host,
  20 Hz gameplay, and 60 Hz physics, so each scheduled host frame still runs one
  current `IntegratedServer` simulation tick.
- The runner config now carries a cadence config and rejects invalid cadence
  values before reporting the server thread ready.
- `frame.physics_steps` is intentionally not consumed by the runner yet because
  physics still runs inside the full `IntegratedServer` gameplay tick. This was
  superseded by the later split below.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_config_defaults_to_twenty_hz_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_rejects_invalid_cadence_config -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_crosses_commands_and_updates_over_frames -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server --quiet
cargo check --manifest-path native/Cargo.toml -p mclone-server
```

- Tightened `SimulationCadenceConfig` so default-valid profiles require clean
  integer lane relationships. Lower-rate lanes must divide the host rate, and
  higher-rate lanes must be integer substeps of each host frame.
- Replaced the per-lane fractional accumulator behavior with fixed interval /
  substep lane scheduling. The wall-clock elapsed-time wrapper still accumulates
  partial host frames and caps catch-up work.
- Current tested valid profiles include 20 Hz host / 20 Hz gameplay / 60 Hz
  physics, 60/20/60, 60/60/60, 30/30/60, and 20/10/60.
- Uneven profiles such as 30 Hz host / 20 Hz gameplay / 60 Hz physics are now
  rejected unless a future advanced fractional scheduler is explicitly added.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_config_defaults_to_twenty_hz_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_rejects_invalid_cadence_config -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server --quiet
cargo check --manifest-path native/Cargo.toml -p mclone-server
```

- Split physics stepping out of the monolithic `IntegratedServer` gameplay tick.
  The public compatibility tick path still runs three 60 Hz physics steps for
  one 20 Hz gameplay tick, while the native runner now runs gameplay ticks with
  zero embedded physics and consumes `frame.physics_steps` through a separate
  physics-lane report.
- Added `ServerPhysicsStepReport` and fixed-count physics stepping so a host
  frame can publish debug physics entity updates without incrementing gameplay
  tick, day time, scheduled block/fluid ticks, or entity age.
- Runner diagnostics merge physics diagnostics back into the gameplay tick when
  both lanes run on the same host frame. Physics-only frames refresh runner
  diagnostics without pretending a gameplay tick occurred.
- At this point the native runner still had an explicit wall-clock
  `tick_interval`; this was superseded by the later native CLI cadence slice
  below.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier physics_step_report_advances_debug_cube_without_gameplay_tick -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-server
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
```

- Added a native client developer CLI option for local integrated worlds:
  `--simulation-cadence HOST/GAMEPLAY/PHYSICS`, with `--cadence` as a short
  alias. The parser rejects invalid clean-ratio profiles and rejects the option
  for remote dedicated sessions, where the local client cannot control the
  server cadence.
- Threaded cadence through `SceneOptions` and `LocalSingleViewSceneOptions` into
  `NativeIntegratedServerRunnerConfig`.
- The local native runner now derives its wall-clock `tick_interval` from the
  selected cadence host rate. For example, `--simulation-cadence 60/20/60`
  schedules a 60 Hz host pump, 20 Hz gameplay lane, and 60 Hz physics lane.
- Startup logging includes the selected cadence so manual runs can confirm the
  active profile.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-app-runtime -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime native_runner_config_derives_tick_interval_from_cadence_host_rate -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-native-client cli_parses_simulation_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client cli_rejects_invalid_simulation_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --quiet
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
```

- Added `NativeIntegratedServerRunner::set_simulation_cadence(...)` as the
  runtime control hook for future local server settings UI. The method validates
  cadence through `SimulationCadence`, derives the host `tick_interval` from the
  selected host rate, and queues one runner-control message.
- The native runner loop now owns a mutable timing state, so live cadence
  changes replace the host interval and lane cadence together and reset the next
  scheduled host frame.
- `ServerRunnerDiagnostics` now exposes the active cadence profile and host tick
  interval. This gives future UI a read path for the current integrated-server
  settings.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -p mclone-app-runtime -p mclone-native-client -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_applies_live_simulation_cadence_control -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_rejects_invalid_live_simulation_cadence_control -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_config_can_derive_tick_interval_from_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime native_runner_config_derives_tick_interval_from_cadence_host_rate -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --quiet
cargo check --manifest-path native/Cargo.toml -p mclone-server
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
```

- Added a shared `mclone-ui` server settings screen reachable from Options for
  active local integrated worlds. It exposes cycle controls for Host Rate, World
  Tick Rate, and Physics Rate, and each control emits a full valid clean-ratio
  cadence rather than an invalid partial lane change.
- Threaded local integrated cadence read/apply methods through
  `LocalSingleViewSceneRuntime`, `NativeSingleViewSceneRuntime`, and the native
  window runtime. Remote dedicated sessions intentionally expose no editable
  cadence from this client-side path.
- The native flat client now maps `SetServerSimulationCadence` UI actions to the
  live local runner control and also updates the scene preset so future local
  starts keep the selected cadence.
- Rapier debug physics now receives a physics step duration derived from the
  active physics lane rate. The compatibility tick path remains three 1/60
  steps for one 20 Hz gameplay tick, while live cadence profiles such as
  60/20/120 now run 120 Hz physics at 1/120 second per step instead of doubling
  simulation speed.
- Added a native headless `server-settings-pause` screenshot target and routed
  headless UI render state through the live local runtime cadence so automated
  captures can inspect the actual integrated-server controls.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-ui -p mclone-server -p mclone-app-runtime -p mclone-native-client
cargo fmt --manifest-path native/Cargo.toml -p mclone-ui -p mclone-server -p mclone-app-runtime -p mclone-native-client -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-ui server_cadence -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui server_settings -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-ui --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-server --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-native-client --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-native-client parse_screenshot_ui_accepts_named_screens -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client ui_action_sets_server_simulation_cadence_preset_without_runtime -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime local_single_view_runtime_applies_simulation_cadence_control -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_applies_live_simulation_cadence_control -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server native_runner_rejects_invalid_live_simulation_cadence_control -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier --quiet
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier physics_step_report_advances_debug_cube_without_gameplay_tick -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier debug_physics_cube_applies_minecraft_vertical_drag -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-server
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-server-settings-ui.png --width 1280 --height 720 --startup-wait playable --screenshot-ui server-settings-pause --seed 111 --render-distance 2 --simulation-cadence 20/20/60 --lighting false --fullbright true
```

Screenshot inspection: `/tmp/mclone-server-settings-ui.png` rendered the Server
Settings menu with Host Rate 20 Hz, World Tick Rate 20 Hz, Physics Rate 60 Hz,
and no visible text/control overlap at 1280x720.

Residual validation failure: `pnpm native:web:smoke` still fails with the
top-level canvas aggregate reporting `canvas.ok: false`. The same run reports
browser threading, shared memory, WebGPU readiness, WASM runtime, chunk compile,
textures, sky, actor rendering, and worker runner diagnostics as OK, so this is
tracked as a pre-existing web canvas smoke issue rather than a cadence-control
blocker.

## Follow-Up Work

- Decide whether cadence selections should persist as user preferences or remain
  per-session developer controls.
- Add explicit diagnostics for physics substep count and physics lane timing.
- Extend the full-orientation path with angular velocity or buffered orientation
  samples if active physics presentation still looks too 20 Hz.
- Evaluate active-physics presentation options: higher-rate snapshots for local
  integrated play, velocity/angular-velocity extrapolation, or a shared
  prediction path.
- Decide which entity classes can opt into higher-rate network snapshots.
