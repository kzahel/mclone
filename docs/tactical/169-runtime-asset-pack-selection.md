# 169: Runtime Asset Pack Selection

Status: active implementation parent 2026-07-11; Slices 0-5 landed. Platform
discovery/adoption now uses the shared catalog and epoch contract on every
supported client lane. Resume this tactical at Slice 6 persistence, audit, and
closeout.

Topic: [`asset-pack-profiles`](../topics/asset-pack-profiles.md)

Workstream: shared native Rust and native web/WASM client presentation, plus
first-party asset build tooling. Desktop validation comes first, but the
selection contract and UI are shared from the first slice.

## Goal

Add a shared **Asset Packs** menu that lets the user enable or disable available
asset packs, then apply the staged selection without restarting or reconnecting
the game. The initial installed choices are the Mclone authored pack and the
local Minecraft 1.17.1 reference pack. A generated first-party fallback pack is
always active and makes every optional-pack combination drawable.

The implementation must support an auditable selection in which no proprietary
asset source is eligible or resolved. Missing textures and simple missing
visuals are generated during pack construction, carry identifiable resource
codes, and use ordinary runtime atlas/mesh paths.

## User Experience Contract

Add `Options -> Asset Packs` from both title and pause contexts. Use the shared
native UI on flat, XR, Android, and emulated/offscreen profiles, and project the
same state/action model through native web.

Initial screen shape:

```text
Asset Packs

[x] Mclone Original Assets       First-party · authored
[ ] Minecraft 1.17.1 Reference  Local only · proprietary
[x] Generated Missing Assets     First-party · always active

Effective: Mclone Original
Authored 15 / Required N · Generated N-15 · Minecraft 0

                         [Cancel] [Apply]
```

Requirements:

- checkbox/toggle rows stage changes without immediately rebuilding assets;
- generated fallback is visible but locked on;
- fixed initial priority is authored, then Minecraft reference, then fallback;
- all optional rows may be off;
- Apply shows preparing/progress/failure state and leaves the old world visible
  until the replacement is ready;
- Cancel restores the staged selection to the active selection;
- unavailable packs remain visible with a concise reason when useful (for
  example, reference pack not installed);
- a successful Apply updates the effective label and provenance/coverage
  summary;
- Minecraft-enabled selections are unmistakably labeled local/proprietary;
- no hotkey is added.

The first tactical does not add arbitrary filesystem browsing, downloads,
drag-to-reorder, or server resource-pack prompts.

## Target Pack Set

### Mclone authored pack (optional)

Build a first-party archive from checked-in original sources and repo assets:

```text
generated-assets/texture-lab/mclone-authored.pbp
```

It should include deterministic runtime-compatible texture-lab PNGs, far-LOD
material metadata, first-party figures, and any authored effect/visual
definitions accepted into the runtime set. Generated files stay ignored; the
pack is a build/release artifact.

The current `mclone-default-overlay.pbp` remains a useful compatibility output
during migration. It must not be relabeled standalone while it still relies on
later Minecraft blockstates/models/textures.

### Generated fallback pack (always active)

Build a distinct deterministic archive:

```text
generated-assets/texture-lab/mclone-generated-fallback.pbp
```

Inputs must be repo-owned and available without a hydrated Minecraft reference
tree. Outputs cover every resource required by the canonical first-party visual
catalog, including resources also present in the authored pack. Authored assets
shadow these entries when enabled; complete fallback coverage is what makes an
authored-disabled, fallback-only selection valid. Outputs include:

- missing block/material PNGs;
- missing actor and screen-effect textures;
- first-party fallback visual definitions for uncovered block states;
- missing-resource short-code registry;
- coverage and provenance metadata.

The pack generator uses a tiny checked-in bitmap font and stable id hash. A
given resource id must produce byte-identical PNG and short code across hosts
and runs. Collision detection is mandatory; the build fails or extends the
code deterministically rather than silently aliasing two resources.

Missing audio is represented by generated/suppressed metadata rather than fake
OGG payloads unless a useful first-party sound is later authored.

### Minecraft reference pack (optional, local only)

Expose the existing local `reference/minecraft-1.17.1/extracted.zip` as the
logical `minecraft-1.17.1-reference` pack. Do not duplicate, commit, upload, or
release it merely to obtain a `.pbp` filename. Associate separately installed
local Minecraft sounds with the same logical origin/selection even though they
remain outside the compact render archive.

## Shared Contracts

Introduce shared, platform-neutral facts along these lines; exact Rust names
may adjust to existing module vocabulary:

