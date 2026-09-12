# Release Distribution And Updates

Topic: `release-distribution-and-updates`

Status: **accepted managed-distribution architecture recorded 2026-07-22;
not yet implemented and no launcher tactical is reserved.** Managed direct
desktop distribution should use a separate first-party launcher, store
installations should retain their
store's update ownership, and the game engine should remain unaware of
installation mutation. The first direct updater should install complete signed
artifacts transactionally. Asset-level or binary delta work remains
measurement-gated.

Last reconciled: **2026-09-12** (public development releases implemented;
managed-distribution architecture retained).

## Public development releases (2026-09-12)

The existing repository is public with its history preserved. Experimental
desktop, web, Android, and Quest builds are available from CI and dated GitHub
prereleases. This is a WIP side-project release with limited gameplay. Tactical
[`335`](../tactical/335-public-development-release.md) records implementation,
fresh contributor/player VM acceptance, and exact build/device evidence. The
shorter README prominently links World Explorer as a quick browser demo,
the game, the project hub, and all four labs. Inspected World Explorer and
Asset Lab captures are hosted in the durable `project-media` release; the
original in-game screenshot remains linked. The GitHub description names
desktop, web, Android, and Quest/OpenXR, and its website field links the hub.
Project licensing remains TBD; no project license was added.
Visibility changed before hosted CI validation, as explicitly requested.

The development workflow builds one approved first-party stage and shares its
exact bytes across all package jobs. Desktop ZIPs cover Linux x64/ARM64,
Windows x64, and macOS ARM64; they include the client with optional OpenXR and
the dedicated server. Web downloads contain the full static hub. Flat Android
and Quest downloads are ARM64 APKs signed by a stable nightly identity; its
backup stays outside the repository and CI consumes protected secrets.

Scheduled publication runs daily at 03:17 UTC, with manual dispatch available.
Only a complete matching-revision matrix can publish: upload to a draft,
verify remote sizes/digests, then expose the prerelease. Unchanged scheduled
revisions retain existing downloads. Artifacts expire after seven days;
successful publication prunes generated nightlies older than fourteen days,
except releases marked `<!-- keep-nightly -->`. A failed run keeps the previous
successful nightly. Licensing TBD, unsigned/notarized desktop posture, and
CI-built versus device-tested status are explicit in the public guide.

