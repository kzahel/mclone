# Tactical 335: Public Development Release

Status: **in progress, authorized 2026-09-12. Preserve history, leave the
project license TBD, and make the repository public before validating GitHub
Actions. Commit at substantial implementation and CI verification slices.**

Topics: `release-distribution-and-updates`, `asset-pack-profiles`

## Instruction synthesis

Make this repository public while preserving its existing history. Treat
Mclone as a side-project experiment: a WIP voxel game with experimental
terrain generation and LOD work, and limited gameplay. The deployed web game
is the easiest way to try it, but desktop, desktop XR, flat Android, and Quest
builds should also be available from CI and nightly releases. Shorten the
README and include a representative screenshot.

The maintainer explicitly accepts retaining the terrain generation code
reimplemented through oracle work and reference inspection. Removing that
code, restarting history, a parity rewrite, or a legal-clearance campaign is
not part of this plan. This decision does not authorize bundling proprietary
reference assets. Use Machine Control for fresh-installation evidence.

## Recommendation

Publish an experimental source project with ordinary GitHub prerelease
downloads. Keep one development repository and its history. Build approved
assets from checked-in inputs, package them with each client, and make the
public web bundle use the same asset boundary.

Do not make a launcher, automatic updater, storefront, final brand, commercial
model, gameplay completion, or elimination of provisional art a prerequisite.
The longer-term architecture in
[`release-distribution-and-updates.md`](../topics/release-distribution-and-updates.md)
remains a future option; manual nightly downloads are sufficient here.

## Investigation baseline

Source inspected: `76a9e4efe15eea92c332ab78ca9b7b465f8c8cb7`.
Observations below are source/HTTP evidence unless explicitly marked as a
successful execution. They are not a complete rights or secret audit.

| Finding | Consequence |
| --- | --- |
| No tracked `.github` workflows or root project license. | Add a small workflow set; project licensing remains TBD by maintainer decision. |
| `assets:pack:first-party` and `assets:validate:first-party` already exist. | Reuse their builders, deterministic packaging, and runtime resolution ledger. |
| First-party builders deliberately emit some original content under compatibility `assets/minecraft/...` names. | A namespace grep is not an authorship test. Validate input provenance, hashes, and the complete shipped inventory. |
| `scripts/deploy-native-web.sh`, including `--bundle-only`, runs `pnpm assets:pack` and copies the reference ZIP and sidecar into the output. | The current deploy/bundle command is unsuitable for public sanitized builds, even when runtime selection is Original. |
| On 2026-09-12, the deployed reference ZIP returned HTTP 200 with ZIP content type; its JSON manifest listed 7,196 entries, including 6,114 under `assets/minecraft/` and 2,270 textures in that namespace. | Correct the deployed asset boundary as well as the source build. A strict runtime ledger alone does not inspect unused downloadable files. |
| `native:web:build` is Cargo compilation; the deploy script also builds glue, labs, Explorer, and static files. | Distinguish a Wasm compile check, a runnable game bundle, and the complete hosted hub. |
| Desktop first-party discovery uses environment roots and repository/build-stage paths; it does not currently discover packs relative to the executable. | A bare binary ZIP is insufficient. Add executable/app-resource discovery and test from an unrelated working directory. |
| Flat Android builds optimized Rust but calls `assembleDebug`; Quest release Gradle uses the debug signing configuration. Both use `versionCode = 1`. | Add explicit distribution packaging, a stable nightly signing key, and monotonic Android version codes. |
| Ubuntu setup documentation omits `libudev-dev` and still presents reference hydration as part of setup; lab tooling needs a newer Node baseline than the root's broad `>=20` claim suggests. | Rewrite the clean public bootstrap from measured prerequisites, with reference tooling in an optional section. |
| There are ten tracked `.attachments` PNGs despite `.attachments` being ignored. Inspection shows old debug/loading/menu screens, with two also showing textured terrain. | Review these specific images and their font/UI/texture provenance. They are not evidence of raw extracted assets, and ignoring a directory does not untrack it. |
| The local all-ref scan found 3,280 commits; queried reference/extracted paths and JAR/ZIP/font extensions found only the Gradle wrapper JAR. | Useful preliminary evidence, not proof about every blob, remote-only ref, issue attachment, or release. Preserve history and report concrete exceptions. |

