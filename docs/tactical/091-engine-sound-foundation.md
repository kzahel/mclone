# 091: Engine Audio Foundation and Landing Sound

Status: active; Slices 1-2 code landed 2026-06-26 and Tactical
[`275`](275-first-party-sound-effects.md) extended the foundation on
2026-08-11. Native output, first-party content, and shared event producers are
implemented and validated through automated gates. Manual listening/device
validation and web audio output remain.

## Purpose

The native engine emits no audio on any client lane. This tactical adds the
smallest real audio path that still fits the platform plan:

- one shared `mclone-audio` API and mixer for desktop flat, desktop OpenXR, flat
  Android, Android XR, and eventually web/WASM;
- native desktop, desktop OpenXR, flat Android, and Android XR are wired first;
  web/WASM is last and must stay behind the same Rust API with thin JS glue;
- sound content is loaded from the existing asset path, not embedded in platform
  adapters;
- the first gameplay trigger is the local player landing after a jump or fall,
  detected in shared movement/camera code so all hosts agree;
- missing sound files are graceful: log once, stay silent, never crash.

This is a foundation, not the final sound system. Later tacticals own material
step/break/place sounds, exact Java `SoundType` parity, volume categories,
ambient/music beds, remote-player/entity audio, and XR spatialization.

## Review Outcome

The current shape is mostly aligned with the platform constraints: audio stays
out of simulation/server code, platform adapters only own device lifecycle, and
the first trigger comes from shared movement state. The doc needed these
corrections before implementation:

- Do not make web/WASM a first-slice commitment. It has the most policy surface:
  browser autoplay, worklet choices, worker/shared-memory constraints, and bundler
  glue. Keep it last.
- Do not add custom WebAudio/AudioWorklet JavaScript in this tactical. The web
  acceptance bar is: use the same Rust `mclone-audio` API and only minimal
  gesture/resume glue. If that is not clean enough when reached, web remains
  silent temporarily.
- Use real Minecraft 1.17 placeholder samples instead of a generated `land.wav`.
  The pinned 1.17 asset index exposes the two useful landing files:
  `minecraft/sounds/damage/fallsmall.ogg` and
  `minecraft/sounds/damage/fallbig.ogg`.
- Because those placeholders are OGG files, the first slice should decode Vorbis
  to PCM at load time instead of introducing a WAV-only path that would be
  replaced immediately.
- Be explicit about real-time safety. The audio callback may mix active voices
  and drain a bounded command queue, but it must not block, allocate on normal
  playback, or take locks.

## Decisions

- **Sample playback only.** No synth engine, no procedural placeholder waveform,
  and no DSP port from Playbox. Playbox is only a lifecycle/reference pattern:
  app thread sends commands, the callback mixes, device failure disables audio
  without panicking.
- **One Rust audio surface.** Client apps call `AudioEngine::play(...)`; they do
  not know whether output is desktop, Android, XR, or browser.
- **CPAL for native/Android/XR first.** Use `cpal` as the device layer for
  desktop flat, desktop OpenXR, flat Android, and Android XR. These lanes should
  share the same mixer and device code except for platform lifecycle entry points.
- **Web last, thin only.** When web is implemented, prefer CPAL's
  `wasm-bindgen` WebAudio path if it stays thin. Do not author a bespoke
  JavaScript audio graph or AudioWorklet here. AudioWorklet can be a later
  performance tactical if we deliberately accept its extra browser/build policy.
- **Local Mojang placeholder overlay.** Fetch a tiny allowlist from the 1.17
  asset index for personal/local validation. Do not commit or redistribute Mojang
  sound files.
- **Stable sound keys, asset paths behind them.** `mclone-audio` exposes typed
  `SoundKey` constants or a small key enum. The asset path mapping stays inside
  the sound bank so later Java parity can swap mappings without platform edits.

## Starting Point Before Slice 1

- No audio crate, dependency, or device code exists in `native/`.
- Landing is already computed deterministically in shared code. In
  `LocalPlayerController::move_colliding`
  (`native/crates/mclone-client/src/player.rs`):
  `self.on_ground = self.vertical_collision && requested.y < 0.0;`.
- Every client lane drives walking through `EngineCameraController` in
  `mclone-render-session` (`tick_walking` ->
  `tick_walking_movement_with_impulse`). Current call sites are desktop flat,
  flat Android, web, and the shared XR scene driver.
