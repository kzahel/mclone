# Building and trying Mclone

Mclone is an experimental voxel game. The [web game](https://mclone.kzahel.com/play/)
is the easiest way to try it. [Nightly downloads](https://github.com/kzahel/mclone/releases)
are experimental builds; worlds and formats can change between revisions.
Project licensing is TBD. Third-party content retains its own notices.

## Build from a fresh checkout

Install Git, Python 3, [Rustup](https://rustup.rs/), and Node **22.23.2**
(`.node-version`). Rustup uses the repository's pinned **Rust 1.98.1**, including
the Wasm target. Use **pnpm 9.15.1** (`npm install --global pnpm@9.15.1`).

On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y ripgrep build-essential pkg-config libasound2-dev libudev-dev libvulkan1 mesa-vulkan-drivers
```

On macOS, install Xcode Command Line Tools (`xcode-select --install`). On
Windows, install Visual Studio Build Tools with the C++ desktop workload and
Windows SDK, and Git for Windows. Run Cargo from PowerShell or Git Bash,
using the native MSVC toolchain. Bash scripts use Git Bash, not WSL.

```sh
git clone https://github.com/kzahel/mclone.git
cd mclone
pnpm install --frozen-lockfile
pnpm --dir tools/texture-lab install --frozen-lockfile
pnpm assets:pack:first-party
pnpm assets:validate:first-party
cargo run --locked --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client
```

No Minecraft installation, reference files, decompilation, external asset
archive, or oracle bootstrap is required. The asset builder uses original
source art, generated placeholders, and the tracked audio/provenance inputs.
Allow several gigabytes for dependencies and build outputs. Cold compilation
on a small VM can take tens of minutes.

## Web build

```sh
pnpm exec playwright install chromium
pnpm native:web:bundle
```

This builds `dist-native-web/`: the game, project hub, labs, and World Explorer.
Playwright Chromium generates lab thumbnails; Linux may also need
`pnpm exec playwright install --with-deps chromium`. The bundler installs
locked lab dependencies, each thumbnail tool's matching Playwright browser,
and the pinned wasm-bindgen CLI as needed.
`pnpm native:web:build` only compiles Wasm; it does not produce a hosted bundle.

For local interactive development, run `pnpm native:web:serve` after building
assets. Static hosting of the downloadable bundle must serve `.wasm` as
`application/wasm` and use the isolation headers in `worker/_headers`.
Deployment is a separate maintainer operation (`pnpm native:web:deploy`).

## Desktop and headset builds

```sh
cargo build --locked --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client --features xr
cargo build --locked --release --manifest-path native/Cargo.toml -p mclone-dedicated-server
python3 scripts/package-desktop.py --platform linux-x64
```

Use `linux-arm64`, `windows-x64`, or `macos-arm64` for a matching native host.
The packager does not cross-compile. It writes ZIPs and SHA-256 checksums to
`dist-release/`, with packs beside the executable or in macOS app resources.
Run the complete extracted folder from any working directory. Desktop
packages include `--desktop-xr` support; supply an installed OpenXR runtime for your
headset. Desktop builds are not publisher-signed or notarized.

Each desktop archive also includes `mclone-dedicated-server`. Use `--help` for
hosting options and `--world-dir` with a path outside the installation for
persistent server worlds. CI checks server startup, initial protocol data,
clean shutdown, and world database integrity.

For Android/Quest, install JDK 17, Android SDK platform/build-tools 35,
NDK **27.0.12077973**, `cargo-ndk` **4.1.2**, and the Rust
`aarch64-linux-android` target. Drive builds through the scripts:

```sh
pnpm native:android:apk --release
pnpm native:android-xr:apk
```

The scripts resolve SDK/NDK locations and report exact missing prerequisites.
Nightly APKs use a consistent signing key and increasing version codes;
local builds use the development signer unless nightly signing is configured.
A development-signed installation cannot be updated in place by the nightly
signer. Preserve any worlds before replacing that installation. Install or
update a matching nightly with `adb install -r path/to.apk`.
The version code comes from the development workflow's run counter. Before
renaming or replacing that workflow, preserve a version-code floor above the
last published APK; a new workflow counter alone may start too low.

## Checks and CI

```sh
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --locked --manifest-path native/Cargo.toml -p mclone-app-runtime --lib
pnpm assets:pack:first-party:test
python3 scripts/test-public-assets.py
python3 scripts/check-public-assets.py dist-native-web
```

[Development checks and nightlies](https://github.com/kzahel/mclone/actions/workflows/development.yml)
run shared correctness, architecture guards, asset provenance, and secret scans
on main pushes and PRs. These checks compile test code and validate original
assets, but do not build distribution packages or upload artifacts.

The daily 03:17 UTC schedule and manual workflow runs build the complete
desktop, web, Android, and Quest package matrix from one first-party asset
artifact. Scheduled runs publish dated prereleases; manual runs publish only
when `publish` is selected. Normal pushes cannot cancel a running nightly.
Package artifacts expire after seven
days; successful publication prunes nightlies older than fourteen days.
A failed nightly keeps the prior successful downloads.
Maintainers can retain a specific nightly by adding `<!-- keep-nightly -->`
to its release notes. Before toolchain setup or builds, a small planning job
checks whether the scheduled revision already has a published nightly. If so,
it skips checks, asset generation, packages, and publication. A failed or draft
release does not suppress retries. Manual runs always allow another build.

Web deployment remains a separate local post-push hook, using the maintainer's
Wrangler login and build cache; GitHub Actions does not deploy the website.
See [local web deployment](native-web.md#local-post-push-deploy-hook).

If publication fails after all build jobs pass, maintainers can run
`gh workflow run publish-existing.yml -f run_id=<development-run-id>`.
It accepts only completed main development runs with every check and package
job green, downloads that run's artifacts, checks their embedded source
revision and hashes, and resumes a matching complete draft. It does not rebuild
the game or substitute newer artifacts. A partial or corrupted draft remains
hidden for inspection; remove that failed draft before retrying fresh uploads.

CI compilation is not GPU, headset, or gameplay acceptance. Current validation
receipts and limitations live in [Tactical 335](tactical/335-public-development-release.md).
The [platform guide](platforms.md) owns device and rendered acceptance commands.

## Further development

Start with [engine architecture](native-engine-architecture.md),
[terrain profiles](topics/world-generation-profiles.md), or [LOD](topics/lod.md).
The Rust workspace under `native/` owns the shared engine on every platform.
Optional comparative/oracle tooling is documented separately in
[the reference guide](reference-minecraft.md).