Relevant owners: [`asset pack topic`](../topics/asset-pack-profiles.md),
[`asset tooling`](../../tools/minecraft_assets/README.md),
[`platform matrix`](../platforms.md), [`web operations`](../native-web.md),
and [`LOD terminology`](../topics/lod.md). Current LOD means the shared
procedural-horizon clipmap; do not advertise the retired chunk-based Far LOD
as another currently available feature.

## Fresh-installation evidence

The investigation uses the common Machine Control CLI and a claimed,
disposable Ubuntu 24.04.4 ARM64 overlay. Exact private target identity, claim
IDs, access routes, and VM inventory stay outside this repository. The source
VM was retained; Machine Control confirmed that the disposable workspace was
discarded and released after evidence capture.

A `git archive` of the exact revision above supplies tracked source without
controller credentials, ignored reference files, generated assets, build
outputs, or dependency caches. This tests the source-download path; it is not
yet an anonymous GitHub clone or a prebuilt-package installation. A new guest
test user and empty task-local Rust/Node directories isolate project setup
from the prepared appliance's existing developer state.

The VM has 4 GiB RAM and was given two Cargo build jobs. Toolchains installed
from scratch: Rust/Cargo 1.98.1, Node 22.23.2 (archive checksum verified),
pnpm 9.15.1, and system Python 3.12. Root and Texture Lab frozen-lockfile
installs succeeded without a Git checkout; the hook installer correctly
skipped the source archive.

| Probe | Observed result |
| --- | --- |
| `pnpm assets:sfx:check` | Passed. The tracked provenance inventory contains 129 OGG files; the CLI's success message still hardcodes 121 and should be corrected. |
| `pnpm assets:pack:first-party:test` | All six tests passed. |
| `pnpm assets:pack:first-party` | Passed without a reference tree. Produced/staged authored (9,928,365 bytes), provisional (936,974 bytes), and diagnostic (903,188 bytes) packs. |
| `pnpm assets:validate:first-party` | Passed after the cold optimized engine build. Strict ledger: 160 first-party and 159 provisional resolutions, zero reference/unknown/diagnostic resolutions, two optional missing resources, and two suppressions. Prepared 309 block states, 163 atlas sprites, 28 figures, 44 audio families, and 129 samples. |
| `pnpm native:web:bundle` | Failed with exit 1 before Wasm compilation: `reference/minecraft-1.17.1/extracted not found. Run ./scripts/decompile-mc.sh first.` No reference bootstrap was run. |
| `cargo fmt --manifest-path native/Cargo.toml --all --check` | Passed on the controller checkout; this is separate from VM execution. |

The complete probe took roughly 23 minutes including provisioning and cold
compilation on this small VM. Cargo.lock remained byte-identical to the source
revision. This is a setup/cold-build observation, not a hosted Actions timing
or runtime performance claim. Reuse the compiled host graph for the test and
provenance steps instead of compiling it independently in several jobs.

This reproduces the public web source-installation failure and proves that
first-party asset generation and runtime preparation are a usable CI
foundation. It does not claim a successful player installation, full workspace test pass,
desktop/Quest package, or graphical acceptance. Those are explicit execution
gates below.

## 1. Close the public asset boundary

Deliver a public web bundler that has no reference prerequisite and fails
closed if disallowed inputs enter its output. Reuse the current script where
practical, but separate local reference development from distributable output.
Run the same check over desktop archives, APK ZIP contents, the game bundle,
and the complete hub, including lab previews and media.

- Build authored and provisional packs only from approved tracked inputs.
  Do not run diffusion or download model weights on CI. Keep curated images
  and recorded provenance as source inputs.
- Audit the two frozen texture inputs, provisional textures, structure atlas,
  fonts, and audio provenance; retain CC0 and vendored dependency notices.
  The runtime origin label is one check, not a substitute for input review.