- The asset system already supports bytes at resource paths. `AssetSource`
  (`native/crates/mclone-assets/src/source.rs`) exposes `read(path)` and
  `list(prefix, suffix)`, with chained sources.

The engine already knows the platform-neutral tick where a landing happens. The
new work is audio output, asset fetch/staging, and a small event bridge.

## Implementation Status

Landed on 2026-06-26:

- Added `native/crates/mclone-audio` with typed sound keys, OGG/Vorbis decode via
  `lewton`, fixed-capacity sample voices, a bounded non-blocking command queue,
  linear resampling, mono/stereo channel mapping, and CPAL desktop output.
- Added `LandingEvent { impact_speed, position }`,
  `LANDING_MIN_IMPACT_SPEED`, and `take_landing_events()` to
  `EngineCameraController`.
- Wired desktop flat to construct `AudioEngine`, drain landing events each
  movement frame, and play `LANDING_SMALL` or `LANDING_BIG` with impact-scaled
  gain.
- Added optional sound overlay loading in `mclone-app-runtime`; it is appended
  only after core Minecraft assets are found, so it does not mask missing texture
  or blockstate assets.
- Added `scripts/fetch-sound-assets.ps1` and fetched the two local placeholder
  OGGs into the gitignored
  `reference/minecraft-1.17.1/sound-overlay/` directory.
- Added shared XR-scene audio ownership via an optional `AudioEngine`, with
  desktop OpenXR and Android XR hosts creating audio from their existing asset
  sources and the shared scene draining landing events after controller
  locomotion.
- Added flat Android audio initialization in the renderer lifecycle and landing
  playback after touch movement.
- Updated Android and Android XR build scripts to pass Gradle `minSdk` to
  `cargo ndk` (`--platform 28` today), which is required for CPAL's Android
  AAudio link path.
- Routed `native:android-xr:*` package scripts through `run-native-bash.mjs` so
  they work from the repo's normal PowerShell-driven workflow.

Validated:

- `cargo test --manifest-path native/Cargo.toml`
- `pnpm native:movement:smoke`
- `pnpm native:web:build`
- `cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-native-client --features xr`
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client --target x86_64-linux-android`
- `pnpm native:android:apk:avd`
- `pnpm native:android-xr:apk`

Not yet validated by automation: audible playback, AVD/Quest runtime audio, and
desktop/headset manual listening. Web remains deferred by design.

Extended by Tactical 275 on 2026-08-11:

- Mclone Original now loads 119 redistributable Kenney CC0 OGGs into 33
  semantic variant families; Generated Fallback Only remains silent.
- Playback commands carry bounded gain, pitch, stereo pan, and deterministic
  event seeds while preserving the bounded callback and voice model.
- Shared scene policy emits grounded distance-based footsteps, material-aware
  landings, replica-confirmed break/place sounds, and semantic UI feedback in
  mono and XR.
- The same first-party bank prepares successfully on web/WASM. Browser output
  remains an explicit unavailable capability until thin autoplay/resume and
  output glue are implemented.
- Flat Android and Android XR APK builds include the authored pack and pass.
  Actual loudness/cadence review on speakers, headphones, AVD/device, and
  headset remains a human acceptance step.

## Placeholder Sound Assets

Add a small script under `scripts/` that reads
`reference/minecraft-1.17.1/1.17.1.json`, follows `assetIndex.url`, downloads
only an allowlist, verifies SHA-1 against the asset index, and writes a
gitignored local overlay preserving Mojang asset paths.

Initial allowlist:

```text
minecraft/sounds/damage/fallsmall.ogg
minecraft/sounds/damage/fallbig.ogg
```

Optional seed samples for the next walking slice can be fetched by the same
script, but should not expand this implementation beyond landing:

```text
minecraft/sounds/step/grass1.ogg
minecraft/sounds/step/stone1.ogg
```

Suggested local overlay root:

```text
reference/minecraft-1.17.1/sound-overlay/assets/minecraft/sounds/...
```

Implementation notes:

- Keep the overlay out of git. The script and allowlist can be committed; the
  downloaded OGG files cannot.
- Add the overlay as an optional `AssetSourceChain` entry in
  `mclone-app-runtime` so every native/XR/Android adapter can see the same sound
  files without platform-specific paths.
- Preserve original `minecraft/...` asset paths for provenance, but expose
  engine keys such as `LANDING_SMALL` and `LANDING_BIG`.
- Exact Java sound event, pitch, volume, and fall-distance rules are not part of
  this tactical. When that parity slice starts, read the 1.17.1 Java source first.

## Architecture

Three layers, matching the existing engine/app/platform split:

```text
shared engine and render session - deterministic, no audio dependency
  EngineCameraController.tick_walking
    detects airborne -> on_ground transition
    records LandingEvent { impact_speed, position }