```text
AssetPackId
AssetPackOrigin
AssetPackCapabilities
AssetPackDescriptor
AssetPackCatalog
AssetPackSelection
AssetPackSelectionDraft
AssetPackApplyState
AssetResolutionOrigin
AssetProvenanceReport
PreparedAssetSet { epoch, selection, mesh assets, actors, effects, audio }
```

The UI should use a compact copyable `AssetPackUiId` in `GameUiAction` rather
than putting owned strings into the currently copyable action enum. The shared
controller resolves UI ids against the current catalog and emits an asset-pack
selection request/effect for the scene host.

Manifest additions should include stable pack id, display name, declared
origin, role/capabilities, asset schema compatibility, and content fingerprint.
Keep additions backward-compatible when possible, but treat absent provenance
as `Unknown`; strict/proprietary-free UI claims require complete known
provenance.

Replace or augment anonymous `AssetSourceChain` reads with named resolution so
the prepared set can report which source answered every resource. The resolver
must not append local Minecraft sounds or loose reference files outside the
enabled logical selection.

## Runtime Replacement Shape

Do not mutate environment variables or rebuild an anonymous source chain from
the desktop event loop. Selection is shared policy; platform adapters supply
only discovered pack descriptors and readable sources/bytes.

The general Apply path is:

```text
staged selection
  -> validate catalog + provenance
  -> build named source stack
  -> prepare catalog/atlases/actors/effects/audio off frame path
  -> start new epoch render compiler
  -> compile current visible sections under the new catalog
  -> frame-boundary atomic presentation commit
  -> retire old compiler/resources; discard stale old-epoch results
```

The active scene continues drawing the old asset set during preparation. A
failed prepare or compile leaves the active selection untouched. World/session,
camera, inventory, input, chunk snapshots, and server connection survive the
commit.

Replacement must cover all source-backed consumers participating in the
provenance claim:

- terrain mesh catalog and texture atlas;
- render compile dispatcher/worker resident catalog;
- uploaded visible terrain sections;
- far-LOD material palette and derived cache;
- actor texture atlas and figures;
- underwater/other source-backed screen effects;
- world/UI texture consumers backed by the terrain atlas;
- audio source/suppression policy.

The first implementation always uses this correctness path. Record catalog and
atlas-layout fingerprints, but defer a compatible-atlas-only fast path until a
measured reload cost justifies it. The UI Apply flow makes a short asynchronous
prepare acceptable and avoids premature canonical-atlas complexity.

Web must preserve the same epoch semantics. Its resident render compiler worker
may be replaced/reinitialized with a newly composed pack payload; stale worker
messages/results cannot enter the new selection. JavaScript remains responsible
for fetching/staging bytes and worker lifecycle, not pack priority or fallback
policy.

## Implementation Slices

This is a parent tactical because pack construction, shared runtime replacement,
and platform adoption are independently reviewable. Keep only one slice active
at a time and update the topic after each landed slice.

### Slice 0 - Contract and Baseline Locks

- [x] Add `AssetPackId`, origin, descriptor/catalog, selection, and provenance
  report types in the shared asset/runtime boundary.
- [x] Extend pack manifest parsing with optional identity/origin/role/schema
  facts; legacy packs resolve to `Unknown` unless discovery configuration gives
  a trusted local identity.
- [x] Add named first-source-wins resolution tests, including disabled-source
  non-resolution and unknown-origin claim suppression.
- [x] Capture current real asset consumers: terrain, LOD, actors, figures,
  effects, UI atlas users, and sounds.
- [x] Define the repo-owned canonical first-party material/visual inventory
  without reading Minecraft JSON at first-party pack build time.
- [x] Add CI/static checks that runtime/app crates do not depend on texture-lab
  TypeScript or pack-builder Python.

Exit criteria: the catalog can represent authored, reference, generated, and
unavailable packs; a selection deterministically produces named source order;
tests prove a disabled Minecraft source cannot answer a read.

Landed evidence (2026-07-10):

- `mclone-assets::profile` owns validated ids, declared/trusted origins and
  roles, available/unavailable descriptors, fixed-priority catalogs,
  selections, resolution provenance, aggregate reports, and strict-claim
  suppression for Minecraft/unknown resolutions.
- Pack manifests accept optional `pack_id`, `display_name`, `origin`, `roles`,
  `asset_schema`, and `content_fingerprint`. Existing manifests remain valid
  and become `Unknown` unless an explicit trusted discovery identity supplies
  missing origin/role facts.
- `AssetSourceChain::from_selection` admits only selected plus required packs,
  orders them by engine priority then stable id, and exposes named
  first-source-wins `resolve` results. Tests place a reference-only asset in a
  disabled Minecraft source and prove it cannot resolve.
