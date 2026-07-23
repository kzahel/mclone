# Steam Deck Test Bed

Topic: steam-deck-test-bed

Status: active provisioning. As of 2026-07-23, the first retail Steam Deck has
Developer Mode enabled and the Linux deployment host has a working Steam
client plus the SteamOS Devkit Client payload. The host is paired and its
managed SSH path is verified; the first native mclone deployment remains open.

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

## Bring-up Ledger

- [x] Retail Steam Deck Developer Mode enabled.
- [x] Linux host Steam package and per-user Steam client bootstrapped.
- [x] Sign the Linux host into Steam and install App ID `943760`.
- [x] Pair/register the Deck through **Pair new host**.
- [ ] Create a reproducible staging script and launch wrapper.
- [ ] Deploy the first native release payload.
- [ ] Record interactive Linux/Gamescope/controller acceptance.
- [ ] Record the first reproducible release performance baseline.
