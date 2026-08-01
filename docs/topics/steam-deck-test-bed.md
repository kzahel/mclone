# Steam Deck Test Bed

Topic: steam-deck-test-bed

Status: active test lane. As of 2026-08-01, the first retail Steam Deck is a
direct physical Wi-Fi testbed reachable from the MacBook and registered in the
private dotfiles inventory. Project-neutral status, doctor, Devkit transport,
game upload/registration/launch, file transfer, and panel control live in the
public `kzahel/steamdeck-testbed` repository. Mclone's SteamRT4 staging,
screenshot smoke, and bounded performance collection pass through that shared
helper. Manual launch-button release/wake, controller, persistence,
suspend/resume, and dock/undock acceptance remain open.

## Scope

This topic owns mclone's repeatable Steam Deck build, payload, playtest, and
performance-validation policy. The public
[`steamdeck-testbed`](https://github.com/kzahel/steamdeck-testbed) repository
owns project-neutral physical-device operation. It does not build mclone or
know about its asset pack, world roots, launch modes, results, or assertions.

Steam Deck remains an ordinary shared desktop-flat Linux target. It consumes
the ordinary controller contracts, renderer, assets, scene, and persistence
paths rather than a separate gameplay implementation.

Long-lived machine addresses, local account state, package inventories, and
other private host facts do not belong here. Keep them in the private dotfiles
`machines/steamdeck` record and its provisioning history.

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

The Devkit Client provisions Valve's managed SSH listener, host key, and
utility scripts during pairing. The public testbed helper uses that listener
directly with strict host-key checking and each authorized development
machine's ordinary SSH key. Do not enable a separate general-purpose SSH
service, set a password, or unlock SteamOS's read-only root solely for this
workflow.

The first pairing was verified on 2026-07-23 by authenticating with the
Devkit-generated key, synchronizing Valve's utility scripts, and completing a
machine-readable `steamos-get-status` query while the Deck was in its Gamescope
session. The laptop was a one-time pairing/bootstrap path, not an ongoing
controller or relay. Keep the device address, exact host paths, and local
account state in private dotfiles.

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

The mclone adapter is
[`scripts/steam-deck.sh`](../../scripts/steam-deck.sh). It builds and stages
mclone, emits a `steamdeck-testbed.game.v1` manifest, owns safe mclone process
replacement and bounded-result policy, and delegates all physical-device
operations to `~/code/steamdeck-testbed/bin/steamdeck`. Its package commands
are:

```bash
pnpm steamdeck:status
pnpm steamdeck:power-status
pnpm steamdeck:screen-off
pnpm steamdeck:screen-on
pnpm steamdeck:stage
pnpm steamdeck:upload
pnpm steamdeck:deploy
pnpm steamdeck:launch
pnpm steamdeck:stop
pnpm steamdeck:smoke
pnpm steamdeck:perf
pnpm steamdeck:matrix
pnpm steamdeck:matrix:smoke
pnpm steamdeck:pull-results -- RUN_ID

pnpm steamdeck:build:steamrt4
pnpm steamdeck:stage:steamrt4
pnpm steamdeck:deploy:steamrt4
pnpm steamdeck:install:production
pnpm steamdeck:deploy:production
pnpm steamdeck:smoke:steamrt4
pnpm steamdeck:perf:steamrt4

pnpm steamdeck:auto-deploy:on
pnpm steamdeck:auto-deploy:off
pnpm steamdeck:auto-deploy:status
pnpm steamdeck:auto-deploy:log
```

The wrapper defaults to the private `steamdeck` SSH alias. On a machine with a
Devkit Client key it preserves that key as a compatibility default; otherwise
the SSH alias selects the development machine's normal authorized key.
Override either boundary without recording private state in this repository:

```bash
MCLONE_STEAM_DECK=HOST pnpm steamdeck:status
MCLONE_STEAM_DECK_TESTBED=PATH pnpm steamdeck:status
```

### Unattended AC power and screen control

The paired Deck is configured with `IdleSuspendACSeconds=0`, so Gaming Mode
does not idle-suspend it while AC remains connected. Its battery idle-suspend
timer remains 900 seconds. This is the desired unattended-test-bed policy:
losing AC still has a bounded battery safeguard, while an AC-powered Deck
retains SSH and Devkit reachability without a long-running inhibitor process.

Use the wrapper, which delegates to the shared physical testbed helper, to
inspect that state and sleep only the built-in panel:

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

Each payload upload also registers `Devkit Game: screenoff` in
Gaming Mode. It is a tiny native shortcut, not an mclone gameplay mode: launch
it like a game after a manual test to arm the same local wake watcher, disable
the internal connector, and return to the library with SteamOS and SSH still
running. The watcher waits for the launch-button press to be released before
arming, so the A press used to start the shortcut cannot immediately wake the
panel. The next mapped Deck button restores the panel. The physical power
button deliberately retains its ordinary whole-device suspend behavior.
The current SteamOS Devkit client accepts simple identifier characters rather
than a display label with spaces or hyphens, hence the deliberately plain
`screenoff` registration name. Steam's shortcut properties may give the tile a
friendlier display name without changing its deployment identity.

The 2026-07-25 on-device registration and remote `run-game` acceptance pass
started `screenoff`, changed `card0-eDP-1` to `disabled`, returned from the
short-lived control script, and left the one-shot watcher active on the built-in
controller. Before the test, no mclone client process was live, GPU busy was
zero, and the system fan was stopped; an old interactive-owner file referred to
a dead process and was not evidence of background game work.

The Gamescope convar is a forced state: Gamescope does not clear it merely
because ordinary input arrives. Before disabling the connector, the public
testbed helper deploys and arms a one-shot, non-grabbing user service that
watches the built-in Deck controller's read-only HID reports. The next mapped
Deck button press clears the convar and exits the watcher. Touch and motion
alone are deliberately ignored, while the physical power button retains its
system suspend semantics. If the watcher cannot arm, `screen-off` refuses to
blank the panel. The byte masks follow the upstream Linux
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
and assembles `dist/steamdeck` with a SHA-256 build receipt. `upload` passes
that directory and the generated generic manifest to the public testbed for
validated Devkit upload and registration as `Devkit Game: mclone`. `deploy`
also launches interactive play. `launch` restarts an already uploaded
interactive build. `stop` invokes only the ownership-aware stop command in the
staged mclone payload; the public helper never performs a process-name sweep.

The payload owns one interactive Devkit process through a lifetime file lock
and a PID plus Linux process-start identity. `launch` and `deploy` stop that
exact owner before issuing Valve's `run-game` RPC; a direct duplicate payload
start is rejected, and stale owner records self-heal. Stop sends `TERM`, waits
five seconds, and escalates to `KILL` only if that exact development process
does not exit. This is iterative Devkit replacement policy, not a name-wide
process sweep or an assumption about production Steam process management.

Launch replacement can be abrupt and is not the persistence acceptance path.
Use normal in-game quit/quit-to-title behavior when validating save lifecycle.
The shared menu-first and explicit-entry decision is recorded in
[`client-entry-lifecycle.md`](client-entry-lifecycle.md).

Physical validation on 2026-07-24 launched one SteamRT4 menu process, launched
again, and proved that the recorded old PID exited before a new PID became the
sole `mclone-native-client`. A direct concurrent payload invocation exited with
the dedicated duplicate-owner status while the existing owner remained the
only process. `steamdeck:stop` then removed the owner record and left zero
client processes.

The two complete production-style commands share asset/build/stage/upload
policy. Each checks the asset pack against the tracked lock before the
expensive build. If the check fails, it rebuilds the ignored archive once and
checks again; neither rewrites the tracked lock automatically, so intentional
source/tool changes still require inspection and an explicit
`pnpm assets:pack:write-lock`. Once assets pass, both build the ordinary
`x86_64` client inside the pinned SteamRT4 SDK, stage the binary/pack/receipt,
upload incrementally, and register `Devkit Game: mclone`. The upload also
asks the public testbed to register `Devkit Game: screenoff` against its own
small project-neutral helper payload without a compatibility runtime.

`steamdeck:install:production` stops there, leaving the title ready for the
user to launch and refreshing the screen-off shortcut without changing the
current foreground game or display state.
`steamdeck:deploy:production` additionally launches the title and is therefore
the explicit interactive manual command.

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
`main` SHA, coalesces quick successive pushes, and asks the shared testbed for
a bounded read-only probe. An unavailable Deck is an explicit logged skip. A
reachable Deck is built and deployed from a clean reusable sibling worktree at
the exact pushed commit, independently of the web deploy worker. Push
automation uses `steamdeck:install:production`: it uploads and registers the
new build but deliberately does not launch it or wake the panel. Generated
asset archives and SteamRT4 target/cache state remain ignored and local. The
worker incrementally copies the extracted source tree into its worktree before
packing so manifests remain repository-relative and web/Deck pack generation
cannot race through a shared output archive.

Inspect structured state or the latest log lines with:

```bash
pnpm steamdeck:auto-deploy:status
pnpm steamdeck:auto-deploy:log
```

State is under `.git/mclone-steam-deck-after-main-push/` with separate pending,
completed, skipped, failed, summary, and append-only log records. The
machine-local host is read from `mclone.steamDeckHost` in this checkout's local
Git configuration, with `MCLONE_STEAM_DECK` retaining highest precedence.

The 2026-07-24 end-to-end checks covered both outcomes. A reserved unreachable
address produced `deck-unreachable`, cleared the pending request, and exited in
one second. A reachable clean-worktree deploy built commit `ae4b64a4` through
the pinned SDK, recorded `dirty=false` and `GLIBC_2.39`, uploaded, registered,
and launched in 92 seconds with a cold target directory. Repeating the exact
manual production command used the cached 0.27-second Cargo build and completed
the full check/build/stage/upload/launch path in 7.15 seconds. The Deck remained
SSH-reachable with its internal connector disabled.

After separating installation from interactive launch, the real background
worker built and installed commit `42cc3ca0` in nine seconds (eight seconds in
the production install command). Its log contained asset verification,
SteamRT4 build, upload, and shortcut registration, with no `run-game` RPC or
command-line launch. The Deck remained SSH-reachable and the internal connector
remained disabled after completion.

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
occlusion, frame pacing, and FPS cap controls. Settings categories and Controls
Help now use a shared clipped scroll region: wheel or trackpad input scrolls
it, controller focus brings hidden rows into view, headings and Back/Done
remain fixed, and compact widths switch from two columns to one. Direct
touch-drag and draggable-scrollbar interaction remain future input work; Deck
controller focus and the mouse-emulating trackpads are covered. Future
detailed settings should be added only with real runtime ownership and effects
(for example particles, clouds, entity distance, mipmaps, UI scale, ambient
occlusion, and fullscreen/video modes), not as cosmetic menu rows.

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
recorded in the private laptop ledger. Ordinary processes need a new
login/session after the group change; the SteamRT4 builder can re-enter the
already-authorized group through `sg` when invoked from an older shell. The
Docker daemon itself was not reconfigured.

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

## 2026-07-24 Touchscreen Input

The original physical Deck report was that a direct touchscreen tap on Quit
at the title screen did nothing. The native desktop host observed
`WindowEvent::Touch` only as capability activity and discarded its position
and phase. Under the Deck's Gamescope X11/Xwayland presentation, winit emits
native touch while suppressing pointer-emulated mouse buttons, so the missing
touch route could not fall back to a click.

The `touchscreen-input` series now:

- routes the first menu contact as direct pointer down/move/up and cancels
  without activation;
- uses the same gameplay touch adapter, HUD, settings, and preference document
  as Android and browser clients;
- advertises touch at startup for `--platform-profile steamos`;
- leaves menu tapping available when gameplay Touch Controls is Off; and
- keeps Auto source switching shared across flat hosts.

No Steamworks API is needed. Standard winit/OS touch events are authoritative;
future Steam Input support remains optional controller work for action sets,
trackpads, gyro, rear buttons, rebinding, and glyph origins.

Host Rust tests, native and WASM builds, an ARM64/x86_64 Android APK, Android
AVD touch smoke, and headed-Wayland mobile-browser touch smoke passed. Android
and browser touch HUD/menu/options captures were inspected. The remaining gate
is a physical Gaming Mode pass using the checklist in
[`touchscreen-input.md`](touchscreen-input.md#physical-steam-deck-acceptance).

## 2026-07-24 Gamescope Lock And RD13 Accounting A/B

The reported failure looked like a device hard lock: the game stopped
responding, Gamescope counters stopped updating, and the Steam button could
not render its menu. SSH and the Linux kernel remained alive. There was no
kernel GPU-reset or OOM evidence, and Steam still observed focus/button state,
so this was a wedged Gaming Mode userspace/display session rather than a
kernel hard lock. Stopping only the game did not restore presentation;
`steamosctl switch-to-game-mode` restarted the Gamescope/Steam session and
restored the 89.887 Hz panel.

The controlled screenshot matrix did not reproduce a screenshot-triggered
wedge:

- two `gamescopectl screenshot` calls on the Steam home screen passed;
- two calls on a fresh mclone title screen passed at 90 Hz;
- two calls on a fresh static RD13 aerial world passed;
- calls after the unbounded-accounting run had degraded into the 50-65 FPS
  range also passed; and
- post-fix calls at fresh, five-minute, and ten-minute points returned in
  4-6 ms, wrote valid 1280x800 RGBA captures, left the client alive, and did
  not move Gamescope from 90 Hz.

The first apparent automated recurrence was not a compositor failure. The
bounded run had completed and its cleanup intentionally slept the internal
panel before the next screenshot probe. Waking `card0-eDP-1` immediately
resumed presentation. Future classification must check connector enabled
state as well as Gamescope/app liveness.

The autonomous reproduction lane is:

```bash
MCLONE_STEAM_DECK_BUILDER=steamrt4 \
MCLONE_STEAM_DECK_SKIP_ASSET_CHECK=1 \
pnpm steamdeck:gamescope-repro
```

It launches seed 12345 directly into a view-settled transient RD13 world,
places a no-clip camera at `(8, 196, 8)` looking vertically at `(8, 64, 8)`,
and records a long window-frame report. The corresponding
`--window-camera-eye` and `--window-camera-target` flags require each other
and are restricted to window-report runs, so no human loading, render-distance
editing, flight, or camera positioning is needed.

Before the accounting correction, run
`20260724T082305Z-c3846792fe7e-gamescope-repro-3263339` used SteamRT4 binary
SHA-256
`2925ea4f17a677807a5bbd303eb17d10fe8700ec1509729c706baa5beeab359b`.
With a static view and no streaming input, it measured approximately:

- 87 FPS and 709,984 KiB RSS at 2:40;
- 79.5 FPS and 718,900 KiB RSS at 3:43;
- 73 FPS and 728,104 KiB RSS at 4:52; and
- 64.6 FPS around 5:28, with screenshots still completing.

A five-second late `perf` sample attributed about 29% of all process cycles to
`FrameAccumulator::summary_report`, percentile sorting, and cloning/sorting
`worst_frames`. This is the primary cause of the time-dependent low-FPS,
low-aggregate-utilization report.

The fixed comparison run
`20260724T084815Z-c3846792fe7e-gamescope-repro-3287052` used SteamRT4 binary
SHA-256
`c5be566f4bf22ceacba7219ea78e0734b067d1c971a8a85df9a9e4399a21a297`.
It held 89.98-90.01 FPS through ten minutes and then completed all 72,000
requested frames: 72,000 presented, zero skipped, and zero reconfigured at a
native 1280x800 world and output. The full-window frame-wall p50/p95 were
11.123/12.218 ms; the last 6,000 frames averaged 11.113 ms. `VmData` stayed
exactly 792,936 KiB from the first recorded sample through completion, RSS
settled near 725-727 MiB, and no accounting symbol appeared above 0.5% in the
late profile.

The remaining static RD13 work is not fill-rate limited. The fixed run used
about 1.33-1.45 CPU cores and 34-36% of the client GPU graphics engine.
Thread accounting split that into roughly 81% of one logical CPU on the
render/main thread and 53% on the integrated-server thread; aggregating that
over the Deck's eight logical CPUs explains the roughly 19-23% UI reading.
The late profile's real hot spots were integrated-server distance-level scans,
terrain culling/traversal, pending-render-work scans, client chunk snapshots,
and draw encoding. Looking down from altitude exposes more terrain and makes
those visibility/draw-preparation costs worse even while total CPU and GPU
percentages remain low. The final runtime had zero generation jobs,
publications, render-compile jobs, or upload backlog, but still reported 112
pending render chunks; the meaning and repeated scan cost of that stationary
set is a concrete next investigation.

These are controlled dirty-tree diagnostic A/B rows, not a release threshold:
the source changes were uncommitted and the unrelated asset-lock mismatch was
explicitly bypassed. The exact binary hashes and pulled `/tmp` result bundles
make the two rows reproducible without mislabeling their provenance.

The original userspace Gamescope wedge is therefore not proven to have been
caused by `gamescopectl screenshot`. The strongest current explanation is that
the screenshot happened during severe main-thread/accounting collapse, with a
separate Gamescope or explicit-sync failure possible. Keep the controlled
repro lane and automatic session recovery, but do not treat screenshot capture
as the root cause without a panel-enabled recurrence.

## 2026-07-24 Stationary And Traversal Matrix

The Deck lane now drives stationary and moving camera tests without a human
loading a world, changing render distance, flying, or positioning the view.
`--window-camera-velocity X,Y,Z` translates the diagnostic eye and target in
world units per second and immediately reconciles the local player/server
interest center. `--window-frame-report-seconds` gives moving cases the same
wall-clock duration and therefore the same travel distance even when their
frame rates differ.

A view-settled report now settles twice: once through ordinary startup and once
after restoring and reconciling the exact diagnostic camera. The gate requires
the accepted server view to be complete, every requested chunk to be
client-resident, and target render, compile, and upload work to be clear in the
same driven observation; it does not add a fixed stable-tick delay. Global
server queue depths remain diagnostics rather than settlement blockers. This
prevents a fast but visibly incomplete row from being accepted as a rendering
improvement. It deliberately does not wait for all scheduled fluid simulation
to end.

`pnpm steamdeck:matrix` builds in SteamRT4, wakes the native panel, and runs:

- stationary oblique and top-down views at RD5, RD8, RD10, and RD13;
- top-down traversal at 16 blocks/second for 20 seconds at the same distances;
- RD13 oblique traversal; and
- an RD13 top-down traversal A/B with adaptive render admission enabled.

Each row records frame/surface timing, render and culling load, server and
publication-budget state, generation/update/render queues, rebuild/upload
work, process CPU use, GPU busy/clock/temperature, memory, and thread count.
The summarizer trims system samples to the measured frame interval. The smoke
variant proves the settled stationary-to-traversal transition with two RD5
rows.

The first distance-correct full run was
`20260724T122203Z-93ce5851e860-perf-matrix-3546078`. It used dirty-tree
SteamRT4 artifact SHA-256
`e0c3b3b34c6f49a6a1ce5da6df97575c539bd00b6bf91e21b42bda8164a3e871`,
completed all 14 rows, restored the interactive shortcut, slept only the
internal panel, and left SSH reachable. Key native-panel results were:

| Case | FPS | p95 frame | p95 encode | Avg drawn sections | Avg CPU cores | Avg GPU |
|---|---:|---:|---:|---:|---:|---:|
| stationary top-down RD5 | 89.8 | 12.50 ms | 5.34 ms | 282 | 0.70 | 15.2% |
| stationary top-down RD10 | 86.1 | 16.29 ms | 13.32 ms | 1,053 | 1.28 | 27.5% |
| stationary top-down RD13 | 68.2 | 21.10 ms | 20.09 ms | 1,151 | 1.61 | 39.5% |
| traverse top-down RD5 | 89.8 | 13.12 ms | 6.71 ms | 320 | 1.74 | 29.7% |
| traverse top-down RD10 | 68.4 | 21.26 ms | 16.20 ms | 1,047 | 2.59 | 48.5% |
| traverse top-down RD13 | 46.1 | 28.02 ms | 26.09 ms | 1,178 | 2.96 | 55.9% |
| traverse oblique RD13 | 54.0 | 24.62 ms | 21.90 ms | 546 | 2.83 | 38.2% |

Every traversal covered approximately 320 blocks. RD5/RD8/RD10/RD13 published
253/343/403/489 feature chunks respectively. The maximum publication backlog
was 2/14/17/22 chunks and the worldgen mailbox itself never exceeded one job.
The client update queue stayed almost empty, with maximum oldest-applied ages
between 47 and 121 ms. At RD13 the pending render-chunk set nevertheless grew
to 250 while only two compile jobs were pending. Adaptive render admission did
not improve this row: 45.8 versus 46.1 FPS, near-identical frame tails, draw
counts, publication counts, and render queue depth.

Disabling adaptive publication from process start is not a valid RD13
comparison. In reconnaissance run
`20260724T120439Z-93ce5851e860-perf-matrix-3533259`, that row could not reach
the then-current complete idle gate within 120 seconds and wrote no frame
report. Earlier incomplete-camera runs made the same policy look artificially
fast because most terrain had not reached the renderer. Keep completeness and
equal travel as benchmark invariants.

### Interpretation and next optimization order

The high-altitude loss is primarily CPU draw preparation/encoding, not simple
fill rate. Top-down RD13 traversal raises average visible draws from 546 to
1,178 sections and encode p95 from 21.90 to 26.09 ms. GPU busy also rises from
38.2% to 55.9%, so view-dependent fragment/geometry load is real, but the GPU
is not saturated while the CPU encode time already exceeds the 11.125 ms
90 Hz period. A five-second live RD13 sample measured the main/render thread at
93.6% of one logical CPU, the integrated-server thread at 57.1%, and one render
compile worker at 16.6%. That is only about 1.67 of the Deck's eight logical
CPUs, explaining a low aggregate meter while the latency-critical main thread
is effectively full.

Freshly idle does not mean terrain-static. Stationary rows began with zero
generation jobs and publications, yet rebuilt 400-834 sections in 20 seconds
and ended with 116-367 scheduled fluid ticks. Top-down RD13 improved from
59.4 FPS in the first quarter to 77.9 in the last as quarter rebuilds fell from
237 to 67. Frames that happened to accept no rebuilt section averaged 80.1 FPS;
frames accepting rebuild work averaged 53.6 FPS. The older ten-minute soak
eventually held 90 Hz once this activity decayed. Therefore benchmark both a
freshly settled world and a time-soaked world, and add a scheduled-fluid freeze
A/B before treating remesh cost as unavoidable steady-state rendering.

The measured low-hanging CPU candidates, in order, are:

1. Cache `ChunkDistanceManager::active_levels` across unchanged tickets.
   Stationary server ticks currently rebuild the full propagated `BTreeMap`
   repeatedly; RD13 scheduler p95 was 40-42 ms stationary and 82-120 ms during
   traversal.
2. Remove diagnostic full-set scans from every frame. Scene preparation calls
   `pending_render_chunk_count` before and after work; each call allocates a
   `BTreeSet`, walks dirty/resident/inflight sections, and repeatedly looks up
   client snapshots. The traversal-ready stamp similarly reconstructs the
   complete loaded-chunk set before discovering that a stationary frame did
   not change it. Use mutation generations/cached counters and refresh rich
   diagnostics at their publication cadence.
3. Cache static-camera cull/draw plans by camera and render-generation stamp.
   For moving top-down views, reduce per-section CPU encoding with spatial
   hierarchy plus batching/indirect or multi-draw work; cache reuse alone
   cannot solve traversal.
4. Attribute scheduled-fluid mutations separately, then coalesce dirty-section
   marking and avoid rebuilding unchanged or offscreen sections. Preserve
   vanilla simulation semantics; a freeze row is diagnostic, not a proposed
   gameplay default.

No renderer or scheduler optimization was applied in this matrix slice. These
are measured candidates for focused A/B changes, with RD5 traversal retained
as the frame-pacing guardrail and RD13 as the pressure case.

The executable proof and implementation campaign now lives in Tactical
[`232`](../tactical/232-steam-deck-rd10-plus-performance.md). It owns the
paired A/B rules, instrumentation controls, correctness gates, ordered
bookkeeping/culling/server/fluid/worker/batching slices, and result ledger.

### Accepted RD10+ rendering result

The first campaign-changing renderer result is the shared terrain arena and
multi-draw series ending at `e5a45346`. Exact-hash physical run
`20260724T153005Z-e5a453469c10-perf-matrix-3868806` used SteamRT4 binary
SHA-256
`82a0ebf512d2f7ee0b220a3b44669665b73256c6819f8a88c934d2e8df42c897`.
It retained unique section geometry while replacing per-section GPU buffers
and most solid/cutout submission calls with paged shared arenas and
feature-detected multi-draw-indirect groups. Translucent order stays exact and
direct; stereo and multiview share the arena storage through their existing
direct paths.

The cumulative RD13 top-down traversal result, compared with the
column-readiness parent, was:

| Metric | Before | Arena/multi-draw |
|---|---:|---:|
| FPS | 76.6 | 84.6 |
| frame p95 | 19.65 ms | 19.46 ms |
| surface encode p95 | 14.38 ms | 11.43 ms |
| terrain encode p95 | 4.18 ms | 0.85 ms |
| GPU terrain p50 | 2.68 ms | 2.62 ms |

The full equal-distance top-down traversal curve on the final artifact was:

| Render distance | FPS | frame p95 | frame p99 |
|---:|---:|---:|---:|
| 5 | 89.9 | 12.69 ms | 14.03 ms |
| 8 | 89.8 | 12.92 ms | 14.22 ms |
| 10 | 89.0 | 13.10 ms | 20.44 ms |
| 13 | 84.3 | 20.28 ms | 25.00 ms |

All four rows traveled approximately 320 blocks and completed their expected
253/343/403/493 feature and light publications. Stationary RD5 through RD13
held 89.5-90.0 FPS. The RD13 oblique traversal control reached 87.2 FPS,
again showing that top-down cost follows exposed terrain work rather than
aggregate CPU or GPU saturation.

Treat RD8 as the refresh-rate-average handheld default class on this artifact.
RD10 is a reasonable near-90 quality setting, but neither is a strict
11.125 ms p95 guarantee. RD13 is a supported stress/quality setting at roughly
84 FPS in the hardest traversal row, not a 90 Hz promise. These are
native-panel results for this world and workload, not a universal content
guarantee.

Deterministic parent/candidate captures differed by zero pixels. Mono,
per-eye stereo, full-frame multiview, placed-terrain, and resource-ownership
tests passed. The final RD13 traversal used approximately 424 MiB of vertex
data and 64 MiB of index data; bounded vertex pages are required because the
Deck adapter exposes a 256 MiB maximum for one vertex buffer.

The later holder-plan slice removed the approximately 4 ms p95 server
reconciliation span. The remaining priorities are 1.3-3.4 ms
cull/occlusion work as distance grows, the 0.3-1.2 ms runtime-target admission
scan, and fresh-fluid remesh/upload plus GPU tails despite low steady GPU p50.

### Holder-plan and campaign closeout

Commit `d7344628` retains the sorted runtime-target plan behind ticket,
priority, topology, and lighting-policy generations. Unchanged server ticks
skip approximately 3,100 repeated holder updates while still scanning the
runtime targets that can become schedulable as persistence, generation, and
lighting complete.

Focused run
`20260724T160927Z-d7344628d640-perf-matrix-attribution-3906378` used
SteamRT4 binary SHA-256
`dc3ea20d378adbf366890ad1a4879150b5a0dffff347215dc7ce59cd098c7f8a`.
Against the instrumented parent, native RD13 traversal reconciliation p95
fell from 3.77 to 1.10 ms, holder-update p95 from 2.56 ms to zero, and frame
p95 from 19.69 to 16.53 ms. Travel remained 319.9 blocks, feature/light
publication remained 493/493, and 9,571 sections rebuilt.

Final full run `20260724T161657Z-d7344628d640-perf-matrix-3916887` produced:

| Render distance | traversal FPS | frame p95 | frame p99 |
|---:|---:|---:|---:|
| 5 | 89.9 | 12.60 ms | 14.47 ms |
| 8 | 89.8 | 12.83 ms | 14.46 ms |
| 10 | 89.5 | 12.98 ms | 14.61 ms |
| 13 | 83.6 | 20.97 ms | 25.23 ms |

All traversal rows covered approximately 320 blocks. RD13 oblique reached
87.8 FPS and 13.80 ms p95; adaptive top-down admission tied the native
83.6 FPS row and remains unrecommended. Every stationary RD5-RD13 oblique and
top-down row averaged 89.7-89.9 FPS.

A release-shaped build without `perf-diagnostics`, binary SHA-256
`aff985906db3610dbe3dd5fe7e0f543bf9be0bc26e8194046808dab9b5322808`,
ran as
`20260724T162954Z-d7344628d640-perf-matrix-attribution-3935581`.
Its native RD13 traversal was 85.0 FPS/18.21 ms p95 versus the diagnostic
run's 85.6/16.53; the half-resolution comparison moved oppositely. Treat the
difference as run variance, not measurable diagnostic overhead.

Fresh-fluid work is real but already coalesced: one final stationary native
row combined 2,105 fluid mutations into 418 rebuilt sections with zero stale
compiles. Frozen fluids remain a diagnostic control, not gameplay policy.
Further coalescing needs evidence of duplicate accepted meshes, not merely a
visible rebuild count.

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
- [x] Validate the SteamOS presentation profile at native 1280x800.
- [ ] Record interactive Linux/Gamescope/controller acceptance.
- [ ] Record physical touchscreen menu/gameplay acceptance.
- [x] Record the first reproducible release performance baseline.