- `canonical_first_party_asset_inventory()` covers all 209 checked-in runtime
  block states with engine-owned fallback material ids and conservative visual
  classes, without filesystem or Minecraft JSON reads. It also lists direct
  LOD, actor, figure, effect, colormap, and audio paths.
- `runtime_tooling_boundary` recursively scans native app/runtime manifests and
  source files during `mclone-assets` tests and rejects imports/invocations of
  texture-lab TypeScript or pack-builder Python.

Current consumer baseline captured by Slice 0:

| Consumer | Current source-backed input / replacement implication |
|---|---|
| Terrain | Minecraft blockstate/model JSON selects atlas materials; catalog, atlas plan/image, and compiled section meshes must share one epoch. |
| Far LOD | Optional `assets/mclone/lod/materials.v1.json` palette plus derived retained patches must be replaced/invalidated together. |
| Actors | Minecraft cow PNG feeds the actor atlas; the atlas and layout are prepared resources. |
| Figures | Three repo-owned `assets/mclone/figures/*.figure.json` files are loaded with actor resources. |
| Effects | Minecraft underwater PNG is uploaded into mono/per-eye/multiview screen-effect resources. |
| UI atlas users | Hotbar/catalog icons use UVs from the terrain atlas; they have no independent asset source but must change with its layout. |
| Sounds | Two Minecraft landing OGG paths are optional today; disabled-reference policy must make their absence explicitly suppressed. |

Focused validation:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-assets
  41 unit tests passed
  1 runtime-tooling boundary integration test passed
  0 failures
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime --lib
  passed
cargo check --manifest-path native/Cargo.toml -p mclone-assets \
  --target wasm32-unknown-unknown
  passed