- Check both absence of the reference archive and zero resolved
  reference/unknown runtime resources. Add a contamination test that places
  reference files beside the build and proves they are neither read nor shipped.
- Remove public reference fetches, stale catalog entries, and reference routes
  from the public build. The normal public launch must select Original without
  a special URL, prior preference, or local developer environment.
- Handle existing browser preferences that enabled Reference: public startup
  must recover to the available original selection and render successfully.
- Deploy the exact validated revision through the existing hosting mechanism,
  remove stale hosted files/cache entries where needed, and verify the former
  ZIP and sidecar return an actual 404/410 rather than a SPA success response.
- Inspect a screenshot from that deployment, plus request/provenance evidence
  showing the real game works without a reference download.

Acceptance: clean source can produce the public bundle; contaminated local
state cannot alter its approved content; old public reference URLs are gone;
fresh and previously used browser profiles both launch the original game.

## 2. Make public setup and checks reproducible

Add a small public development guide with exact commands and a pinned tested
Rust toolchain (1.98.1 from this probe), Node 22.23.2, pnpm 9.15.1, and Python
3.12 baseline. Verify the same pins on the other runners. Keep the
workspace MSRV distinct from the CI toolchain. Use Cargo `--locked` and every
relevant pnpm lockfile. Install root and Texture Lab dependencies explicitly;
install other lab packages only for their checks or hub builds.

The first-party portion already exercised in the VM is:

```bash
pnpm install --frozen-lockfile
pnpm --dir tools/texture-lab install --frozen-lockfile
pnpm assets:pack:first-party:test
pnpm assets:pack:first-party
pnpm assets:validate:first-party
```

Browser builds additionally need `rustup target add wasm32-unknown-unknown`
and the wasm-bindgen CLI version matching the locked Rust dependency (the
current bundler pins `0.2.125`). Provision those explicitly. The public bundle
command following this sequence becomes usable only after slice 1.

Ubuntu desktop prerequisites include `build-essential`, `pkg-config`,
`libasound2-dev`, and `libudev-dev`; rendering also needs an appropriate Vulkan
runtime/driver. JDK 17, SDK platform 35, and NDK `27.0.12077973` are currently
declared by Android build files; provision them from those declarations and
run the existing APK scripts. Do not introduce an alternate Android build path.

Use small repository scripts as the build/check API so local runs and Actions
execute the same steps. Ensure source archives work without a `.git` directory.
Keep optional oracle/reference regeneration outside default setup and CI.
Inventory tests that skip without local reference inputs and report that
coverage separately; do not download proprietary assets to make a green badge.

Recommended required PR checks:

1. `cargo fmt --check`, Clippy for the supported host targets, and the shared
   Cargo test gate. Measure the baseline before making existing warning debt
   fatal; fix relevant warnings without adding a repo-wide cleanup project.
2. `assets:sfx:check`, `assets:pack:first-party:test`, first-party pack build,
   and strict runtime provenance validation.
3. Existing thin-adapter/scene/XR frame-driver guards as applicable, web Wasm
   compilation and generated-glue TypeScript validation, and affected lab
   typechecks/tests. The current `native:web:typecheck` also builds glue;
   provision packs before invoking it or split its reusable stages.
4. A distributable-file inventory check and secret scanning. Run a one-time
   full-history scan before opening visibility, then scan new changes in CI.
   Redact findings; do not publish scanner output containing secrets.

Start with one Linux correctness job plus a web job. Use path-aware expensive
checks and one stable aggregate required check so docs-only changes do not
leave required checks pending. Aim for a warm PR run around ten minutes, then
adjust from actual timings rather than making that an unsupported promise.

## 3. Build downloadable packages on CI

All client families belong in the first nightly implementation, with explicit
build-versus-runtime evidence. Proposed hosted runner labels below are
available in GitHub's current
[runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners);
pin OS labels rather than `latest` and recheck at implementation time.