The first complete refined release is
[`nightly-20260912-8`](https://github.com/kzahel/mclone/releases/tag/nightly-20260912-8)
from `baebbb46`. CI recovery through `publish-existing.yml` accepts only a
completed main development run with every correctness/package job green and
rechecks its exact artifacts. It can resume a complete matching draft after
a publication-only correction; partial or corrupt uploads remain hidden.
Fresh VM installation/save-upgrade evidence and final web deployment passed;
physical Quest acceptance remains pending rather than implied by its APK.

A launcher, automatic updates, storefronts, and final branding are not part of
this bounded milestone; the architecture below remains longer-term guidance.

## Scope

This topic owns the release-distribution and update direction for public Mclone
builds:

- the technical boundary needed to support the direct and store offers defined
  by [`distribution-go-to-market.md`](distribution-go-to-market.md);
- direct desktop launcher and update ownership;
- Steam, Quest Store, Android sideload, web, and dedicated-server update
  boundaries;
- signed release metadata, artifact layout, channels, promotion, repair, and
  rollback;
- executable versus asset-pack update units; and
- the validation and operational evidence required before enabling automatic
  updates publicly.

It does **not** own:

- gameplay, scene, renderer, session, or persistence behavior;
- first-party asset authorship or the proof that public artifacts contain no
  Minecraft reference content — that remains in
  [`asset-pack-profiles.md`](asset-pack-profiles.md);
- the current platform build and smoke matrix — that remains in
  [`../platforms.md`](../platforms.md);
- web deployment mechanics — those remain in
  [`../native-web.md`](../native-web.md); or
- final store pricing, commercial approval, tax, entitlement, or supporter-SKU
  decisions. This topic records the product intent and the technical shape that
  can support it, not a claim that every store has approved it.

## Product Intent

Mclone should retain a supported first-party distribution path for players who
do not want Steam or another store. Whether that path is a complete free game,
a free trial followed by a direct purchase, or a free base with supporter
products is a commercial decision owned by
[`distribution-go-to-market.md`](distribution-go-to-market.md). The current
leading candidate is a paid Steam edition and proper Steam demo, plus a paid
direct edition with a first-party trial.

The intended game remains the same across flavors: payment must not create a
gameplay fork, and side loading must remain a real supported path rather than a
deliberately degraded build. Store-specific achievements, presence, cloud
saves, entitlements, package identity, and update ownership may differ.

Steam's published key rules require Steam customers to receive an offer
comparable to Steam-key purchasers, but do not directly answer every case
involving a separately distributed non-Steam build. Obtain written guidance
before announcing the final Steam SKU model:
[`Steam Key Rules and Guidelines`](https://partner.steamgames.com/doc/features/keys).

## Top-Level Decision

Use a separate launcher for **first-party desktop distribution**, but do not
turn the launcher into another engine host.

- A small Tauri application is the preferred initial launcher because it gives
  the product a cross-platform desktop UI and a maintained self-update path.
- Tauri's updater updates the launcher itself.
- A narrow first-party payload installer owned by the launcher installs,
  repairs, selects, and launches game versions.
- Steam and platform stores remain the install owners of files they deliver.
  A store build must not race or modify those files through the first-party
  updater.
- Direct Android and Quest executable updates go through Android package
  installation semantics rather than replacing native libraries from inside a
  running game.
- Web deployment remains the web client's update mechanism.

The resulting ownership is:

```text
release pipeline
  |
  +-- Steam artifacts ----------------> SteamPipe owns install/update
  +-- Quest Store APK ----------------> Meta owns install/update
  +-- web build ----------------------> deployment owns activation/cache
  +-- direct APK ---------------------> Android installer + user owns update
  +-- direct desktop artifacts
          |
          v
     immutable object storage / CDN
          |
          v
     first-party launcher
       +-- Tauri updater owns the launcher
       +-- payload installer owns versioned game files
       +-- repair/channel/launch UI
       +-- game startup health handshake
```

Updating is an installation/platform concern. It does not belong in
`mclone-scene`, `mclone-app-runtime`, or another shared gameplay/runtime crate.
The game may expose neutral build facts and a startup-health acknowledgement;
it must not know how SteamPipe, Tauri, a CDN, or Android's installer replaces
files.

## Platform Ownership Matrix

| Distribution | Install and update owner | Launcher posture |
|---|---|---|
| Direct Windows, macOS, Linux | First-party launcher | Required for the managed direct installation; Tauri self-updates separately from the game payload. |
| Steam desktop flat/OpenXR | SteamPipe | Optional only when it provides independent value such as settings, accounts, profiles, or news. First-party file mutation is disabled. |
| Quest Store | Meta platform | No separate in-headset launcher required for updates. Store-managed APK files are never self-replaced. |
| Direct Quest / flat Android APK | Android package installer and user | Initially notify and provide a signed APK/manual installation path. A later in-app request must handle user confirmation and unknown-source approval. |
| Web/WASM | Web deployment and browser cache lifecycle | No native launcher. Use immutable versioned assets and deliberate service-worker activation if one is introduced. |
| Direct dedicated server | Server operator, package/container manager, or later non-UI updater | Reuse signed release metadata where helpful, but do not require a graphical launcher. Never update a running authoritative process in place. |

Android's `PackageInstaller` can install or upgrade APKs, but callers must be
prepared for `STATUS_PENDING_USER_ACTION`; update flows cannot assume silent
installation:
[`PackageInstaller`](https://developer.android.com/reference/android/content/pm/PackageInstaller).

## Launcher Boundary

The launcher should live in the Mclone repository so release schema and build
changes remain reviewable with the product, but it should be a separate package
or workspace with its own dependency surface and release cadence. It must not
depend on engine crates merely to share convenient types. A tiny standalone
release-manifest library is acceptable if it has no engine or renderer
dependencies.

The launcher owns:

- install root selection and free-space checks;
- stable, beta, development, or other explicitly published channels;
- release metadata refresh and launcher-minimum-version enforcement;
- resumable artifact downloads;
- signature, length, and digest verification;
- staged installation, atomic activation, repair, and old-version collection;
- launch arguments and process lifetime observation;
- a bounded startup-health handshake; and
- user-visible recovery when an update cannot be installed or launched.

The game owns only neutral facts at this boundary:

- product and release version;
- build target and feature/flavor facts needed for diagnostics;
- save-schema and network-protocol compatibility versions;
- an acknowledgement that startup reached the defined healthy milestone; and
- an optional explicit request to open the launcher for update or repair.

Do not share mutable launcher state through engine configuration directories.
Worlds, preferences, logs, and crash reports must remain outside replaceable
version directories.

## Launcher Self-Update Versus Game Update

These are deliberately separate systems.

Tauri's updater is appropriate for the stable launcher bootstrap. It supports
static JSON or dynamic update endpoints, requires update signatures, and emits
whole platform updater artifacts such as a macOS app archive, Windows
installer, or Linux AppImage artifact. It is not the game asset-diff system:
[`Tauri Updater`](https://v2.tauri.app/plugin/updater/).

The game payload updater consumes the Mclone release manifest and installs
Mclone-specific artifacts. A manifest can require a minimum launcher version;
the launcher must update itself before parsing or installing a release whose
contract it cannot safely execute.

Do not bundle every new game build into a new launcher release. An initial
installer may contain a bootstrap game build or offer a separate complete
offline bundle, but ordinary game releases should leave the stable launcher
binary unchanged.

If a rich launcher stops being valuable and direct desktop distribution only
needs a conventional application updater, evaluate an established updater
before expanding custom code. Velopack currently provides cross-platform,
language-neutral installers and delta packages:
[`Velopack`](https://github.com/velopack/velopack).

## Release Repository And Manifest

The initial first-party service should be mostly static:

- immutable versioned or content-addressed artifacts in object storage;
- a CDN in front of those objects;
- small signed channel metadata documents;
- a release publisher in CI; and
- a promotion step that changes channel metadata only after validation.

No database-backed update API is required for the first release. A dynamic
service may be added later for staged rollout, entitlement-independent
analytics, regional mirrors, or operational control, but signed metadata must
remain authoritative if that service or the CDN is compromised.

The first manifest schema should carry at least:

```text
schema version
product release id and semantic/display version
channel and publication time
target OS, architecture, and distribution flavor
minimum launcher version
metadata version and expiration
artifact paths, byte lengths, cryptographic digests, and install sizes
artifact roles: executable, authored assets, fallback assets, audio, etc.
save-schema read/write range
network-protocol compatibility range
required versus optional artifacts
rollout policy, if staged rollout is enabled
signature/key identity
```

Release objects are immutable. Promotion changes the signed channel view; it
does not overwrite an artifact behind an existing URL. A release may be
withdrawn by publishing new metadata, but clients retain enough trusted state
to reject an attacker replaying older metadata as current.

Use an established signed-update threat model instead of treating an HTTPS
connection or a digest from an unsigned manifest as authentication. The Update
Framework defines trusted root, target, snapshot, and timestamp metadata for
key rotation and rollback/freeze protection while leaving installation to the
application:
[`The Update Framework overview`](https://theupdateframework.io/docs/overview/).

The first implementation should either use a maintained TUF implementation or
document how its narrower metadata model supplies the required properties:

- a pinned and rotatable offline root of trust;
- separate online release/promotion credentials where practical;
- signed artifact identities, sizes, and digests;
- monotonically versioned, expiring metadata;
- rollback and mix-and-match rejection;
- safe key rotation and revocation; and
- a recovery procedure for compromised online publishing credentials.

## Transactional Install Contract

The managed direct-desktop layout should retain complete version slots:

```text
Mclone/
  launcher/                # conceptual; platform packaging may place it elsewhere
  versions/
    <release-a>/
    <release-b>/
  content/                 # optional immutable/shared objects
  state/
    current.json
    last-good.json

client-data/               # separate OS-appropriate application-data root
  worlds/
  preferences/
  logs/
```

An update transaction is:

1. Fetch and authenticate current metadata.
2. Resolve the exact target, architecture, channel, and distribution flavor.
3. Check launcher compatibility, disk space, and currently running processes.
4. Download missing artifacts into a staging location.
5. Verify every artifact before making it executable or visible as current.
6. Materialize a complete new version slot alongside the active version.
7. Atomically replace the small current-version pointer.
8. Launch with a one-time health token and wait for the defined
   acknowledgement.
9. Mark the slot last-known-good after acknowledgement.
10. Retain at least the previous good slot and collect older unreferenced data
    only after the transaction is complete.

Never reconstruct or patch the active slot in place. Cancellation, power loss,
network loss, a full disk, a corrupt CDN response, or launcher termination must
leave either the old complete slot active or the new complete slot active.

Repair uses the same authenticated manifest and artifact verification path as
installation. It must not invent a second source of expected file hashes.

## Rollback, Saves, And Protocol Compatibility

Keeping an old executable is not proof that rollback is safe. A new build may
have migrated a world, preferences, an asset-pack preference schema, or a
dedicated-server database beyond the old build's readable range.

- The release manifest must state save-schema read and write compatibility.
- A migration must follow the persistence subsystem's backup and transaction
  policy rather than relying on the launcher to copy live databases blindly.
- Automatic rollback is allowed only before incompatible durable state is
  written, or when the old release explicitly declares that it can read the
  resulting schema.
- Otherwise the launcher should retain the failed release for diagnostics and
  offer recovery or a forward fix instead of silently launching the old game.
- Client and dedicated-server artifacts must declare protocol ranges so a
  launcher update cannot silently turn a known compatible deployment into an
  unexplained connection failure.

The health acknowledgement should prove that the process reached a bounded
product milestone such as initialized title UI or a target-specific equivalent;
mere process creation is insufficient. It must remain diagnostics/launch glue,
not a new platform-specific gameplay readiness definition.

## Asset And Delta Strategy

Mclone already builds deterministic standalone authored and generated fallback
`.pbp` artifacts with stable logical identities and content fingerprints.
Those are the first update units for public assets. Minecraft reference packs
remain local development inputs and must never enter the public repository.

The accepted optimization order is:

1. **Whole changed artifacts.** Download a complete executable bundle or
   changed `.pbp`. Establish correctness, recovery, and signing first.
2. **Churn-aligned sharding.** If measured updates are too large, separate
   independently changing terrain, actor, UI, localization, or audio payloads.
3. **Content-addressed chunks.** Add fixed or content-defined chunk reuse only
   when release-to-release measurements justify the extra client, publisher,
   cache, and repair complexity.

Do not begin with arbitrary binary deltas over compressed archives. Stable
entry order, deterministic metadata, bounded pack sizes, and per-entry
compression improve both whole-file reproducibility and downstream chunk
reuse. SteamPipe already divides content into approximately 1 MiB chunks and
reuses matching chunks; Valve warns that shuffled asset ordering,
cross-boundary compression, and very large packs can turn small edits into
large downloads and expensive local rewrites:
[`SteamPipe uploading and pack-file guidance`](https://partner.steamgames.com/doc/sdk/uploading).

Before implementing direct-channel chunks, retain empirical records for:

- complete install download and installed size by target;
- changed bytes by artifact across representative releases;
- SteamPipe-reported update size;
- direct whole-artifact update size and wall time;
- temporary disk-space amplification during activation; and
- expected savings versus chunk-store metadata, cache, and reconstruction
  costs.

## Platform Signing And Package Identity

Update-feed signatures authenticate the release decision. Operating-system
code signing authenticates executable code to the platform. Both are required;
neither substitutes for the other.

- Direct macOS launcher and game executables must use the appropriate
  Developer ID signing, hardened runtime, notarization, and stapling workflow:
  [`Apple notarization guidance`](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
- Direct Windows launcher, installer, game executable, and relevant helper
  binaries must use a trusted code-signing workflow. Unknown unsigned binaries
  may be blocked by Windows application-control policy:
  [`Smart App Control`](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview).
- Android APK updates require compatible package identity and signing. Android
  users can update an installed app only when the update is signed with the
  expected signing identity:
  [`Android app signing`](https://developer.android.com/studio/publish/app-signing).

The relationship between direct and Quest Store package IDs and signing keys is
an early release decision, not packaging trivia. If both channels cannot
legitimately share the required signing identity and update ownership, use
distinct package IDs and design explicit world export/import. Do not promise an
in-place direct-to-store transition until it has been tested with production
signing and entitlement behavior.

## Store And Direct Build Parity

All public flavors should be produced from one versioned release intent and the
same gameplay/content source. Byte identity is not required when a store SDK,
entitlement adapter, platform manifest, or signing process genuinely differs.
Behavioral parity is required.

A distribution flavor may own:

- store SDK or entitlement initialization;
- achievements, presence, cloud-save, or overlay adapters;
- platform manifests and package identity;
- code signing and store receipt facts; and
- selection of the external install owner.

It may not own divergent gameplay, renderer behavior, world formats, asset-pack
selection semantics, or a better/worse update compatibility policy. Store
flavor detection must never be inferred from an arbitrary path alone; package
or build metadata should establish the install owner explicitly.

## Future Managed-Distribution Delivery Sequence

The simpler public development release is implemented in Tactical 335 above.
For later managed distribution, use bounded slices rather than attempting
launcher, CDN, signing, delta updates, and every store simultaneously.

1. **Release contract and artifact audit**
   - assign canonical release/flavor/build facts;
   - produce public first-party-only artifacts;
   - define save and protocol compatibility versions; and
   - preserve signed/notarized store-ready outputs in CI.
2. **Static direct repository**
   - publish immutable artifacts and signed stable-channel metadata;
   - prove fresh manual installation and authenticated download; and
   - retain a complete offline installer path.
3. **Stable desktop launcher bootstrap**
   - create the isolated Tauri launcher;
   - enable Tauri self-update; and
   - establish the narrow launch/health contract without custom deltas.
4. **Transactional whole-artifact updates**
   - implement staging, atomic activation, repair, last-good retention, and
     failure recovery; and
   - test every supported desktop OS and architecture from old public builds.
5. **Store adapters and direct Android UX**
   - prove first-party mutation is disabled in Steam and Quest Store flavors;
   - settle Android package identity/signing; and
   - add honest notification/manual or PackageInstaller-based sideload update
     UX.
6. **Channels and rollout controls**
   - add beta/development channels, promotion receipts, withdrawal, and staged
     rollout only after the stable transaction is reliable.
7. **Measured optimization**
   - shard high-churn artifacts, then evaluate content-addressed chunks or an
     established delta framework from recorded release measurements.

## Validation And Release Gates

Before public automatic updates, CI and release qualification must cover:

- manifest parsing, unknown-schema rejection, target/flavor selection, and
  minimum-launcher enforcement;
- valid signatures plus corrupt, truncated, wrong-length, expired, replayed,
  downgraded, and mixed-release metadata/artifacts;
- fresh install and at least previous-two-supported-release updates on every
  direct desktop target;
- interruption during metadata fetch, download, materialization, activation,
  first launch, and cleanup;
- insufficient disk space, read-only paths, locked/in-use executable files,
  antivirus delay, and permission failure;
- launcher self-update followed by a game update requiring the new launcher;
- startup crash/no-acknowledgement, repair, last-good retention, and
  save-schema-safe recovery behavior;
- OS signature verification, macOS notarization/stapling, and first-launch
  behavior on clean machines;
- a release-artifact provenance audit proving no Minecraft reference or
  unknown asset source is shipped;
- Steam/Quest Store assertions that first-party file mutation is disabled;
- real direct-APK update behavior with production package identity and signing;
  and
- explicit human inspection of launcher progress, failure, recovery, and
  support-copy UX.

Update logs should be structured and privacy-conscious: release IDs, stages,
durations, byte counts, error classes, and recovery actions are useful; world
paths, usernames, access tokens, signing material, and arbitrary command lines
must not be uploaded or exposed.

## Open Decisions

- What is the final Steam and Quest supporter/store SKU model, and what written
  store guidance applies to a free direct build?
- Should the direct desktop installer initially bundle one playable build or
  remain a small online bootstrap, and is a separate complete offline package
  required from day one?
- Which maintained TUF implementation and signing/key-management service best
  fit the Rust/Tauri release toolchain?
- Where should the launcher workspace and standalone release-schema crate live,
  and which dependencies can be kept out of the bootstrap trust surface?
- What exact game milestone constitutes a successful first-launch health
  acknowledgement for flat and desktop OpenXR runs?
- What save-schema and protocol-version records need to be established before
  the first public build?
- Can direct and Quest Store packages share a supported package/signing
  identity, or must they be separate applications with explicit data transfer?
- Which object store/CDN, retention window, bandwidth budget, and geographic
  requirements apply to free direct distribution?
- At what measured update size or frequency does pack sharding or chunk-level
  reuse justify its additional operational complexity?

## Related Documents

- [`distribution-go-to-market.md`](distribution-go-to-market.md) — commercial
  channels, pricing models, positioning, competitive landscape, and launch
  sequence.
- [`../platforms.md`](../platforms.md) — platform posture, packaging owners, and
  validation matrix.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  durable shared-engine versus platform-adapter ownership.
- [`asset-pack-profiles.md`](asset-pack-profiles.md) — distributable
  first-party asset artifacts, content fingerprints, and provenance audit.
- [`../native-web.md`](../native-web.md) — browser build, smoke, and deploy
  mechanics.
- [`platform-parity.md`](platform-parity.md) — behavioral parity across client
  targets and adapters.