```

### Slice 1 - Deterministic Standalone Pack Build

- [x] Add canonical commands, expected names such as
  `texture-lab:pack-authored`, `assets:pack:generated-fallback`, and a combined
  `assets:pack:first-party` convenience gate.
- [x] Package accepted texture-lab runtime output plus repo first-party assets
  into the authored pack without local Minecraft payloads.
- [x] Generate missing texture PNGs, short ids, registry, fallback visual
  definitions, and coverage/provenance report into the generated pack.
- [x] Add deterministic-byte/fingerprint tests and short-id collision tests.
- [x] Build and verify both packs in an environment where the Minecraft
  reference root is absent/unavailable.
- [x] Verify archive entries and fingerprints contain no reference-pack bytes
  or roots.
- [x] Keep generated packs/PNGs ignored while making release/platform staging
  consume the canonical build outputs.

Exit criteria: authored + generated packs form a standalone first-party input
for the canonical asset inventory; missing art is visibly identifiable; pack
generation has zero Minecraft artifact prerequisite.

Landed evidence (2026-07-10):

- `first_party_inventory` exports the shared Rust inventory as deterministic
  JSON: 209 block states, 137 engine-owned materials, and 10 direct consumer
  requirements. The export reads no filesystem asset/reference data.
- `texture-lab:pack-authored` starts with a clean `--no-reference` export, then
  packages 94 canonical namespaced authored PNGs, 21 compatibility PNGs,
  far-LOD metadata, and three repo-owned figures as `mclone-authored.pbp` (119
  payload files). Canonical `mclone:block/*` textures can therefore shadow the
  generated material layer; compatibility paths remain available during
  migration.
- `assets:pack:generated-fallback` creates 139 labeled PNGs (137 materials plus
  cow and underwater replacements), copies the three required first-party
  figures, and emits block visuals, missing registry, coverage, and suppressed
  audio policy as `mclone-generated-fallback.pbp` (146 payload files).
- Missing PNGs use deterministic resource-derived checker colors, magenta
  borders, the checked-in 3x5 hexadecimal font, and a visible short code.
  Colliding four-character prefixes extend until unique; a full digest
  collision fails.
- Both pack manifests declare id, display name, origin, roles,
  `mclone-visuals-v1`, and a SHA-256 payload fingerprint. All ZIP entries use a
  fixed timestamp and stored compression; both PNG encoders use deterministic
  stored-DEFLATE streams, so pack and image bytes are independent of host zlib
  behavior.
- Isolated tests build beside a sentinel
  `reference/minecraft-1.17.1/` tree and prove neither the sentinel bytes nor
  reference roots enter either pack. Two builds produce identical pack and
  sidecar bytes.
- Canonical generated outputs remain under ignored `generated-assets/`. The
  combined gate copies both packs and sidecars byte-for-byte into
  `generated-assets/first-party-stage/first-party-packs/` and writes a
  fingerprinted staging catalog. Per-platform installation and live selection
  adoption remain Slice 5 work.

Focused validation:

```text
pnpm assets:pack:first-party:test
  5 tests passed
pnpm texture-lab:typecheck
  passed
cargo test --manifest-path native/Cargo.toml -p mclone-assets
  41 unit tests + 1 boundary integration test passed
pnpm assets:pack:first-party
  mclone-authored.pbp: 119 payload files
  mclone-generated-fallback.pbp: 146 payload files
second canonical build
  pack and sidecar SHA-256 values unchanged
first_party_pack.py verify <pack> --manifest <sidecar>
  both packs passed
```

Visual inspection:

- `/tmp/mclone-missing-stone-256.png`: enlarged generated stone fallback;
  inspected checker contrast, alternating magenta border, and legible `E67C`
  code.
- `/tmp/mclone-missing-cow-512.png`: enlarged 64x32 actor fallback; inspected
  distinct checker/border treatment and legible `BDE7` code.

### Slice 2 - First-Party Visual Catalog and Prepared Set

- [x] Add the engine-native first-party visual definition/catalog adapter.
- [x] Compile authored and fallback visual entries into the same neutral
  `TexturedMeshCatalog` used by the Minecraft JSON adapter.
- [x] Load terrain, LOD, actors, figures, effects, and audio policy into one
  epoch-tagged `PreparedAssetSet`.
- [x] Make missing solids/plants/fluids visibly distinct with conservative
  fallback geometry.
- [x] Produce an exact resolution ledger and aggregate coverage report.
- [x] Prove `Mclone authored -> generated fallback` prepares with no loose or
  packed Minecraft source installed.

Exit criteria: a headless/offscreen first-party-only capture is drawable and
inspected; provenance reports zero Minecraft/unknown resolutions; every missing
resource in the capture maps to a registry id.

Landed evidence (2026-07-10):

- `mclone-assets` validates `mclone-visuals-v1` against all 209 canonical
  runtime block-state ids/keys, parses the generated missing-resource registry
  and silent-audio policy, and provides a source view that records the named
  first-source-wins result for each logical path.
- `mclone-mesh` compiles those visual definitions into the same
  `TexturedMeshCatalog` consumed by render-section meshing. Full cubes,
  double-sided crossed plant planes, a low flat plane, and the existing basic
  fluid renderer provide distinct conservative geometry; normal atlas/mip and
  render-layer paths remain unchanged.
- `mclone-app-runtime::PreparedAssetSet` owns one epoch and selection plus the
  composed source chain, CPU terrain catalog/atlas, far-LOD palette, actor
  atlas, three figures, decoded underwater effect, explicit audio policy,
  missing-resource registry, exact provenance ledger, and aggregate coverage.
  This slice does not upload or replace live scene resources.
- The preparation diagnostic opened only `mclone-authored.pbp` followed by the
  required `mclone-generated-fallback.pbp`. It prepared 209 states, 137 atlas
  sprites, 3 figures, 107 far-LOD colors, and 139 missing registry entries.
  The ledger contained 11 first-party and 135 generated resolved paths, 2
  suppressed sounds, 1 absent optional colormap lookup, and zero
  Minecraft-reference or unknown resolutions. Every resolved generated PNG
  was required to have a registry entry.
- The native offscreen client ran with `MCLONE_ASSET_MODE=pack-only`, the
  authored archive as its sole overlay, and the generated archive as its sole
  base. `/tmp/mclone-first-party-slice2.png` rendered at 960x540 with 64
  sections, 11 drawn sections, 2 entities, and 2 drawn actors. Inspection
  confirmed authored terrain remained legible while generated checker/code
  textures and conservative silhouettes were conspicuous and drawable.

Focused validation:

```text
cargo test --manifest-path native/Cargo.toml \
  -p mclone-assets -p mclone-mesh -p mclone-app-runtime --lib
  43 + 85 + 195 tests passed; 0 failures
cargo run -p mclone-app-runtime --bin first_party_asset_prepare -- \
  generated-assets/texture-lab/mclone-authored.pbp \
  generated-assets/texture-lab/mclone-generated-fallback.pbp
  prepared; minecraft_reference=0 unknown=0
first-party-only native offscreen screenshot
  passed and inspected at /tmp/mclone-first-party-slice2.png
```

### Slice 3 - Transactional Native Scene Replacement

- [x] Add an async/budget-respecting prepare request owned below app crates.
- [x] Add asset epochs to compiler requests/results or replace compiler
  instances so stale results cannot cross a selection commit.
- [x] Recompile the current visible set and preserve the old drawable set until
  replacement readiness.
- [x] Add frame-boundary replacement for terrain draw resources, UI atlas
  consumers, actors, effects, far LOD, and audio policy.
- [x] Preserve session/world/camera/player state and avoid server commands or
  reconnects.
- [x] On error, retain the old selection/resources and expose a concise shared
  failure state.
- [x] Exercise the shared Mono and XR/multiview render paths; no per-eye-only
  resource replacement.

Exit criteria: a scripted `Vanilla -> Mclone Original -> Vanilla` transition
in one active world completes without reconnecting; old-epoch results are
rejected; first and final frozen-camera vanilla captures agree within the
existing deterministic render tolerance.

Landed evidence (2026-07-10):

- `PreparedSceneAssetsRequest` performs CPU pack preparation on a named worker
  and reports Pending/Ready/Failed without blocking the frame path. A second
  worker compiles the current resident section set plus neighbor snapshots
  against the candidate catalog. If relevant snapshots or target sections
  change before commit, the scene restarts that compile instead of installing
  stale geometry.
- `McloneSceneHost` owns one shared Active/PreparingAssets/PreparingMeshes/
  Failed lifecycle. Mono, per-eye stereo, and full-frame multiview entry points
  poll it at frame boundaries while the old draw set remains active.
- A successful commit creates all fallible terrain, actor, screen-effect,
  world/mono GUI atlas, far-LOD, and optional audio resources first. It then
  installs a fresh catalog-bound compiler dispatcher, replaces resident cache
  metadata from the prepared full-view report, clears far-LOD/upload state,
  and swaps every presentation consumer without touching session authority.
- Compiler-instance replacement drops the retired worker queue/results. The
  render-session epoch reset clears queued completed results and in-flight
  markers before accepting the prepared metadata; its focused stale-result
  test passes.
- `PreparedAudioAssets` makes sound-bank decode a CPU preparation product.
  First-party selections install an explicit silent bank; an active audio
  device is rebuilt from the candidate bank as part of the same fallible
  transaction.
- Failure is non-destructive. An invalid-pack diagnostic reached shared Failed
  state with active epoch 0 retained and surfaced the pack-open error; no
  resource/compiler commit occurred.

Rendered validation:

- Mono `/tmp/mclone-asset-replacement-vanilla-{baseline,first-party,restored}.png`:
  one frozen active session completed epochs `0 -> 1 -> 2`; the middle frame
  visibly used labeled first-party fallback textures, while baseline and
  restored vanilla frames had 0 differing pixels. The commit report recorded
  unchanged session, camera, command count, and update count.
- Synthetic stereo `/tmp/mclone-asset-replacement-xr.png`: the same round trip
  completed through the shared per-eye path, then rendered 24 sections, 49,723
  differing eye pixels, 32 GUI commands, and two eye UI composites. The image
  was inspected for vanilla terrain, stereo parallax, and two-eye UI.
- The five multiview terrain/actor/effect/GUI/outline distinct-view uniform
  tests pass. The existing headless GPU-layer proof reports a supported skip on
  this Mac adapter because it does not expose wgpu `MULTIVIEW`; the same
  replacement resources and frame-boundary poll are used by the compiled
  full-frame multiview path.

Focused validation:

```text
cargo test --manifest-path native/Cargo.toml \
  -p mclone-assets -p mclone-audio -p mclone-render-session \
  -p mclone-app-runtime -p mclone-render -p mclone-scene --lib
  563 passed; 0 failed; 2 pre-existing GPU proofs ignored
background prepare failure + retired epoch result tests
  passed
Mono Vanilla -> Original -> Vanilla offscreen smoke
  passed; restored pixel difference 0.000%
synthetic stereo Vanilla -> Original -> Vanilla smoke
  passed; session/camera/command/update facts preserved
cargo check -p mclone-assets -p mclone-audio \
  -p mclone-render-session -p mclone-app-runtime \
  --target wasm32-unknown-unknown
  passed (two pre-existing mclone-server warnings)
```

### Slice 4 - Shared Asset Packs UI

- [x] Add shared Asset Packs screen state, row projection, effective-selection
  label, coverage summary, staged selection, Apply, Cancel, progress, and error
  presentation in `mclone-ui`.
- [x] Add copyable pack-row UI actions and classify/route them through the
  shared client-experience facade.
- [x] Reach the screen from title and pause options.
- [x] Keep fallback visible/locked and unavailable reference packs legible.
- [x] Ensure pointer, controller, touch, and keyboard navigation all use normal
  shared UI contracts.
- [x] Add shared widget/action tests and a rendered offscreen UI capture.
- [x] Do not add a hotkey.

Exit criteria: desktop flat can stage and apply all four well-known optional
pack combinations from the UI, including fallback-only, with accurate status
and provenance labels.

Landed evidence (2026-07-10):

- `mclone-ui` owns a fixed-capacity `AssetPacksUiState` with copyable rows,
  compact `AssetPackUiId`, active/staged flags, all six row statuses,
  effective label, provenance/coverage counts, progress phase, and concise
  failure text. The generated fallback is rendered checked and locked.
- `GameUiAction` remains Copy and now carries Open/Toggle/Apply/Cancel asset
  actions. The client-experience facade resolves deterministic compact ids
  against the current catalog, rejects collisions/capacity overflow, owns the
  draft, validates Apply, and emits one `AssetPackSelection` effect.
- `Options -> Asset Packs` is present in both title and pause contexts. The
  retained UI surface provides the same pointer path used by mouse, touch, and
  XR controller-ray adapters; Escape uses the shared Cancel action and is
  suppressed while an Apply is preparing. No hotkey was added.
- The screen shows stable ids, first-party/generated/reference/unknown origin,
  unavailable reasons, active/enabled/preparing/failed status, exact resolved
  coverage, and an explicit local/proprietary warning for a staged reference
  selection. Apply/Cancel disable while preparation is in flight.
- A platform-neutral source registry prepares authored/reference/generated
  combinations through the Slice 3 request. Reference-enabled selections use
  the Minecraft catalog adapter; reference-disabled selections use the native
  first-party visual catalog and enforce zero reference/unknown resolutions.
  Offscreen injects this registry only for validation; platform discovery and
  persistence remain later work.
- One desktop-flat UI/facade smoke applied Mclone Original, Hybrid Authoring,
  Generated Fallback Only, and Vanilla Reference in one active world at epochs
  1 through 4. It then staged Hybrid without applying, proving Apply remains a
  separate user decision.
- The final 480x320 capture at `/tmp/mclone-asset-packs-ui.png` was inspected.
  It legibly shows three rows, stable ids, enabled/active/locked states,
  reference provenance warning, resolved coverage, staged Hybrid label, and
  enabled Cancel/Apply buttons.
- Web receives the same screen/action projection and compiles for WASM, but an
  Apply reports the explicit Tactical 170 cutover requirement. No asset policy
  or compiler lifecycle was added to `WebChunkRenderSession`.

Focused validation:

```text
cargo test --manifest-path native/Cargo.toml \
  -p mclone-assets -p mclone-ui -p mclone-app-runtime \
  -p mclone-scene --lib
  409 passed; 0 failed
cargo test --manifest-path native/Cargo.toml \
  -p mclone-native-client --bin mclone-native-client
  130 passed; 0 failed
UI-driven four-selection desktop-flat smoke
  passed; epochs 1..4; final staged label Hybrid Authoring
rendered Asset Packs capture
  passed and inspected at /tmp/mclone-asset-packs-ui.png
cargo check -p mclone-assets -p mclone-ui -p mclone-app-runtime \
  -p mclone-web-client --target wasm32-unknown-unknown
  passed (two pre-existing mclone-server warnings)
```

**Required next-work checkpoint:** do not automatically continue from this
slice into all of Slice 5 merely because it is numerically next. Re-read
[`170`](170-web-scene-host-adoption.md) and the current topic status. The
default recommendation after the shared replacement/UI contract is stable is
to start or resume Tactical 170 so browser adoption lands on
`McloneSceneHost`, not on the production `WebChunkRenderSession` orchestrator
that Tactical 170 deletes. Non-web discovery/staging work from Slice 5 may
still proceed first when it is the immediate platform priority, but an agent
proposing the next slice must call out this handoff explicitly.

### Slice 5 - Platform Discovery and Adoption — DONE (2026-07-11)

This is a cross-tactical adoption slice, not a requirement to modify every
current platform host in place. Native, Android, XR, and offscreen adoption may
land independently against the shared contract. The web item is coordinated
with Tactical 170:

- if Tactical 170's production cutover has not landed, switch to Tactical 170
  before implementing web asset-pack lifecycle or UI policy;
- do not add selection, compiler-epoch, resource-replacement, or UI policy to
  `WebChunkRenderSession` merely to complete this checklist; and
- implement the web item through Tactical 170's browser service adapters and
  shared-host cutover, then return here to record the platform evidence and
  finish this tactical's acceptance/audit work.

- [x] Desktop: project local loose/packed discovery into the shared catalog;
  preserve environment variables as launch/CI defaults rather than live UI
  policy.
- [x] Flat Android and Android XR: stage both first-party packs in packaging,
  discover optional local reference content, and use the shared catalog/action
  path.
- [x] Desktop XR: use the same shared scene/UI state with no app-local pack
  selector.
- [x] Web, coordinated with Tactical 170: fetch/stage pack descriptors and
  bytes, compose/reinitialize its resident compiler worker under the shared
  selection/epoch contract, and show the same native-rendered UI screen through
  `McloneSceneHost`; do not extend the old production orchestrator.
- [x] Offscreen and synthetic-stereo: add programmatic selection inputs for
  deterministic validation without inventing a separate product policy.
- [x] Update the platform parity capability/exception ledger if any host cannot
  yet apply a selection; do not silently omit the screen/action.

Handoff status (2026-07-11): Tactical 170 Slice 5 atomically moved production
local worker, IndexedDB local-world, and remote WebSocket modes onto one
`McloneSceneHost` and deleted `WebChunkRenderSession` plus the proof-only path.
The host retains the existing prepared epoch 0 across session replacements;
no competing asset epoch or browser-local selection policy was added.

Landed evidence (2026-07-11):

- `AssetPackSourceRegistry::discover_native_with_reference` projects the two
  logical first-party packs plus the optional local reference source into the
  shared catalog. Launch/CI environment paths remain discovery overrides;
  normal product discovery searches platform asset roots and the deterministic
  first-party stage. The generated fallback is required, while a missing
  authored pack remains a truthful unavailable row.
- Desktop flat, desktop XR, flat Android, Android XR, offscreen flat, and
  synthetic stereo configure the same registry and `McloneSceneHost` action
  path. There is no app-local selector. The offscreen four-profile smoke now
  exercises production discovery rather than validation-only source injection.
- Both Android builds run the deterministic first-party pack command and embed
  the staged catalog, packs, and sidecars under APK
  `assets/first-party-packs/`. Startup atomically stages the two packs into the
  app-owned `assets/packs/` discovery root before scene construction.
- The production browser fetches reference, authored, and fallback bytes, then
  configures the shared host with the resulting catalog. Apply emits a typed
  external preparation request from `McloneSceneHost`; the browser constructs
  a candidate resident compiler for the requested selection/epoch, completes
  the shared prepared-set transaction, and retires the old compiler only after
  the host reports the new epoch active. Failure retains the old compiler and
  active scene resources.
- Browser pack priority, fallback admission, selection validation, CPU asset
  composition, visible-section replacement, and commit policy stay in shared
  Rust. JavaScript owns only fetch, rAF pause/resume, worker construction, and
  retirement. The current browser CPU prepare/full-view compile is a truthful
  synchronous main-Wasm fallback while rAF is paused; moving that work to a
  dedicated preparation worker remains a performance follow-up, not a second
  policy path.
- No supported client needs a capability-ledger exception: all consume the
  shared Asset Packs screen/action and can apply a selection. Persistence is
  intentionally still Slice 6 work.

Focused validation:

```text
cargo test -p mclone-assets -p mclone-mesh -p mclone-app-runtime \
  -p mclone-scene -p mclone-ui --lib
  passed
cargo test -p mclone-native-client --bin mclone-native-client
  passed
cargo check -p mclone-app-runtime -p mclone-scene \
  -p mclone-web-client --target wasm32-unknown-unknown
  passed (two pre-existing mclone-server warnings)
pnpm native:web:typecheck
pnpm native:web:asset-pack-smoke
pnpm native:web:bundle
  passed; epoch 0 -> 1, session preserved, selected catalog 265 files,
  compiler asset epoch 1, three pack loads, shared-result transport,
  no generated-view fallback or result overflow
MCLONE_ASSET_PACK_UI_SMOKE=1 pnpm native:desktop-offscreen:smoke
  passed; production discovery applied Original, Hybrid, Fallback, and
  Vanilla at epochs 1..4 in one session
pnpm native:desktop-offscreen:smoke
pnpm native:xr-emulation:smoke
pnpm native:xr:check
  passed
pnpm native:android:apk:avd
pnpm native:android:avd-smoke -- --skip-build ...
pnpm native:android-xr:apk
  passed; flat debug and Quest release APKs contain authored and fallback
  packs; flat Android logged two staged/discovered packs and reached playable
  9/9
web scene-host, thin-adapter, scene-host, XR-frame, and asset-lock gates
  passed
```

Rendered evidence was inspected at
`/tmp/mclone-t169-slice5-native-asset-pack-ui.png`,
`/tmp/mclone-t169-slice5-native-world.png`,
`/tmp/mclone-xr-emulation.png`,
`/tmp/mclone-native-web-asset-pack-ui-probe.png`, and
`/tmp/mclone-android-avd-chunk.png`. The captures show the truthful three-row
catalog, normal textured terrain/actors, distinct stereo views, the browser's
active proprietary-free Original selection, and a live flat-Android
terrain/HUD/touch frame. No capture is committed. The Quest APK rebuilt, but
`adb` reported no attached device, so Slice 5 does not claim a fresh headset
runtime result.

Exit criteria: the Asset Packs screen is accessible and truthful on every
supported client lane; each lane either applies the selection or carries an
explicit temporary parity exception with reason and follow-up.

If the only unfinished Slice 5 item is web adoption and Tactical 170 has not
yet cut production over, the recommended next work is Tactical 170 rather than
an implementation against the soon-to-be-deleted web host. This tactical stays
open and resumes after that handoff; the explicit exception above is a truthful
intermediate state, not permission to claim Slice 5 complete.

### Slice 6 - Persistence, Audit, and Closeout

- [ ] Persist enabled logical pack ids as a client-global preference through a
  shared preference contract with platform storage adapters.
- [ ] Gracefully retain unavailable selected ids and fall back to generated
  content until they reappear.
- [ ] Add machine-readable active-selection/provenance diagnostics.
- [ ] Add a strict validation command that fails if Minecraft/unknown origin is
  resolved with the Minecraft pack disabled.
- [ ] Record reload preparation/compile/upload timing and peak retained CPU/GPU
  bytes; optimize only from evidence.
- [ ] Update tactical `115`, the topic, asset tooling docs, packaging docs, and
  platform matrix to state the final standalone boundary.

Exit criteria: selection survives restart on implemented platforms, strict
first-party validation is automated, no required provenance consumer is
unaccounted, and the topic accurately records remaining coverage rather than
claiming completion from texture counts alone.

## Validation Matrix

Run focused gates per slice, then the applicable platform matrix from
[`../platforms.md`](../platforms.md#validation-policy). Minimum campaign gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-assets
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-ui
pnpm texture-lab:typecheck
pnpm assets:pack:first-party
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
```

Rendered-output slices must capture to `/tmp`, inspect the first drawable
milestone, and repeat visual checks as replacement complexity grows. Required
scenarios:

1. Vanilla reference only.
2. Mclone authored plus generated fallback, Minecraft disabled.
3. Hybrid authoring, showing authored overrides and nonzero Minecraft origin.
4. Generated fallback only, with legible stable missing ids.
5. In-world Apply failure, proving the old set remains drawable.
6. Vanilla -> Original -> Vanilla in one unchanged session.
7. UI screen under flat pointer/touch and XR controller navigation.

For strict scenario 2, delete/rename nothing in the user's real reference tree;
use explicit test sources or an isolated staging root to prove no reference
dependency.

## Performance Contract

- Missing PNG/visual generation is pack-time only.
- No shader branch or extra draw call distinguishes authored from missing
  textures.
- No frame waits on filesystem/network pack IO.
- Preparation may retain old and new CPU/GPU resources transiently; measure and
  bound that overlap, especially on Quest and mobile web.
- Old resources retire only after the frame/queue lifetime makes replacement
  safe.
- Do not add an atlas-only fast path until full replacement timing demonstrates
  a material user-visible or memory benefit.

## Non-Goals

- No asset-pack hotkey.
- No arbitrary user pack import/browser or drag-to-reorder.
- No server resource-pack negotiation, download, or enforcement.
- No runtime dependency on texture-lab authoring code.
- No runtime/shader missing-texture synthesis.
- No requirement for generated fallback geometry to match vanilla shapes.
- No proprietary-free claim for hybrid or unknown-origin selections.
- No world/save/protocol mutation caused by asset selection.

## Documentation Handoff

Update [`../topics/asset-pack-profiles.md`](../topics/asset-pack-profiles.md)
whenever a slice changes the current contract, evidence, coverage, or next
recommended work. Related commits should normally carry:

```text
Topic: asset-pack-profiles
```

When Slice 4 lands, the topic's recommended-next section must compare the
remaining non-web Slice 5 work with Tactical 170 instead of mechanically
recommending the next number. When web is the next missing adoption lane, it
must recommend Tactical 170 until the shared browser host/cutover is ready,
then route back here for Slice 5 evidence and Slice 6 closeout.
