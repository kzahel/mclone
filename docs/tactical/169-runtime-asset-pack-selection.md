# 169: Runtime Asset Pack Selection

Status: active implementation parent 2026-07-10; Slices 0-1 landed. Stop
boundary honored after deterministic standalone pack construction; Slice 2 is
next.

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

- [ ] Add the engine-native first-party visual definition/catalog adapter.
- [ ] Compile authored and fallback visual entries into the same neutral
  `TexturedMeshCatalog` used by the Minecraft JSON adapter.
- [ ] Load terrain, LOD, actors, figures, effects, and audio policy into one
  epoch-tagged `PreparedAssetSet`.
- [ ] Make missing solids/plants/fluids visibly distinct with conservative
  fallback geometry.
- [ ] Produce an exact resolution ledger and aggregate coverage report.
- [ ] Prove `Mclone authored -> generated fallback` prepares with no loose or
  packed Minecraft source installed.

Exit criteria: a headless/offscreen first-party-only capture is drawable and
inspected; provenance reports zero Minecraft/unknown resolutions; every missing
resource in the capture maps to a registry id.

### Slice 3 - Transactional Native Scene Replacement

- [ ] Add an async/budget-respecting prepare request owned below app crates.
- [ ] Add asset epochs to compiler requests/results or replace compiler
  instances so stale results cannot cross a selection commit.
- [ ] Recompile the current visible set and preserve the old drawable set until
  replacement readiness.
- [ ] Add frame-boundary replacement for terrain draw resources, UI atlas
  consumers, actors, effects, far LOD, and audio policy.
- [ ] Preserve session/world/camera/player state and avoid server commands or
  reconnects.
- [ ] On error, retain the old selection/resources and expose a concise shared
  failure state.
- [ ] Exercise the shared Mono and XR/multiview render paths; no per-eye-only
  resource replacement.

Exit criteria: a scripted `Vanilla -> Mclone Original -> Vanilla` transition
in one active world completes without reconnecting; old-epoch results are
rejected; first and final frozen-camera vanilla captures agree within the
existing deterministic render tolerance.

### Slice 4 - Shared Asset Packs UI

- [ ] Add shared Asset Packs screen state, row projection, effective-selection
  label, coverage summary, staged selection, Apply, Cancel, progress, and error
  presentation in `mclone-ui`.
- [ ] Add copyable pack-row UI actions and classify/route them through the
  shared client-experience facade.
- [ ] Reach the screen from title and pause options.
- [ ] Keep fallback visible/locked and unavailable reference packs legible.
- [ ] Ensure pointer, controller, touch, and keyboard navigation all use normal
  shared UI contracts.
- [ ] Add shared widget/action tests and a rendered offscreen UI capture.
- [ ] Do not add a hotkey.

Exit criteria: desktop flat can stage and apply all four well-known optional
pack combinations from the UI, including fallback-only, with accurate status
and provenance labels.

### Slice 5 - Platform Discovery and Adoption

- [ ] Desktop: project local loose/packed discovery into the shared catalog;
  preserve environment variables as launch/CI defaults rather than live UI
  policy.
- [ ] Flat Android and Android XR: stage both first-party packs in packaging,
  discover optional local reference content, and use the shared catalog/action
  path.
- [ ] Desktop XR: use the same shared scene/UI state with no app-local pack
  selector.
- [ ] Web: fetch/stage pack descriptors and bytes, compose/reinitialize its
  resident compiler worker under the shared selection/epoch contract, and show
  the same native-rendered UI screen.
- [ ] Offscreen and synthetic-stereo: add programmatic selection inputs for
  deterministic validation without inventing a separate product policy.
- [ ] Update the platform parity capability/exception ledger if any host cannot
  yet apply a selection; do not silently omit the screen/action.

Exit criteria: the Asset Packs screen is accessible and truthful on every
supported client lane; each lane either applies the selection or carries an
explicit temporary parity exception with reason and follow-up.

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
