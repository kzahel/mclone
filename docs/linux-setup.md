# Linux Development Setup

This guide covers native Rust development and no-window offscreen validation
on Linux. It was first validated on Ubuntu 24.04.4 LTS on 2026-07-12, using an
AMD Radeon 890M through Mesa RADV. The full-frame screenshot path does not need
X11, Wayland, a window manager, or a `DISPLAY` value.

The Android, Android XR, desktop OpenXR, and browser lanes have additional
requirements. Use [`platforms.md`](platforms.md) for their validation commands
and the platform-specific READMEs for device setup.

## Ubuntu Packages

Install the command-line bootstrap tools, native compiler baseline, ALSA
development files required by `mclone-audio`, and a Vulkan runtime:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  ca-certificates \
  curl \
  git \
  jq \
  libasound2-dev \
  libvulkan1 \
  mesa-vulkan-drivers \
  openjdk-17-jdk \
  pkg-config \
  python3 \
  unzip \
  vulkan-tools
```

Why the less-obvious packages are present:

- `libasound2-dev` supplies `alsa.pc`. The Linux `cpal` audio backend will not
  compile without it, even for an offscreen run that does not play audio.
- `openjdk-17-jdk`, `curl`, `jq`, `unzip`, and Python are used by the Minecraft
  reference and oracle bootstrap scripts. JDK 17 or newer is supported.
- `mesa-vulkan-drivers` provides RADV, Intel ANV, and llvmpipe on Ubuntu. A
  vendor Vulkan driver can replace Mesa where appropriate.
- `vulkan-tools` is diagnostic rather than a build dependency, but
  `vulkaninfo --summary` is the quickest way to distinguish hardware Vulkan,
  software Vulkan, and device-permission failures.

The first validated build did not require CMake, Ninja, Vulkan headers, or a
window-system development package. Add such packages only when another lane or
a future dependency reports that it needs them.

Validated host versions were:

| Component | Version |
|---|---|
| Rust / Cargo | 1.97.0 (workspace minimum is 1.92) |
| Node | 25.2.0 (repository minimum is 20) |
| pnpm | 9.15.1 |
| Python | 3.12.3 |
| OpenJDK / javac | 17.0.19 |
| Mesa Vulkan | 25.2.8 |

## Rust, Node, And pnpm

The native workspace declares `rust-version = "1.92"`. Install rustup and use a
current stable toolchain:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup update stable
rustup default stable
rustc --version
cargo --version
```

Node is not needed by Cargo itself, but the repository's named validation and
asset commands are pnpm scripts. Install Node 20 or newer, then enable the
checked-in pnpm major through Corepack:

```bash
corepack enable
corepack prepare pnpm@9.15.1 --activate
node --version
pnpm --version
pnpm install --frozen-lockfile
```

Using nvm, a distribution Node package, or another Node version manager is
fine as long as `node` and `pnpm` are available in the shell.

## GPU Device Access On A Headless Host

A render node normally belongs to the `render` group:

```bash
ls -l /dev/dri
id
sudo usermod -aG render "$USER"
```

Log out and back in after changing group membership. To validate immediately
in a temporary group shell:

```bash
sg render -c 'vulkaninfo --summary'
```

The expected hardware result lists the real GPU. This warning is harmless on
a headless machine:

```text
'DISPLAY' environment variable not set... skipping surface info
```

If `vulkaninfo` reports `Permission denied` for `/dev/dri/renderD*` and lists
only `llvmpipe`, the process cannot open the hardware render node. The
offscreen smoke can still work through llvmpipe, but it will use CPU rendering
and should not be used for performance conclusions.

## Minecraft 1.17.1 Reference And Assets

Hydrate the gitignored vanilla client source tree and renderer assets from the
repository root:

```bash
./scripts/decompile-mc.sh 1.17.1 --parchment
```

The script is idempotent and performs the verified Mojang download, remap,
Vineflower decompile, Parchment parameter pass, and filtered asset extraction.
Important outputs are:

```text
reference/minecraft-1.17.1/client-deobf.jar
reference/minecraft-1.17.1/src/
reference/minecraft-1.17.1/extracted/
```

See [`reference-minecraft.md`](reference-minecraft.md) for standalone asset
extraction, server-jar, and Java oracle setup.

## Initial Build And No-Window Smoke

Build the native flat client:

```bash
cargo build \
  --manifest-path native/Cargo.toml \
  -p mclone-native-client \
  --bin mclone-native-client
```

Then run the full-frame offscreen client smoke:

```bash
pnpm native:desktop-offscreen:smoke
```

It writes `/tmp/mclone-desktop-offscreen.png`. Inspect the image rather than
treating a zero exit status as sufficient rendered-output validation. On a
machine whose current login session predates the `render` group change, use:

```bash
sg render -c 'pnpm native:desktop-offscreen:smoke'
```

The 2026-07-12 bring-up produced a valid 2560x1600 terrain frame with textured
blocks, cutout foliage, flowers, and actors both through llvmpipe and through
the Radeon RADV device, with no display server running.

This proves the native compile and offscreen lane, not the interactive Linux
window path. Interactive validation still requires a working X11 or Wayland
session and was not available on the first headless host.

## Common Failures

- `rustc ... is not supported` with a requirement for Rust 1.92: run
  `rustup update stable` and confirm the active `rustc --version`.
- `Package 'alsa' ... not found` or missing `alsa.pc`: install
  `libasound2-dev`; changing `PKG_CONFIG_PATH` is not needed for Ubuntu's
  normal package layout.
- hardware GPU `Permission denied`: add the user to `render`, then start a new
  login session or use `sg render -c ...` temporarily.
- no `DISPLAY` or `WAYLAND_DISPLAY`: expected for the offscreen smoke. Do not
  add Xvfb or a hidden window just for this validation path.
- no Vulkan device at all: install a working Vulkan driver, verify `/dev/dri`,
  and rerun `vulkaninfo --summary` before diagnosing the renderer.