mclone-audio - one crate
  SoundBank   : SoundKey -> decoded PCM sample variants
  Mixer       : active voices -> render(&mut [f32], channels)
  AudioEngine : owns the CPAL stream and sends bounded commands to the callback

client apps and scene adapters - thin glue
  each frame: drain landing events -> audio.play(LANDING_SMALL/BIG, gain)
```

Dependency direction:

- client apps depend on `mclone-render-session` to drain events and
  `mclone-audio` to play them;
- `mclone-audio` depends on `mclone-assets`, `cpal`, a small OGG/Vorbis decoder
  such as `lewton`, `anyhow`, and `log`;
- core simulation, movement, server, worldgen, meshing, renderer internals, and
  shared protocol crates do not depend on audio;
- `mclone-render-session` does not depend on `mclone-audio`.

### Landing Event

Add a transition tracker to `EngineCameraController`:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingEvent {
    pub impact_speed: f64, // blocks/sec, downward speed at the landing tick
    pub position: Vec3d,   // centered for now, useful for later spatial audio
}
```

Detection outline:

```rust
let was_on_ground = self.player.on_ground();
let pre_move_fall_speed = (-self.player.delta_movement().y).max(0.0);
let position = self.player.position();
let moved = self
    .player
    .tick_walking_movement_with_impulse(/* ... */)
    .is_some();

if moved && !was_on_ground && self.player.on_ground() {
    let impact_speed = pre_move_fall_speed * LOCAL_PLAYER_TICKS_PER_SECOND;
    if impact_speed >= LANDING_MIN_IMPACT_SPEED {
        self.landing_events.push(LandingEvent { impact_speed, position });
    }
}
```

Notes:

- A jump and a fall both end in the same airborne -> `on_ground` transition.
- `LANDING_MIN_IMPACT_SPEED` filters jitter.
- `probe_ground` must not double-fire; keep detection in the normal walking tick
  and cover it with a test.
- No-clip/spectator movement should not produce landings.
- The app drains events with `take_landing_events()` once per frame.

### Audio Engine

```rust
pub struct AudioSettings {
    pub enabled: bool,
    pub master_gain: f32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SoundKey(&'static str);

pub const LANDING_SMALL: SoundKey = SoundKey("mclone:landing_small");
pub const LANDING_BIG: SoundKey = SoundKey("mclone:landing_big");

pub struct AudioEngine {
    // bounded command sender plus the owned CPAL stream
}

impl AudioEngine {
    pub fn new(assets: &impl AssetSource, settings: AudioSettings) -> anyhow::Result<Self>;
    pub fn play(&self, sound: SoundKey, gain: f32);
    pub fn set_settings(&self, settings: AudioSettings);
}
```

Runtime contract:

- `AudioEngine::new` loads known keys from `AssetSource`, decodes OGG to
  interleaved `f32` PCM, and builds a `SoundBank`.
- Missing files are logged once. Playing a missing key is a silent no-op.
- The callback owns the mixer state, drains a bounded non-blocking command queue,
  and keeps a fixed/preallocated active-voice capacity.
- Normal `play` must not allocate in the callback, block, or take a mutex.
- If the command queue or voice capacity is full, drop the new play command and
  increment/log a diagnostic counter outside the callback.
- Do simple linear resampling if the device rate differs from the sample rate.
  Downmix/duplicate mono/stereo to the output channel count.
- Choose `LANDING_SMALL` for normal hops and `LANDING_BIG` above a simple impact
  threshold. Exact vanilla fall-distance rules are deferred.

## Platform Wiring

