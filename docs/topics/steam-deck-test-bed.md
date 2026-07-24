# Steam Deck Test Bed

Topic: steam-deck-test-bed

Status: active test lane. As of 2026-07-23, the first retail Steam Deck is
paired with the Linux deployment host. Automated staging, incremental upload,
SteamRT4 launch, screenshot smoke, and bounded performance collection pass on
the device. Manual controller, persistence, suspend/resume, and dock/undock
acceptance remain open. The pinned production-style Steam Runtime SDK builder
is implemented and its clean-source artifact passes device smoke and perf.

## Scope

This topic owns the repeatable physical Steam Deck build, deploy, playtest, and
performance-validation lane. It does not make Steam Deck a separate gameplay
implementation target: the Deck consumes the shared desktop-flat Linux host,
ordinary controller contracts, renderer, assets, scene, and persistence paths.

Long-lived machine addresses, local account state, package inventories, and
other private host facts do not belong here. Keep them in the relevant private
machine ledger.

## Deployment Direction

Use Valve's SteamOS Devkit Client as the normal deployment path:

1. Enable Developer Mode on an ordinary retail Steam Deck.
2. On the Deck, use **Settings > Developer > Pair new host**.
3. Install SteamOS Devkit Client (Steam App ID `943760`) on a Linux or Windows
   build host.
4. Register the Deck in the client, using its IP explicitly if multicast-DNS
   discovery is unavailable.
5. Upload a staged native Linux build and launch the resulting
   `Devkit Game: mclone` entry from Gaming Mode.

The Devkit Client already performs incremental `rsync` over SSH and provisions
the host key during pairing. Do not enable the Deck's general-purpose SSH
service, set a password, or unlock SteamOS's read-only root solely for this
workflow. Direct SSH/rsync remains a fallback for command-line automation, not
the first provisioning step.

The first pairing was verified on 2026-07-23 by authenticating with the
Devkit-generated key, synchronizing Valve's utility scripts, and completing a
machine-readable `steamos-get-status` query while the Deck was in its Gamescope
session. Keep the device address, exact host paths, and local account state in
the private laptop ledger.

Valve's current reference instructions are:

- [How to load and run games on Steam Deck and Steam Machine](https://partner.steamgames.com/doc/steamhardware/loadgames)
- [Developing for SteamOS and Linux](https://partner.steamgames.com/doc/store/application/platforms/linux)
- [Steam Deck and Steam Machine compatibility review](https://partner.steamgames.com/doc/steamhardware/compat)

## Mclone Build And Payload Contract

The production-shaped test payload should be a native Linux `x86_64` release
build produced with the selected matching Steam Linux Runtime SDK. A plain
Ubuntu release build is acceptable for initial bring-up only; it is not the
long-term distribution-compatibility proof.

Steam Deck is an `x86_64` Linux target, so it does not need a special Rust
target or a console-style cross compiler. The initial target remains
`x86_64-unknown-linux-gnu`. The important production distinction is the
userspace ABI: build inside Valve's Steam Linux Runtime 4 SDK and select Steam
Linux Runtime 4 when launching the title.

Keep the first production artifact on Rust's generic `x86-64` CPU baseline.
Never build it with `-C target-cpu=native` on the development laptop: that
would optimize for the laptop CPU and can emit instructions that do not exist
on the Deck. If a Deck-only artifact later warrants CPU specialization,
benchmark a separate Zen 2 / AVX2 profile on both LCD and OLED Decks and keep
the generic artifact as the compatibility control. Wgpu's Vulkan shaders do
not need a separate Deck compilation path.

A minimal staged directory is:

```text
dist/steamdeck/
├── mclone-native-client
├── run.sh
└── assets/
    └── extracted.zip
```

The launcher must resolve resources relative to itself and set explicit
runtime roots:

```sh
#!/bin/sh
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export MCLONE_ASSET_MODE=pack-only
export MCLONE_ASSET_PACK="$HERE/assets/extracted.zip"
export MCLONE_WORLD_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/mclone-deck/worlds"

exec "$HERE/mclone-native-client" "$@"
```

Explicit `MCLONE_ASSET_PACK` is required for the current developer payload
because native fallback asset paths derive from the build-time repository
root, which does not exist on the Deck. The extracted Minecraft reference pack
is internal parity-test content only and must not become a public release
payload. Replace it with the complete first-party pack before distribution.

Keep the Deck world root outside the uploaded title directory. A clean Devkit
Client upload may delete files absent from the local staging directory and
must never delete playtest worlds or preferences.

## Implemented Command Line

The repository wrapper is [`scripts/steam-deck.sh`](../../scripts/steam-deck.sh).
Its package commands are:

```bash
pnpm steamdeck:status
pnpm steamdeck:power-status
pnpm steamdeck:screen-off
pnpm steamdeck:screen-on
pnpm steamdeck:stage
pnpm steamdeck:upload
pnpm steamdeck:deploy
pnpm steamdeck:launch
pnpm steamdeck:smoke
pnpm steamdeck:perf
pnpm steamdeck:pull-results -- RUN_ID

pnpm steamdeck:build:steamrt4
pnpm steamdeck:stage:steamrt4
pnpm steamdeck:deploy:steamrt4
pnpm steamdeck:deploy:production
pnpm steamdeck:smoke:steamrt4
pnpm steamdeck:perf:steamrt4

pnpm steamdeck:auto-deploy:on
pnpm steamdeck:auto-deploy:off
pnpm steamdeck:auto-deploy:status
pnpm steamdeck:auto-deploy:log
```

The wrapper defaults to the paired device's mDNS name and the Devkit-managed
SSH key. Override discovery without recording a private address in the
repository:

```bash
MCLONE_STEAM_DECK=HOST pnpm steamdeck:status
```

### Unattended AC power and screen control

The paired Deck is configured with `IdleSuspendACSeconds=0`, so Gaming Mode
does not idle-suspend it while AC remains connected. Its battery idle-suspend
timer remains 900 seconds. This is the desired unattended-test-bed policy:
losing AC still has a bounded battery safeguard, while an AC-powered Deck
retains SSH and Devkit reachability without a long-running inhibitor process.

Use the wrapper to inspect that state and sleep only the built-in panel:

```bash
MCLONE_STEAM_DECK=HOST pnpm steamdeck:power-status
MCLONE_STEAM_DECK=HOST pnpm steamdeck:screen-off
MCLONE_STEAM_DECK=HOST pnpm steamdeck:screen-on
```

`screen-off` asks the active Gaming Mode Gamescope session to set
`drm_sleep_internal_screen=true`. On the 2026-07-23 device this changed
`card0-eDP-1/enabled` from `enabled` to `disabled` while SSH remained live.
`screen-on` clears the convar and verifies that the connector becomes enabled
again. This is real connector sleep, not minimum brightness, and it does not
suspend the Deck. The commands fail explicitly outside an active Gamescope
session and affect only the internal panel; they do not blank a docked external
display.

The Gamescope convar is a forced state: Gamescope does not clear it merely
because ordinary input arrives. Before disabling the connector, the wrapper
therefore deploys and arms a one-shot, non-grabbing user service that watches
the built-in Deck controller's read-only HID reports. The next mapped Deck
button press clears the convar and exits the watcher. Touch and motion alone
are deliberately ignored, while the physical power button retains its system
suspend semantics. If the watcher cannot arm, `screen-off` refuses to blank
the panel. The byte masks follow the upstream Linux
[`hid-steam`](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-steam.c)
Deck report layout. `screen-on` over SSH remains the fallback recovery path.
The end-to-end 2026-07-23 hardware check disabled `card0-eDP-1`, retained SSH,
then observed an A-button report on `/dev/hidraw2` and restored the connector
to `enabled`.

Turn the screen on before interactive play, screenshot acceptance through the
live compositor, or presentation/performance measurement. Leaving the
connector disabled is appropriate for idle availability and CPU-only or
offscreen work, but it is not representative live-presentation evidence. An
ordinary Gamescope restart resets this session-scoped forced-sleep state.

`stage` builds the release client, verifies the checked-in asset-pack lock,
and assembles `dist/steamdeck` with a SHA-256 build receipt. `upload` performs
Valve's upload preparation, an incremental clean `rsync`, and registration as
`Devkit Game: mclone`. `deploy` also launches interactive play. `launch`
restarts an already uploaded interactive build.

`steamdeck:deploy:production` is the complete manual production-style command.
It checks the asset pack against the tracked lock before the expensive build.
If the check fails, it rebuilds the ignored archive once and checks again; it
never rewrites the tracked lock automatically, so intentional source/tool
changes still require inspection and an explicit
`pnpm assets:pack:write-lock`. Once assets pass, it builds the ordinary
`x86_64` client inside the pinned SteamRT4 SDK, stages the binary/pack/receipt,
uploads incrementally, registers `Devkit Game: mclone`, and launches it.

### Optional deploy after pushing `main`

The existing non-blocking local pre-push hook also schedules a separate Steam
Deck lane when enabled:

```bash
pnpm steamdeck:auto-deploy:on
```

The toggle is local to this checkout and defaults to off in a new clone. Disable
it before pushing from a network that cannot reach the paired device:

```bash
pnpm steamdeck:auto-deploy:off
```

Scheduling never blocks or determines the success of `git push`. The
background worker first waits until the remote really reports the pushed
`main` SHA, coalesces quick successive pushes, and probes Devkit-managed SSH
for only a few seconds. An unavailable Deck is an explicit logged skip. A
reachable Deck is built and deployed from a clean reusable sibling worktree at
the exact pushed commit, independently of the web deploy worker. Generated
asset archives and SteamRT4 target/cache state remain ignored and local.

Inspect structured state or the latest log lines with:

```bash
pnpm steamdeck:auto-deploy:status
pnpm steamdeck:auto-deploy:log
```

State is under `.git/mclone-steam-deck-after-main-push/` with separate pending,
completed, skipped, failed, summary, and append-only log records. The
machine-local host is read from `mclone.steamDeckHost` in this checkout's local
Git configuration, with `MCLONE_STEAM_DECK` retaining highest precedence.

The payload launcher passes `--platform-profile steamos` for interactive and
live-presentation runs. This is a launch-policy hint carried by the ordinary
SteamRT4 Linux binary, not a separate Deck executable:

- it requests borderless fullscreen so Gamescope supplies the active output
  extent instead of applying X11 logical-window DPI expansion;
- `--width` and `--height` are physical fallback dimensions for window
  creation, rather than logical dimensions;
- the swapchain and screen-space UI stay at the compositor-provided output
  resolution;
- the 3D world stays native through a 1080-pixel-high / 1920x1080 pixel
  budget, then scales down proportionally while preserving the active
  display's aspect ratio; and
- resize events recompute that automatic world scale, so dock/undock can move
  between handheld-native and capped external-display rendering.

On the built-in 1280x800 panel the automatic world scale is `1.0`. A 4K
external output uses a native 3840x2160 swapchain and UI with a 1920x1080 world
target. Ultrawide and 16:10 outputs derive both world dimensions from the
actual output rather than forcing 16:9. The launch profile must not be treated
as proof that the built-in panel is active.

The shared Graphics page now makes that presentation contract visible:

- `Output Resolution` reports the current compositor/swapchain extent;
- `World Resolution` reports the actual internal 3D target;
- `World Scale` cycles through `Auto`, 50%, 67%, 75%, and 100%;
- `Auto` retains the launch profile's dock-aware policy, while a fixed
  percentage is an explicit runtime override and remains fixed across resize
  and dock/undock events; and
- the screen-space UI remains native-output resolution in every mode.

The fixed override is intentionally launch-scoped in this slice. Relaunching
returns to `Auto`; persisting it requires the shared typed graphics-preference
codec rather than a desktop-app-only file. Graphics/display preferences must
remain machine-local and must not be cloud-synchronized across dissimilar
displays. The complete settings inventory, persistence investigation, and
implementation order live in
[`graphics-video-settings.md`](graphics-video-settings.md).

Output resolution is factual/read-only for now. A real selector must enumerate
platform video modes, distinguish Gamescope's game-resolution envelope from a
desktop monitor mode, preserve aspect-ratio choices, and provide a timed
apply/revert path. Do not turn the read-only row into a window-size control and
call that display-mode switching.

The Graphics page also retains the already-live render distance, section
occlusion, Far LOD/detail/range, frame pacing, and FPS cap controls. Settings
categories and Controls Help now use a shared clipped scroll region: wheel or
trackpad input scrolls it, controller focus brings hidden rows into view,
headings and Back/Done remain fixed, and compact widths switch from two columns
to one. Direct touch-drag and draggable-scrollbar interaction remain future
input work; Deck controller focus and the mouse-emulating trackpads are covered.
Future detailed settings should be added only with real runtime ownership and
effects (for example particles, clouds, entity distance, mipmaps, UI scale,
ambient occlusion, and fullscreen/video modes), not as cosmetic menu rows.

`smoke` temporarily registers a bounded command that:

1. enables the internal panel before live presentation work;
2. captures a deterministic 1280x800 offscreen frame from a fixed camera;
3. runs 300 frames through the live Gamescope swapchain and present path;
4. writes logs, JSON reports, a status file, and the PNG under the Deck user's
   state directory;
5. restores the normal interactive shortcut;
6. pulls the result to `/tmp/mclone-steam-deck-results/RUN_ID`; and
7. disables the internal panel again while leaving the Deck reachable.

`perf` uses the same lifecycle for a 600-frame offscreen timedemo followed by
a 3600-frame, render-distance-10 Gamescope presentation run. It is a
repeatable developer benchmark, not an automatic performance acceptance
threshold. Record the Deck refresh rate, frame cap, TDP, thermals, clock
policy, and whether the display is docked before comparing runs. Both bounded
commands also attempt to restore the interactive shortcut and disable the
internal panel if the build, stage, upload, run, result wait, or result pull
fails. Interactive `deploy` and `launch` deliberately leave the panel enabled
for playtesting.
The 2026-07-23 bounded-smoke check completed all 300 presentation frames,
pulled a valid 1280x800 PNG and status-zero result, then left
`card0-eDP-1=disabled`, SSH reachable, and the Deck-button wake service active.

The `:steamrt4` commands build the client inside the pinned SDK rather than on
the Ubuntu host. The build-only command leaves the artifact and provenance
receipt under `native/target/steamrt4`; the stage/deploy/smoke/perf commands
feed that exact artifact through the same payload and Devkit lifecycle.

The asset-lock check intentionally fails closed. During the first bring-up the
working tree already contained unrelated extracted/packed asset drift. Fast
iteration reused the existing release binary and pack with:

```bash
MCLONE_STEAM_DECK=HOST \
MCLONE_STEAM_DECK_SKIP_BUILD=1 \
MCLONE_STEAM_DECK_SKIP_ASSET_CHECK=1 \
pnpm steamdeck:smoke
```

Do not use the asset-check override for a production payload. The staging
receipt records both overrides so a reused binary or unchecked pack cannot be
mistaken for a clean build. The final smoke rebuilt with `cargo build
--release --locked`; it still required only the asset-check override.

## Production SteamRT4 Builder

Valve currently recommends Steam Linux Runtime 4 for new native Linux games
and recommends compiling inside the matching SDK container. The implemented
builder is [`containers/steamrt4/Dockerfile`](../../containers/steamrt4/Dockerfile)
plus
[`scripts/steam-deck-build-steamrt4.sh`](../../scripts/steam-deck-build-steamrt4.sh).
Its contract is:

1. Base the dedicated image on
   `registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk`, pinned by digest. As
   observed on 2026-07-23, Valve's stable `latest` alias identifies SteamRT4
   build `4.0.20260608.242786` and manifest digest
   `sha256:584939ebd7d2f1eec719e771fdde4ae3bd469ee741c783abb7fe812ddaaf3ee4`.
   Treat that as the current pin, not a forever version.
2. Verify the SDK's compiler, `pkg-config`, ALSA development headers for
   `cpal`, and libudev development headers for `gilrs`. The pinned SDK already
   contains this dependency set, so the image does not mutate Valve's package
   state through an unpinned `apt-get`.
3. Install Rust 1.97.0 through checksum-pinned rustup-init 1.28.2. The
   workspace requires at least 1.92.
4. Run the container as the invoking host UID/GID with all Linux capabilities
   dropped and `no-new-privileges`. Mount the repository read-only and expose
   only the isolated SteamRT4 target and Cargo cache as writeable paths. Use a
   dedicated empty Docker client config so unrelated/stale desktop credential
   helpers do not affect anonymous Valve SDK pulls.
5. Build `mclone-native-client` with `cargo build --release --locked` for
   the SDK's `x86_64-unknown-linux-gnu` host. Reject
   `RUSTFLAGS=-Ctarget-cpu=native`; explicit portable or future Deck-only
   flags are recorded.
6. Record the base-image digest, Rust version, Cargo.lock hash, source commit
   and dirty state, target/Rust flags, binary hash, asset-pack hash, and final
   dynamic dependency / GLIBC symbol audit. Embed the builder receipt in the
   staged `build.json`.
7. Test the staged artifact under SteamRT4 on the Deck. A successful build in
   the SDK is necessary ABI evidence, but it does not replace the device
   smoke, interactive controls, persistence, or performance lanes.

The Linux deployment account is deliberately authorized for the system Docker
daemon. This is root-equivalent access; the exact group and machine setup are
recorded in the private laptop ledger. A new login/session is required after
the group change. The Docker daemon itself was not reconfigured.

The builder's advantage is compatibility and provenance, not automatic frame
rate:

- it prevents Ubuntu's newer headers/libraries from leaking into a release;
- a pinned SDK, Rust toolchain, lockfile, target and flags make local and CI
  release inputs repeatable;
- a clean container build catches undeclared native dependencies;
- the embedded receipt makes a deployed binary traceable and auditable; and
- the same builder can become the Linux CI/release lane without changing the
  Deck deployment workflow.

It does not emulate the Deck GPU, Gamescope, controller, power envelope, or
suspend behavior, and it does not intrinsically optimize code for Zen 2. Those
remain physical-hardware tests. The first pull also has a meaningful local
storage cost because Valve's SDK is a full development sysroot.

## Validation Contract

Run interactive acceptance from Gaming Mode so Gamescope, Steam's controller
path, suspend/resume behavior, and the real handheld presentation envelope are
in scope. Desktop Mode and no-window rendering remain useful diagnostics but
are not substitutes.

Initial acceptance should cover:

- native launch and clean exit at 1280x800;
- title/menu/world navigation using only built-in controls;
- correct 16:10 layout, readable HUD/menu text, and controller prompts;
- local world create, save, quit, relaunch, and persistence;
- suspend/resume, including Wi-Fi loss and remote-session recovery;
- dock/undock and audio-device changes;
- cold startup, warm startup, steady stationary play, movement/streaming, and
  representative world interaction;
- the live swapchain/present path through `--window-frame-report`; and
- repeated release measurements under recorded refresh-rate, frame-cap, TDP,
  render-distance, seed/world, and warmup conditions.

Use an ordinary release build for representative performance. Use a separate
`perf-diagnostics` build only when its instrumentation is required, and do not
compare its absolute numbers directly with the production-shaped release.

Local validation for the graphics-menu/scroll slice on 2026-07-23:

- `cargo test --manifest-path native/Cargo.toml -p mclone-ui
  -p mclone-app-runtime -p mclone-scene -p mclone-native-client` passed;
- an inspected 1280x800 offscreen `options-graphics` capture showed all ten
  rows, native/output and world sizes, the live scale control, and fixed Done
  footer without overlap; and
- an inspected 320x140 compact capture showed the responsive one-column list,
  clipped content, fixed footer, and visible scrollbar. A focused unit test
  also proves that controller navigation scrolls the FPS-cap row into view.

This is shared/local rendered evidence. The menu-driven scale change and
trackpad/controller scrolling still need one Gaming Mode pass on the physical
Deck before being called device-accepted.

## 2026-07-23 Device Evidence

The first automated run used the ordinary host-built release binary under the
Deck's installed `SteamLinuxRuntime_4`. It is bring-up evidence, not the
production ABI or performance baseline.

- Smoke run `20260723T145625Z-c72f4e704bac-smoke-2634116` exited zero after a
  fresh locked release build. The staging receipt records
  `buildSkipped=false` and `assetCheckSkipped=true`. The
  pulled 1280x800 PNG was inspected and showed lit grass/dirt terrain, spruce
  trunks, and cutout leaf geometry. The live path presented all 300 requested
  frames with no skipped or reconfigured frames.
- That short presentation run reported Gamescope FIFO at 89.887 Hz, a
  2667x1875 surface in the current session, 11.095 ms median and 12.339 ms p95
  frame-wall time. Because the live surface was not 1280x800, this is launch
  and presentation evidence rather than a native-panel performance result.
- Perf run `20260723T145002Z-5c588bda8b84-perf-2630189` exited zero. Its
  600-frame 1280x800 render-distance-5 timedemo averaged 5.693 ms per render
  frame after a 6.824 s scene build. The render-distance-10 Gamescope lane
  presented all 3600 frames and reached 529 visible chunks with no pending
  generation/publication work at exit.
- The sustained live run used the same 2667x1875, 89.887 Hz session and
  reported 11.126 ms median, 15.096 ms p95, and 16.229 ms p99 frame-wall time.
  It had 1803 frames over the 11.125 ms refresh budget, seven over twice the
  budget, and one over four times the budget. Do not use these numbers as a
  release threshold until the SteamRT4 SDK build, native 1280x800 presentation,
  fixed power/thermal policy, and clean asset/source state are in place.

The result bundles stay under `/tmp` on the host and are not repository
artifacts.

## 2026-07-23 SteamRT4 Production-Style Evidence

Commit `295d788a8e6ec4290ad9a70a7b9ad56cedf88150` was rebuilt from a clean
source tree in the pinned SDK. Its receipt records:

```text
SteamRT4 build: 4.0.20260608.242786
SDK manifest:   sha256:584939ebd7d2f1eec719e771fdde4ae3bd469ee741c783abb7fe812ddaaf3ee4
Rust:           1.97.0
Target:         x86_64-unknown-linux-gnu
RUSTFLAGS:      empty (generic x86-64)
Source dirty:   false
Binary SHA-256: c6eb94ee6ecb2d21c2261cff0670a354d67417caa1f15275301b590267366d34
Maximum GLIBC:  GLIBC_2.39
```

The ELF needs only `libudev`, ALSA, `libgcc_s`, `libm`, `libc`, and the
standard x86-64 loader. All resolved inside the pinned SDK.

- Smoke `20260723T152012Z-295d788a8e6e-smoke-2663708` exited zero under the
  Deck's `SteamLinuxRuntime_4`. The fresh 1280x800 PNG was inspected and
  matched the accepted terrain/foliage view. All 300 Gamescope frames
  presented with no skipped or reconfigured frames.
- Perf `20260723T152103Z-295d788a8e6e-perf-2665383` exited zero. Its 600-frame
  timedemo averaged 5.789 ms after a 6.759 s scene build. The live
  render-distance-10 lane presented all 3600 frames, reached 529 visible
  chunks, and exited with no pending generation/publication work.
- The live lane still used the current 2667x1875, 89.887 Hz Gamescope surface.
  It reported 11.141 ms median, 15.183 ms p95, and 16.650 ms p99 frame-wall
  time. This is effectively in the earlier host-built bring-up envelope, which
  is expected: the container changes ABI discipline, not the runtime workload.
- The asset-lock override remains recorded as true. Resolve the unrelated
  extracted/packed asset drift before calling this a fully clean production
  payload or establishing the release performance threshold.

## 2026-07-23 SteamOS Presentation Profile

The shared Linux binary now accepts `--platform-profile steamos`, and the Deck
payload supplies it for play plus both bounded live-presentation lanes.
`--width`/`--height` now reach window mode as physical fallback dimensions
instead of being discarded while the app creates a hard-coded logical
1280x900 window.

Host validation passed all 177 native-client tests, including the SteamOS
profile parser, handheld-native scale, 4K-to-1080p world cap, and
aspect-preserving 16:10/ultrawide cases. Commit `1129692a` was then rebuilt
from a clean source tree with the pinned SDK and Rust 1.97.0; its binary hash
is `10abc615367e1a443004f6d1b13915d022031b872f56b6a988ee0c2cbcc4fd0f`.

Device upload and screenshot/presentation validation remain pending because
the paired Deck became unreachable after the build. The next device run must
confirm that Gamescope reports a fullscreen 1280x800 output/world target,
scale `1.0`, native UI, and no recurrence of the old 2667x1875 X11 logical-DPI
surface before this becomes the release baseline.

## Bring-up Ledger

- [x] Retail Steam Deck Developer Mode enabled.
- [x] Linux host Steam package and per-user Steam client bootstrapped.
- [x] Sign the Linux host into Steam and install App ID `943760`.
- [x] Pair/register the Deck through **Pair new host**.
- [x] Create a reproducible staging script and launch wrapper.
- [x] Deploy the first native release payload.
- [x] Record automated screenshot and Gamescope smoke evidence.
- [x] Exercise the bounded timedemo and live presentation command.
- [x] Build the production payload inside a pinned SteamRT4 SDK container.
- [x] Deploy and smoke a clean-source SteamRT4 artifact.
- [x] Record SteamRT4 timedemo and live presentation evidence.
- [ ] Validate the SteamOS presentation profile at native 1280x800.
- [ ] Record interactive Linux/Gamescope/controller acceptance.
- [ ] Record the first reproducible release performance baseline.