| Job | Proposed runner | Initial artifact and validation |
| --- | --- | --- |
| Linux desktop + server | `ubuntu-24.04` x64 | `.tar.gz` packages; flat and XR-enabled desktop entry points, CLI/server smoke, dependency inventory; software-render smoke where supported. |
| Windows desktop + server | `windows-2025` x64 | ZIP packages; native MSVC build, flat/XR compilation and startup checks. Use PowerShell/Git Bash, never WSL Cargo. |
| macOS desktop + server | `macos-15` ARM64 | Desktop `.app` archive and server archive; bundle resources, launch smoke, ad-hoc signing initially. Desktop OpenXR remains explicitly experimental and requires a separately installed runtime. |
| Flat Android + Quest | `ubuntu-24.04` x64, separate matrix jobs | ARM64 APKs through the existing scripts, APK structure/native-library/signature/version checks. Add an x86_64 flat emulator smoke after proving the hosted GPU/emulator route. |
| Web game + hub | `ubuntu-24.04` x64 | Runnable static bundle with Wasm, JS, Workers, first-party packs, and hosting headers; complete hub checked separately if split from the game bundle. |

Windows ARM64, Linux ARM64 release downloads, Intel macOS, installers,
AppImage, and container images are optional follow-ups. The ARM64 VM probe is
useful portability/setup evidence, not validation of an x64 Linux artifact.

Build the canonical asset stage once per workflow revision and share its exact
bytes between package jobs. Allow existing Android scripts to verify/reuse an
explicit staged input; retain local rebuild behavior. Never reuse an asset
artifact from an unrelated revision. Repeat a cold asset build to compare
hashes before trusting that reuse.

Desktop packages need executable-relative/app-resource asset discovery, a
default Original selection, third-party notices, build metadata, and a short
run guide. Keep save/config paths outside versioned installation directories.
Test with no source checkout, no `MCLONE_*` environment overrides, a working
directory elsewhere, and a path containing spaces. Inspect native dependencies
instead of assuming a successful link produces a portable executable.

A single XR-enabled desktop executable may serve flat and OpenXR launches if
flat startup does not require an installed XR runtime; prove that condition.
Otherwise ship an explicitly named XR executable alongside the flat one.

Keep CI resource use bounded: disable incremental output, cache dependencies
with OS/architecture/toolchain/lockfile keys, avoid caching the entire workspace
target tree, and control parallel build jobs. Hosted runner disk and macOS
memory limits warrant measurement before adding more architectures.

## 4. Publish simple nightly prereleases

Proposed workflow split: `checks.yml`, reusable `build-packages.yml`, and
`nightly.yml`. Build changed main revisions for Actions downloads; run nightly
publication on an off-hour schedule such as `17 3 * * *` plus manual dispatch.
Reuse successful exact-revision artifacts or rebuild through the same workflow.
Skip an unchanged successfully released commit unless manually forced.

- Use immutable names such as `nightly-YYYY-MM-DD-<shortsha>` and mark them
  prereleases, not stable releases. README links to a nightly listing rather
  than relying on GitHub's stable `/releases/latest` behavior.
- Create a draft, upload the complete expected matrix, verify contents and
  checksums, then publish. Failed/incomplete runs preserve the last successful
  nightly. Do not present an old APK alongside new desktop files as one build.
- Include full commit SHA, tool versions, target/features, pack hashes,
  SHA-256 checksums, licenses/notices, and a clear “experimental; worlds and
  compatibility may change” note. No promise of save compatibility is added.
- Keep normal CI artifacts for 7 days and successful nightlies for 14 days,
  always retaining the most recent successful nightly and manually pinned
  versions. Make cleanup narrowly match generated nightly releases.
- Generate one dedicated nightly Android signing identity and retain it in
  protected CI secrets with an owner-held backup. Use it for both app IDs;
  keep debug builds separate. Derive increasing version codes across runs,
  including future workflow migrations. Test installing a second nightly over
  the first without deleting app data.
- macOS notarization and Windows signing can follow the initial experimental
  downloads; document the actual platform launch friction. Android's stable
  signing identity is needed from the first upgradable nightly.