| Lane | Plan |
|---|---|
| Desktop flat | First validation lane. Construct `AudioEngine` in the app startup path and drain landing events each frame. |
| Desktop OpenXR | Reuse the same engine in the XR host. For this slice, local landing audio is centered/non-spatial. |
| Flat Android | Construct on activity resume after Android context is available; drop/suspend on pause. |
| Android XR / Quest | Same Android lifecycle plus the shared XR scene driver event drain. No XR-specific sound graph yet. |
| Web/WASM | Last. Try CPAL `wasm-bindgen` WebAudio behind the same API, with only user-gesture resume glue. If it needs broad JS/audio policy work, leave web silent and document the follow-up. |

## Implementation Slices

- [x] **Slice 1: shared engine, placeholder fetch, landing hook, desktop flat.**
  - Add `mclone-audio`: OGG decode, `SoundBank`, fixed-capacity voices,
    bounded command queue, mixer tests, CPAL desktop stream.
  - Add the sound-asset fetch script and a gitignored local overlay path.
  - Add the optional sound overlay to `mclone-app-runtime` asset loading.
  - Add `LandingEvent`, `LANDING_MIN_IMPACT_SPEED`, transition tracking, and
    `take_landing_events()` to `EngineCameraController`.
  - Wire desktop flat to drain events and play `LANDING_SMALL` or `LANDING_BIG`
    with impact-scaled gain.
  - Validate with `cargo test --manifest-path native/Cargo.toml`,
    `pnpm native:movement:smoke`, and a manual desktop run. Automated gates have
    passed; manual listening remains.

- [x] **Slice 2: desktop OpenXR, flat Android, Android XR.**
  - Wire the same event drain and `AudioEngine` into the shared XR scene/host
    path and the flat Android app.
  - Keep platform code limited to lifecycle construction/suspend/drop.
  - Validate desktop XR smoke plus Android/Quest build and headset or AVD smoke.
    Code compile/build gates passed; headset/AVD runtime audio validation remains.

- [ ] **Slice 3: web/WASM, only if the glue stays thin.**
  - Compile the same crate for `wasm32-unknown-unknown`.
  - Resume browser audio from an existing user gesture path.
  - Keep browser audio optional so headless web smokes do not require hardware or
    autoplay permission.
  - Validate `pnpm native:web:build`, `pnpm native:web:smoke`, and app smoke.

## Validation

| Lane | Gate |
|---|---|
| Shared logic | `cargo test --manifest-path native/Cargo.toml` for mixer, bounded-command behavior, missing-key no-op, and landing detection |
| Desktop flat | `pnpm native:movement:smoke` passed; manual run still needed: hop = small landing, ledge fall = big landing, standing = silent |
| Desktop OpenXR | XR-feature tests passed; manual OpenXR listen/smoke remains |
| Flat Android | `pnpm native:android:apk:avd` passed; AVD/device listen smoke remains |
| Android XR | `pnpm native:android-xr:apk` passed; Quest listen smoke remains |
| Web/WASM | Deferred until Slice 3; build/smoke must pass with audio optional |

No screenshot applies. Regression coverage is deterministic mixer/controller
tests plus manual listening on the lanes with real devices.

## Out of Scope

- Exact Java `SoundType` parity, material-aware footsteps, and break/place
  sounds.
- Full walking cadence and per-surface sample selection. The script may fetch
  one or two step samples for local experiments, but this tactical wires landing.
- Remote-player/entity positional audio.
- Volume categories and options UI.
- Ambient sound, music, streaming decode, and long-form asset management.
- True spatial/XR audio, listener pose, distance attenuation, or HRTF.
- Custom WebAudio, AudioWorklet, or broad web worker/audio architecture.

## Risks

- **Mojang placeholder assets:** must remain local/gitignored and should not be
  redistributed. The script must verify hashes and fetch only the allowlist.
- **Web policy creep:** if CPAL web still requires too much glue, keep web silent
  for this tactical instead of adding bespoke JavaScript audio.
- **Android lifecycle:** construct/resume only after Android context is ready and
  tolerate device loss.
- **Real-time callback safety:** use bounded queues and fixed voice capacity; no
  locks, blocking calls, or allocation during normal playback.
- **Landing false positives:** `probe_ground`, no-clip, and analog movement paths
  need direct tests so landing fires once per real airborne -> grounded transition.
