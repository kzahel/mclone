# Steam Deck Test Bed

Topic: steam-deck-test-bed

Status: active test lane. As of 2026-07-23, the first retail Steam Deck is
paired with the Linux deployment host. Automated staging, incremental upload,
SteamRT4 launch, screenshot smoke, and bounded performance collection pass on
the device. Manual controller, persistence, suspend/resume, and dock/undock
acceptance remain open, as does the production Steam Runtime SDK builder.

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
pnpm steamdeck:stage
pnpm steamdeck:upload
pnpm steamdeck:deploy
pnpm steamdeck:launch
pnpm steamdeck:smoke
pnpm steamdeck:perf
pnpm steamdeck:pull-results -- RUN_ID
```

The wrapper defaults to the paired device's mDNS name and the Devkit-managed
SSH key. Override discovery without recording a private address in the
repository:

```bash
MCLONE_STEAM_DECK=HOST pnpm steamdeck:status
```

`stage` builds the release client, verifies the checked-in asset-pack lock,
and assembles `dist/steamdeck` with a SHA-256 build receipt. `upload` performs
Valve's upload preparation, an incremental clean `rsync`, and registration as
`Devkit Game: mclone`. `deploy` also launches interactive play. `launch`
restarts an already uploaded interactive build.

`smoke` temporarily registers a bounded command that:

1. captures a deterministic 1280x800 offscreen frame from a fixed camera;
2. runs 300 frames through the live Gamescope swapchain and present path;
3. writes logs, JSON reports, a status file, and the PNG under the Deck user's
   state directory;
4. restores the normal interactive shortcut; and
5. pulls the result to `/tmp/mclone-steam-deck-results/RUN_ID`.

`perf` uses the same lifecycle for a 600-frame offscreen timedemo followed by
a 3600-frame, render-distance-10 Gamescope presentation run. It is a
repeatable developer benchmark, not an automatic performance acceptance
threshold. Record the Deck refresh rate, frame cap, TDP, thermals, clock
policy, and whether the display is docked before comparing runs.

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

## Production SteamRT4 Builder Requirements

Valve currently recommends Steam Linux Runtime 4 for new native Linux games
and recommends compiling inside the matching SDK container. The proposed
reproducible builder is:

1. Base a dedicated image on
   `registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk`, pinned by digest. As
   observed on 2026-07-23, Valve's stable `latest` alias identifies SteamRT4
   build `4.0.20260608.242786` and manifest digest
   `sha256:584939ebd7d2f1eec719e771fdde4ae3bd469ee741c783abb7fe812ddaaf3ee4`.
   Treat that as a proposed initial pin, not a forever version.
2. Add a pinned Rust toolchain. The current host uses Rust 1.97.0 and the
   workspace requires at least 1.92; use 1.97.0 initially and commit the pin
   with the builder.
3. Declare native build dependencies explicitly: a C/C++ build toolchain,
   `pkg-config`, ALSA development headers for `cpal`, and libudev development
   headers for `gilrs`. Add only other libraries demonstrated by a clean
   container build.
4. Run the container as the invoking host UID/GID, mount the repository
   read-only where practical, and use named/writeable Cargo registry, git, and
   target caches. Do not leave root-owned build artifacts in the checkout.
5. Build `mclone-native-client` with `cargo build --release --locked` for
   `x86_64-unknown-linux-gnu`, run the asset-pack lock check, and feed the
   resulting binary into the existing staging/upload wrapper.
6. Record the base-image digest, Rust version, Cargo.lock hash, source commit
   and dirty state, target/Rust flags, binary hash, asset-pack hash, and final
   dynamic dependency / GLIBC symbol audit in `build.json`.
7. Test the staged artifact under SteamRT4 on the Deck. A successful build in
   the SDK is necessary ABI evidence, but it does not replace the device
   smoke, interactive controls, persistence, or performance lanes.

Docker 29 is already installed on the current Linux host, but the logged-in
user cannot access its root-owned daemon socket. No Docker, group membership,
rootless-container, or project container configuration was changed during
bring-up. Before implementing the builder, choose deliberately between
rootless Podman/Docker (Valve supports both) and granting this user access to
the system Docker daemon. Membership in the `docker` group is effectively
root-equivalent and should not be an incidental setup step.

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

## Bring-up Ledger

- [x] Retail Steam Deck Developer Mode enabled.
- [x] Linux host Steam package and per-user Steam client bootstrapped.
- [x] Sign the Linux host into Steam and install App ID `943760`.
- [x] Pair/register the Deck through **Pair new host**.
- [x] Create a reproducible staging script and launch wrapper.
- [x] Deploy the first native release payload.
- [x] Record automated screenshot and Gamescope smoke evidence.
- [x] Exercise the bounded timedemo and live presentation command.
- [ ] Build the production payload inside a pinned SteamRT4 SDK container.
- [ ] Record interactive Linux/Gamescope/controller acceptance.
- [ ] Record the first reproducible release performance baseline.