- Start `GITHUB_TOKEN` read-only, pin third-party Actions to reviewed commits,
  expose write credentials only to trusted publishing jobs, and do not execute
  fork PR code on personal VM/GPU/device runners. Keep private Machine Control
  inventory out of Actions and the repository.

GitHub schedules can be delayed and public-repository schedules can be
disabled after inactivity; manual dispatch is the recovery path, not a promise
of an exact release time. See
[workflow event behavior](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
and [self-hosted runner precautions](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository).

The web demo can continue using the existing deploy owner initially. Once CI
deployment is added, promote the exact tested bundle and choose one owner;
avoid racing the local post-push hook against Actions.

## 5. Tighten the README

Target roughly 60–90 lines. Suggested opening:

> Mclone is a work-in-progress voxel game and Rust engine experiment. It is
> mostly a playground for terrain generation, distant-terrain rendering, and
> cross-platform builds; there is not much gameplay yet.
>
> **[Try it in your browser](https://mclone.kzahel.com/play/)**

Follow with one real in-game screenshot, an experimental-status paragraph,
nightly downloads, a compact platform table, a short source-build entry point,
and three or four links for development, architecture, terrain experiments,
and contributing/license information. Keep project maps and internal
implementation-policy text in the existing docs. Avoid a rename and new
marketing claims. Say which terrain experiments are selectable today and
link to the deeper terrain/LOD notes rather than cataloguing every tactical.

Capture the README image from the corrected first-party public game with a
recorded revision, seed/profile, camera, and asset hashes. Capture and inspect
under `/tmp` first. Publish the selected image to a durable release/static
media location and embed that URL; do not commit debug screenshots or temporary
paths. A normal attractive terrain view is sufficient; do not build another
showcase or imply completed survival gameplay merely to obtain the image.

Add a short contribution guide asking for platform, revision, reproduction
steps, and logs for bugs. The maintainer has explicitly deferred project
licensing: do not add a project license. Retain third-party notices and report
project licensing as TBD.

## 6. Validate the public experience and open visibility

Use Machine Control to acquire an isolated workspace, run doctor, carry its
claim on each operation, and release it in cleanup. Perform two distinct tests:

1. **Contributor:** follow only the public guide from a fresh source download
   or clone, build first-party assets and a client, and record dependencies,
   cold times, peak disk use, exit statuses, and any skipped tests.
2. **Player:** transfer only a CI-produced package into a clean guest, launch
   without the source/toolchain, create/play/save/reopen a world, inspect its
   screenshot and runtime provenance, then install the next nightly. Use an
   appropriate GPU host for render acceptance if the VM exposes software-only
   graphics. A CLI pass does not count as visual or headset acceptance.

Do a bounded physical Quest launch/controller/render smoke through the
authoritative provider before describing the first APK as tested on Quest.
Later nightlies can clearly distinguish “CI built” from the last device-tested
revision. Run desktop/browser GPU acceptance sequentially on a shared host.

Open visibility after the local asset-boundary and history review, **before
validating GitHub Actions**, as explicitly authorized. Inspect remote refs and
existing releases, preserve history, then push/validate workflows publicly.
CI and package acceptance remain completion gates after visibility changes.

## Review decisions and completion

Already decided: preserve history; retain the reimplemented terrain code;
present a WIP side project; prefer the web link for trying it; cover desktop
and Quest in CI; defer commercial-release machinery.

Project licensing remains TBD. Use the proposed architecture matrix, nightly
retention, and unsigned/notarization posture. These do not block implementation
of the clean asset boundary and repeatable builds. Concrete evidence of an
unreleasable historical file requires a focused decision consistent with the
history requirement; it does not silently authorize a rewrite.

Execution order: **public asset boundary → local bootstrap/checks → public
visibility → packages and CI validation → nightly publishing → final
README/screenshot → fresh-package acceptance**. Each slice records evidence;
in-progress work is not a claim that the release gates already pass.

## Execution record

### Slice 1 — reference-free startup and packaging foundation

- Pinned Rust 1.98.1, Node 22.23.2; retained pnpm 9.15.1 and lockfiles.
- Removed reference build/copy from the web bundler and local deploy hook.
  Browser bootstrap/render workers load Original packs, with saved-reference
  recovery through shared preference reconciliation.
- Native default startup uses first-party packs, with executable-relative and
  macOS app-resource discovery. Local reference content is optional in the
  catalog. Explicit legacy asset modes remain research overrides.
- Added complete output inventory, approved-stage pack hash verification,
  media checks, and reference archive rejection. Two contamination tests pass.
- Shared runtime: 303 tests passed. Native client with XR compiled and built;
  browser Wasm built and web glue typechecked. Full hosted hub assembled:
  619 shipped files before the generated inventory, three approved packs.
- Browser app-loop smoke passed for Mclone Overworld with 160 first-party and
  zero reference/unknown resolutions. Captured and inspected browser/native
  pixels under `/tmp`. The default generic browser interaction probe timed
  out waiting for its aim target; the profile-specific public startup/movement
  lane passed and does not claim that unrelated target-picking acceptance.
- A macOS app package launched from `/tmp` and rendered successfully. Its
  executable/resource layout works; clean-guest package acceptance is pending.
- Full-history Gitleaks 8.30.1 scan passed across 3,278 scanned commits after
  one exact fingerprint exception: a historical Quest virtual-window UUID,
  not a service credential. The downloaded scanner checksum was verified.
- Reviewed the ten old tracked screenshot attachments; removed them from the
  current index while preserving existing history and local ignored copies.
  The retained history contains these old debug/menu/terrain captures; this
  review does not claim to certify every historical image's rights.
- Desktop archive generation includes dependency notices. Android distribution
  signing/version inputs and stage reuse are prepared; hosted package builds
  and signing/install validation remain pending.

### Slice 2 — public workflows and nightly packaging

- Repository visibility changed to public after Slice 1, before any hosted
  Actions validation. Anonymous repository HTTP access returned 200.
- Deployed Slice 1 revision `8e654fc2`; the public reference ZIP now returns
  404. The deployed browser app smoke passed and its pixels were inspected.
- Added pinned-action workflows for shared correctness, architecture guards,
  full-history redacted secret scanning, one reusable first-party asset stage,
  and Linux x64/ARM64, Windows x64, macOS ARM64, web, Android, and Quest
  packages. Linux ARM64 also provides a native package for the fresh VM.
- Added dated nightly prereleases, checksums, seven-day CI artifact retention,
  and fourteen-day nightly retention after successful replacement publication.
- Provisioned the stable Android nightly signer as repository secrets; a
  private local backup exists outside the repository. Release version codes
  follow the monotonically increasing workflow run number.
- Workflow syntax and architecture guards pass locally. Hosted validation is
  next; these workflows and packages are not yet claimed CI-green.

### Slice 3 — public introduction and contributor setup

- Replaced the long front page with a 54-line WIP introduction, web play link,
  development downloads, a real terrain/LOD screenshot, platform entry points,
  and focused development links. Added fresh setup and contribution guides.
- Published the inspected screenshot under the durable `project-media`
  release. Its downloaded SHA-256 matches the original capture:
  `5e245bec166f7f50b1d49402f0240ebc2fcbf31e75db1b74a6e940375285d406`.
  The release notes record seed, profile, camera, resolution, and source.
- Updated platform/web docs and the compatibility ledger: public development
  builds do not establish a generator or save-format freeze. Licensing stays
  TBD; no project license was added.
- The first hosted run exposed missing ripgrep on Ubuntu. Commit `df1372a5`
  provisions it; its rerun passed asset construction and strict provenance and
  has advanced to shared correctness tests.
- A second disposable Machine Control workspace cloned `df1372a5` anonymously
  from public GitHub, built all three packs without reference hydration, and
  is continuing its cold desktop/web installation probe.
- Quest doctor currently reports no attached authorized headset. Physical
  acceptance remains pending; the maintainer has been asked to connect it or
  explicitly retain CI-build-only status.

### CI and fresh-install corrections in progress

- Hosted shared correctness, architecture guards, asset construction/provenance,
  and secret scanning passed in run `34678473657`. Platform jobs started.
- Android provisioning exposed absent `sdkmanager` on the runner; added a
  pinned SDK setup action. Refreshed action pins to current Node 24 releases.
- The anonymous VM desktop build passed. Full web assembly reached Asset Lab
  thumbnail generation, then exposed a different locked Playwright version
  from the root package. The bundler now installs each lab's matching browser.
- Added dependency-supplied notices to web/Android packages, NDK notices beside
  APKs, a web build revision receipt, and checksum verification before nightly
  publication. No project license is assigned by these notices.

### First public CI package evidence

Run [`34678473657`](https://github.com/kzahel/mclone/actions/runs/34678473657)
completed: all shared checks and all four desktop package jobs passed. Android
SDK provisioning and web thumbnail browser provisioning failed at the already
identified setup boundaries; corrected in `86fd84ae` for the next run.

The downloaded macOS CI ZIP passed SHA-256 and strict bundle signature
verification, then rendered from `/tmp` with zero reference/unknown provenance.
Its screenshot was inspected. Linux CI asset bytes match independently built
macOS asset bytes for all three packs. Local flat Android and Quest release
builds passed through the project scripts; full APK inventory checks passed
(14 and 18 files respectively). These local APKs use the development signer;
stable nightly signing and physical installation still require their own
receipts.

### Fresh contributor and player acceptance

The anonymous Ubuntu ARM64 contributor probe completed after the lab browser
fix at `86fd84ae`: strict asset preparation, desktop build, full web hub bundle,
and web typechecking all passed without a reference tree. The working tree and
lockfiles remained clean. Final sizes: 3.6 GiB Rust outputs, 250 MiB generated
assets, and 78 MiB hosted bundle. The disposable source workspace was discarded
and its claim released.

A separate clean Ubuntu ARM64 workspace received only the CI-produced Linux
ARM64 package and checksum from run `34678473657`. A new player account used
`/usr/bin:/bin` with no Cargo available and no source checkout. Package checksum
verification, first-party runtime provenance, a scripted ordinary input action,
SQLite world creation, and world reopen all passed. SQLite integrity returned
`ok`. Both captures were inspected; the reopened world visibly renders terrain,
sky, and an actor. The adapter was Mesa llvmpipe (software Vulkan), so this is
package/install/persistence evidence, not hardware-GPU performance acceptance.
The workspace is retained temporarily for an upgrade check with the next
CI-produced package.

The hosted shared suite passed 2,092 tests with one explicitly ignored legacy
parity gauntlet and no failures. The original macOS/browser GPU captures remain
the hardware-render evidence; Quest hardware acceptance is still pending.

### First complete nightly and upgrade verification

Run [`34679517652`](https://github.com/kzahel/mclone/actions/runs/34679517652)
passed every shared check, all desktop/web/Android/Quest packages, and nightly
publication. It published
[`nightly-20260912-3`](https://github.com/kzahel/mclone/releases/tag/nightly-20260912-3)
from `383f5bf8`.

Both downloaded APK checksums and signatures verified against the stable
nightly signing certificate. Both retain their existing application IDs and
use `versionCode=3`, `versionName=nightly-3`. Inspection found extra loader-only
ABIs in the Quest AAR, so its packaging is now explicitly ARM64-only. The
loader AAR's license is copied beside downloads because Gradle does not retain
that notice in the APK. Local rebuilding and inventory inspection passed with
only `arm64-v8a` in the Quest package.

The clean player VM upgraded from the `df1372a5` CI package to the `383f5bf8`
package, reopened the same saved world, rendered successfully, and retained a
healthy nonempty SQLite chunk store. Its upgraded capture was inspected.
Final package instructions now use the actual `--desktop-xr` command, and the
nightly publisher includes the strict first-party provenance receipt alongside
the downloads. A final hosted publication verifies these packaging corrections.
