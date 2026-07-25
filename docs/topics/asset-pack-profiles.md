# Asset Pack Selection and Provenance Topic

Topic: asset-pack-profiles

Status: implementation complete 2026-07-11. Tactical
[`169`](../tactical/169-runtime-asset-pack-selection.md) Slices 0-6 and Tactical
[`170`](../tactical/170-web-scene-host-adoption.md) Slices 0-6 landed. Selection,
transactional replacement, platform adoption, persistence, strict provenance,
reload measurement, and attached Quest asset discovery are executable.

Post-closeout correction (2026-07-20): the browser driver now waits for any
async asset/session operation to release wasm-bindgen's mutable host borrow
before taking a queued IndexedDB catalog execution. This prevents an Asset
Packs Apply report from racing a catalog completion and recursively borrowing
`WebSceneHost`. The production UI probe completes authored selection and reload
restoration with zero reference/unknown provenance. Its screenshot gate now
forces overview submissions and rejects transparent or monochrome captures;
Chrome 147/150 on the current displayless Linux host remains blocked at that
presentation/readback boundary despite green shared-host receipts.

Post-closeout extension (2026-07-21): Texture Lab now authors fourteen original
farmstead runtime materials for Structure Lab. They replace diagnostic
fallback tiles for plaster, cobblestone, mossy cobblestone, oak log, oak and
spruce planks, brick, poppy, cornflower, and wall torch, then add spruce log,
stone brick, red terracotta, and hay for the barn family. The strict
prepared-set ledger now reports 26 first-party and 126 generated resolutions,
with zero Minecraft-reference or unknown resolutions; Structure Lab preview
receipts report 22 first-party and 122 generated resolutions.

Post-closeout extension (2026-07-24): desktop mono UI configuration now
preserves the already-discovered asset-pack catalog instead of replacing the
entire client-experience controller. This fixes the title/pause screen
incorrectly showing staged first-party packs as unavailable. The process-local
`--asset-pack original` launch profile preflights and forces Mclone Original
without reading or rewriting the saved preference; `pnpm native:original-assets`
builds the packs and launches that profile. Cow has joined player, chicken, and
upright bear as a promoted authored runtime figure, and ordinary cow actors now
select that prepared figure. The strict prepared-set ledger reports 27
first-party and 126 generated resolutions, four figures, zero resolved
Minecraft-reference/unknown assets, two suppressed sounds, and one optional
missing colormap.

Scope: client-side discovery, selection, composition, provenance, preparation,
and replacement of visual/audio asset packs. This topic owns the product truth
behind “Minecraft reference assets are disabled” and the continuing path from
the current texture-lab overlay to a standalone distributable first-party asset
set. Texture authoring itself remains in tactical
[`114`](../tactical/114-ai-texture-pack-lab.md) and its tooling docs; the
existing first-party overlay bridge remains recorded in tactical
[`115`](../tactical/115-first-party-texture-pack-integration.md).

## Product Decision

Asset packs are selected from a shared in-game **Asset Packs** screen. The
screen is reachable from title and pause options on every client profile. The
initial product has no asset-pack hotkey.

The UI presents discovered optional packs as enabled/disabled rows, stages
changes, and applies the complete selection only when the user chooses
**Apply**. The initial implementation uses deterministic engine-defined pack
priority rather than drag-to-reorder. The screen must show:

- display name and stable pack id;
- enabled, unavailable, preparing, active, or failed state;
- first-party, generated, Minecraft-reference, or unknown origin;
- local-only / non-distributable labeling for Minecraft reference content;
- the effective selection label and authored/missing coverage summary;
- an always-last generated fallback row, visible but not disableable.

The useful well-known selections are consequences of the enabled rows, not
hard-coded mutually exclusive runtime modes:

| Optional selection | Effective label | Meaning |
|---|---|---|
| Mclone authored only | Mclone Original | Authored assets over generated first-party fallbacks; no Minecraft source is eligible. |
| Minecraft reference only | Vanilla Reference | Local Minecraft 1.17.1 assets, with generated fallback only for engine-owned gaps. |
| Mclone authored + Minecraft reference | Hybrid Authoring | Authored overrides shadow the local reference pack; useful for texture-lab coverage work, but proprietary content is active. |
| No optional pack | Generated Fallback Only | Identifiable generated textures/shapes and silence for missing audio. |

“Mclone Original” and “Generated Fallback Only” may be described as
proprietary-free only when runtime provenance evidence reports zero resolved
Minecraft/unknown assets. Merely putting the authored overlay before the
Minecraft pack is not sufficient.

## Current State (Verified 2026-07-11)

- `mclone-assets` has validated ZIP-backed `PackedAssetSource` packs and a
  first-source-wins `AssetSourceChain`.
- `mclone-assets` now owns `AssetPackId`, origin/role facts,
  available/unavailable descriptors and catalogs, deterministic selections,
  named resolution results, and epoch-ready provenance reports. Existing
  anonymous chain construction remains compatible and reports `Unknown`.
- `AssetSourceChain::from_selection` constructs only the selected and required
  named sources in fixed priority order. Focused tests prove a disabled local
  Minecraft source cannot answer a read and an unknown resolved source blocks
  a proprietary-free claim.
- Pack manifest parsing accepts optional id, display name, origin, roles, asset
  schema, and content fingerprint. Legacy manifests stay `Unknown` unless
  platform discovery supplies an explicit trusted identity.
- The repo-owned canonical first-party inventory covers the 209 checked-in
  runtime block states using engine-owned fallback material ids/visual classes
  and explicitly lists direct actor, figure, effect, colormap, and sound
  inputs. Constructing it performs no filesystem or Minecraft JSON reads.
- The `mclone-assets` focused suite includes a static native app/runtime source
  scan that rejects texture-lab TypeScript and pack-builder Python
  imports/invocations.
- `pnpm assets:pack:first-party` now produces deterministic standalone
  `mclone-authored.pbp` and `mclone-generated-fallback.pbp` artifacts without a
  Minecraft reference prerequisite. Their manifests declare stable identity,
  origin, roles, `mclone-visuals-v1`, and payload fingerprints.
- The authored pack currently contains canonical namespaced and compatibility
  PNGs, Structure Lab preview artifacts, and four first-party figures.
  Canonical authored `mclone:block/*` materials can shadow the generated
  layer. The fallback pack contains labeled PNGs, the figures, block visual
  records, a short-code registry, coverage facts, and suppressed-audio policy.
- Generated PNGs use deterministic checker colors, alternating magenta border,
  and a checked-in 3x5 font. Prefix collisions extend deterministically and
  full-hash collisions fail. Both PNG encoders use stored-DEFLATE streams to
  avoid host-zlib byte drift. Representative material and actor PNGs were
  enlarged under `/tmp` and inspected for contrast/code legibility.
- The combined build stages byte-identical packs/sidecars plus a fingerprinted
  catalog under ignored
  `generated-assets/first-party-stage/first-party-packs/`. Platform packaging,
  web bundling, and native discovery now consume that canonical stage.
- Native discovery projects environment overrides, platform `assets/packs/`
  roots, and deterministic build-stage candidates into one shared source
  registry. Desktop flat/XR, flat Android, Android XR, offscreen, and synthetic
  stereo all configure that registry on `McloneSceneHost`; no app owns a
  separate selection policy.
- Flat Android and Quest APK builds generate and embed both first-party packs.
  Android startup atomically stages them into the app-owned discovery root.
  The flat AVD logged both packs as staged/discovered and rendered a playable
  scene. On Quest, the writable internal app-data root owns embedded-pack
  staging while external app data remains a read-only discovery source for the
  ADB-installed reference archive. The attached Quest 3 canary discovered the
  6,985-file reference pack plus both internal first-party packs, reached a
  playable 9/9 target, and rendered 134 sections (37 drawn) and two actors on
  Oculus OpenXR/Adreno 740 without an app-fatal marker.
- Production web fetches all three pack payloads, projects their catalog into
  the shared host, and transactionally replaces both shared scene resources and
  the resident compiler at one asset epoch. The browser Apply probe completed
  epoch `0 -> 1` without replacing its local-world session and reported a
  265-file authored-plus-fallback selection with three worker pack loads.
- A versioned shared preference stores enabled logical ids, reconciles them
  against current availability, and retains unavailable/undiscovered ids until
  they reappear. Scene restoration uses the ordinary epoch transaction and
  persistence occurs only after commit. Native clients use a JSON file beside
  the client-global world root; web uses `mclone.assetPacks.v1` localStorage.
- Browser Apply/reload and native file-reopen smokes restore Original at epoch
  1 with the session intact and zero resolved reference/unknown provenance.
  Preference failures are machine-readable diagnostics rather than asset
  transaction failures.
- `pnpm assets:validate:first-party` emits a strict JSON ledger and fails on any
  resolved Minecraft-reference or unknown source. The verified 156-entry
  ledger contains 27 first-party and 126 generated resolutions, two suppressed
  sounds, one optional missing result, and no resolved reference/unknown source.
- Reload diagnostics record stage timings plus estimated simultaneous retained
  CPU/GPU payloads. The measured native Vanilla reload took 68.190 ms prepare,
  195.425 ms compile, 46.896 ms upload, and 310.511 ms total, with estimated
  peaks of 40,803,488 CPU and 82,816,592 GPU bytes. The estimates omit allocator
  and driver overhead and do not yet justify a compatible-atlas fast path.
- `mclone-assets` now validates the engine-native `mclone-visuals-v1` catalog
  against all 209 canonical state ids/keys and parses the generated
  missing-resource registry and explicit silent-audio policy.
- `mclone-mesh` compiles first-party solid, crossed-plane, flat, and fluid
  definitions into the same neutral `TexturedMeshCatalog` used by the
  Minecraft JSON adapter. It builds the ordinary texture atlas directly from
  `mclone:block/*` materials; no blockstate/model JSON is required.
- `mclone-app-runtime::PreparedAssetSet` now groups one epoch/selection with
  the composed source chain, terrain catalog/atlas, actors, figures, decoded
  screen effect, audio policy, missing-resource registry, exact resolution
  ledger, and aggregate coverage. It remains the CPU-side preparation layer
  consumed by the separate live scene replacement request.
- Tactical 170 Slice 1 moved `TexturedMeshAssets`, its CPU atlas wrapper, and
  source-backed CPU preparation into always-compiled
  `mclone_app_runtime::render_asset_data`. Filesystem discovery, native compile
  workers, GPU upload, and prepared replacement requests remain in their
  existing native/platform owners. The Tactical 169 epoch and transactional
  replacement contract is unchanged.
- A real authored-plus-generated prepare resolved 27 unique paths from the
  authored pack and 126 from the generated pack, explicitly suppressed two
  sounds, and reported zero Minecraft-reference or unknown resolutions. All
  resolved generated PNGs were checked against the 139-entry registry.
- A 960x540 first-party-only native offscreen capture rendered 11 terrain
  sections and two actors and was inspected at
  `/tmp/mclone-first-party-slice2.png`. It used only the two standalone packed
  sources; authored terrain and conspicuous checker/code fallbacks were both
  visible.
- Native asset Apply is now transactional below app crates. CPU pack loading
  and resident-view mesh compilation run on background requests while the
  active scene continues drawing. Relevant snapshot/target changes restart the
  candidate compile rather than allowing a stale commit.
- `mclone-scene` owns one frame-boundary commit for terrain/catalog/compiler,
  actor atlas/figures, mono and world GUI atlas users, effects, and prepared
  audio. Compiler-instance replacement plus render-session reset prevents
  retired queued results or in-flight markers from crossing epochs.
- Apply failure retains the old active epoch/resources and exposes a concise
  shared Failed status. Successful commit diagnostics verify that session,
  camera, command count, and update count are unchanged.
- Frozen Mono and synthetic-stereo `Vanilla -> Mclone Original -> Vanilla`
  smokes complete in one session. Mono vanilla baseline/restored captures are
  pixel-identical; the inspected middle capture uses generated labeled
  textures. Stereo retains parallax and both UI composites. Multiview consumers
  share the replaced resources and pass their distinct-view data tests; this
  Mac's headless adapter lacks the optional wgpu GPU multiview feature.
- The shared Asset Packs screen is reachable through title and pause Options.
  It owns compact copyable rows/actions, staged Apply/Cancel, fixed priority,
  locked fallback, unavailable reasons, effective labels, exact active
  provenance/coverage, preparation phases, and retryable failure text.
- The client-experience facade resolves copyable row ids against its catalog
  and emits full selection effects to `mclone-scene`; successful scene commits
  refresh the active selection and provenance projection, while failure leaves
  the draft and old active facts visible.
- A UI-driven desktop-flat smoke applied all four well-known optional
  combinations in one world at epochs 1 through 4. The inspected 480x320
  `/tmp/mclone-asset-packs-ui.png` capture shows a staged Hybrid selection,
  locked fallback, local/proprietary warning, resolved coverage, and enabled
  Apply/Cancel controls.
- `mclone-app-runtime::render_assets` discovers environment/platform paths and
  constructs one source chain at startup. `MCLONE_ASSET_OVERLAY_PACK` inserts
  one or more authored overlays before loose or packed Minecraft sources.
- The generated `mclone-default-overlay.pbp` is deliberately partial. It
  supplies authored texture-lab outputs but still needs local Minecraft
  blockstate/model JSON and missing textures from the later source chain.
- Terrain preparation couples a baked `TexturedMeshCatalog` and texture atlas
  in `TexturedMeshAssets`. Native render compile workers capture the catalog at
  construction, and draw resources own their GPU atlas. Replacing only
  source-chain state would leave stale derived resources.
- Actor, underwater-effect, figure, and landing-audio preparation all resolve
  through the selected named source chain. First-party packs provide actor,
  effect, and figure replacements plus explicit silent-audio policy, so the
  global provenance claim covers more than terrain textures.
- Web fetches reference, authored, and fallback bytes and initializes a
  replacement resident compiler from the selected logical set at each asset
  epoch. Native platforms discover filesystem/staged roots. Pack byte discovery
  remains platform glue; selection and composition policy are shared behavior.

## Logical Pack Model

The runtime should distinguish a physical archive/source from a logical pack
selection. A logical pack has a stable id and can describe one or more staged
components, such as a compact render archive plus a separately installed local
sound root. Suggested shared facts:

- `AssetPackId` and user-facing name;
- `AssetPackOrigin`: `FirstParty`, `Generated`, `MinecraftReference`, or
  `Unknown`;
- pack role/capabilities: authored override, reference base, generated
  fallback, render content, audio content, or metadata;
- asset-schema compatibility and content fingerprint;
- availability and failure reason;
- deterministic priority and whether the row may be disabled;
- source ids used in per-resolution provenance reporting.

`AssetPackCatalog` is the discovered set. `AssetPackSelection` is the ordered
set of enabled logical pack ids. Apps may discover paths, URLs, Android staged
roots, or already-loaded bytes, but they project them into these shared facts.
No app owns different selection semantics.

The initial fixed resolution order is:

1. Mclone authored pack, if enabled.
2. Minecraft reference pack, if enabled.
3. Generated fallback pack, always enabled.

Future user-installed packs may justify explicit reordering, dependency
resolution, or server suggestions. Those are not needed to establish the
first honest two-pack comparison.

## Pack-Time Generated Fallback

Missing render assets are generated by asset tooling before packaging, not by
shaders and not on the frame path. Keep the generated fallback as a distinct
logical layer even if release packaging later combines its bytes with another
first-party archive.

The fallback build consumes a repo-owned canonical visual/material inventory.
It must succeed without `reference/minecraft-1.17.1/` being present and must
not copy, inspect, fingerprint, or package Minecraft payload bytes. It writes:

- deterministic fallback PNGs for every canonical texture material, including
  ones normally shadowed by authored art;
- engine-native fallback visual definitions for every canonical block state;
- generated replacements for required actor/effect textures;
- a missing-asset registry mapping visible short ids to full resource ids;
- a coverage/provenance report embedded in or alongside the pack;
- a normal `mclone-pack.json` manifest with origin, role, schema, and payload
  fingerprints.

A missing texture should use a high-contrast checker/border, deterministic
colors derived from the full resource id, and a small bitmap-font code such as
`7K3P`. The complete registry maps that code to a resource such as
`minecraft:block/oak_log`. When the targeted block uses a missing material,
debug UI can show the exact path. The atlas and normal mip generator consume
the PNG exactly like authored art, so steady-state rendering adds no shader
branch, lookup, draw call, or per-frame generation cost.

Fallback geometry should be conspicuous and useful rather than pretending to
be parity content: full cubes for unknown solids, crossed planes for classified
plants, the shared basic fluid shape for fluids, and explicit missing-shape
metadata. Authored first-party visual definitions replace these entries over
time.

Missing audio in a Minecraft-disabled selection resolves to an explicit
suppressed/silent result with provenance accounting. It must not reach a
separately discovered local Minecraft sound root behind the user's selection.

## Standalone First-Party Boundary

The current overlay is an authoring aid, not the distributable boundary. The
target logical first-party stack is:

```text
mclone-authored.pbp
  original texture-lab outputs
  first-party figures, effects, metadata, and authored visual definitions

mclone-generated-fallback.pbp
  pack-time generated missing textures and fallback visual definitions
  always last, original/procedural provenance
```

The local Minecraft reference archive remains separate and ignored. The
existing `extracted.zip` payload can be exposed through a clearly named logical
pack such as `minecraft-1.17.1-reference`; duplicating it solely to change its
filename is unnecessary.

The engine needs a first-party visual-definition format or equivalent native
catalog builder. Texture-only strict mode is impossible while catalog loading
requires Minecraft blockstate/model JSON before it can discover materials.
Generated imitation Minecraft JSON is not the preferred long-term boundary;
both the Minecraft adapter and the first-party format should compile into the
same neutral baked mesh catalog.

## Runtime Application Contract

Pack changes are client presentation changes. They do not reconnect, mutate
server/world authority, alter saves, or change protocol block-state ids.

Applying a staged selection is transactional:

1. Validate availability, compatibility, and provenance policy.
2. Prepare the complete CPU asset set and replacement render compiler/catalog
   away from the frame path.
3. Recompile the current visible render set under a new asset epoch while the
   old selection remains drawable.
4. At a frame boundary, replace terrain atlas/meshes, actor resources,
   effects, UI texture consumers, and audio resolver as one logical commit.
5. Discard stale old-epoch compile results. On failure, keep the previous
   active selection and show the error in the Asset Packs screen.

The first implementation may use the general full prepared-set replacement
path for every Apply. A UI-mediated switch does not require an atlas-only fast
path. Catalog/layout fingerprints should still be recorded so a later measured
optimization can replace only compatible GPU textures without changing the
contract.

Selection is a client-global preference, never a world fact. The first slice
may keep it process/session-local if a shared cross-platform preference store
is not ready, but the state type and UI action must already live below app
crates so persistence can be added without redefining selection behavior.

## Provenance and Audit Contract

`AssetSource::read -> Option<Vec<u8>>` does not reveal which source answered a
lookup. Pack selection needs named resolution results or an equivalent
resolver-owned ledger containing:

- resource id;
- source/pack id and declared origin;
- whether it was authored, generated, suppressed, or missing;
- active selection/asset epoch;
- aggregate counts suitable for the Asset Packs UI and validation output.

Unknown/unmarked packs may be used in a custom/hybrid selection, but they
prevent the UI and diagnostics from claiming a proprietary-free result. The
Minecraft reference pack and its local sound components must be impossible to
resolve when their logical pack is disabled.

## Ownership

- `mclone-assets`: pack/catalog/selection/provenance contracts, manifests,
  named source resolution, first-party visual definitions.
- Asset tooling and texture lab: deterministic authored and generated pack
  construction; no runtime dependency on TypeScript/Python authoring code.
- `mclone-mesh`: compile neutral visual definitions into catalogs and expose
  catalog/layout fingerprints.
- `mclone-app-runtime`: prepared asset sets, epoch-aware compiler replacement,
  source discovery projections, and provenance reports.
- `mclone-scene`: asset selection request lifecycle and atomic replacement
  across presentation consumers.
- `mclone-ui`: shared Asset Packs screen and staged selection model.
- `mclone-render`: replaceable GPU resources; per-eye and multiview paths keep
  using the same resources.
- App/platform adapters: discover/stage paths, URLs, or bytes and route raw UI
  input. They do not decide pack priority or fallback policy.

## Validation Invariants

- First-party/fallback packs build deterministically on a checkout with no
  Minecraft reference tree.
- Disabling the Minecraft logical pack yields zero Minecraft/unknown resolved
  assets in the audit ledger.
- Deselecting all optional packs remains drawable through the generated
  fallback layer.
- Apply failure preserves the old drawable world and selection.
- `Vanilla -> Mclone Original -> Vanilla` does not reconnect or change world,
  camera, block-state, or session facts.
- After Apply settles, missing assets use normal atlas/render paths with no
  continuing generation work or frame-path branch.
- The same shared UI action works in flat, XR, Android, web, offscreen/emulated
  test profiles; only discovery and byte staging vary.

## Known Gaps

- Legacy reference/overlay builders do not emit the optional manifest
  provenance fields; their outputs remain `Unknown` until trusted discovery
  identifies them. The two new standalone first-party builders do emit them.
- The compatibility overlay remains partial; use the authored + generated
  standalone outputs for new first-party work.
- Web pauses rAF and performs CPU selected-pack preparation plus current-view
  compilation synchronously in main Wasm before the frame-boundary commit. It
  uses the same shared preparation/session interfaces and the resident compiler
  is a real worker at the requested epoch, but browser CPU preparation should
  move to a dedicated worker after measurement.
- Current clients still load/fetch the reference payload to construct epoch 0
  before restoring a first-party preference. The authored/generated archives
  and strict resolution graph are standalone, but a fully proprietary-free
  distribution bootstrap still needs a first-party epoch-0 startup/package
  path. Inactive-reference provenance does not claim the payload was never
  staged or fetched.

## Recommended Next Work

Tactical 169 — Runtime Asset Pack Selection and Tactical 170 — Web Scene-Host
Adoption are complete. Tactical 171 — Convergence And Parity Closeout tracks
the two independent follow-ups: measure whether browser preparation warrants a
dedicated worker, and open a focused first-party epoch-zero bootstrap/package
slice when a never-fetch-proprietary distribution becomes a current product
requirement. Do not reopen Tactical 169 — Runtime Asset Pack Selection.

## Slice 0 Evidence

Focused validation on 2026-07-10:

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

The test set covers optional manifest metadata and trusted legacy discovery,
catalog availability/order, always-active generated fallback admission,
named first-source-wins resolution, disabled-reference non-resolution,
unknown-origin claim suppression, the 209-state canonical inventory, direct
consumer paths, and the authoring-tool dependency lock.

## Slice 1 Evidence

Focused validation on 2026-07-10:

```text
pnpm assets:pack:first-party:test
  5 tests passed
pnpm texture-lab:typecheck
  passed
cargo test --manifest-path native/Cargo.toml -p mclone-assets
  41 unit tests + 1 boundary integration test passed
pnpm assets:pack:first-party
  authored: 119 payload files
  generated fallback: 146 payload files
  staged both packs, sidecars, and catalog
second canonical build
  pack and sidecar SHA-256 values unchanged
first_party_pack.py verify <pack> --manifest <sidecar>
  both packs passed
```

The Python tests build twice from isolated first-party fixtures, exercise
short-code extension and full-collision failure, place proprietary sentinel
bytes under a sibling `reference/minecraft-1.17.1/` tree, and verify that only
the two canonical outputs reach staging. Archive inspection confirmed fixed
timestamps, stored entries, declared fingerprints, no reference-root names,
and no sentinel bytes. Enlarged generated stone (`E67C`) and cow (`BDE7`) PNGs
were visually inspected for checker/border contrast and code legibility.

## Slice 2 Evidence

Focused validation on 2026-07-10:

```text
cargo test --manifest-path native/Cargo.toml \
  -p mclone-assets -p mclone-mesh -p mclone-app-runtime --lib
  43 + 85 + 195 tests passed; 0 failures
cargo run -p mclone-app-runtime --bin first_party_asset_prepare -- \
  generated-assets/texture-lab/mclone-authored.pbp \
  generated-assets/texture-lab/mclone-generated-fallback.pbp
  209 states; 137 atlas sprites; 3 figures; 107 LOD colors
  first_party=11; generated=135; suppressed=2
  minecraft_reference=0; unknown=0
native first-party-only offscreen capture
  960x540; 11 drawn sections; 2 drawn actors
```

The capture at `/tmp/mclone-first-party-slice2.png` was visually inspected.
The generated short-code textures are conspicuous and legible at normal scene
scale, authored terrain remains visible, and actor/plant silhouettes remain
drawable. The prepared-set validator rejects any Minecraft/unknown resolved
origin and any generated PNG resolution without a matching missing-resource
registry id.

## Slice 3 Evidence

Focused validation on 2026-07-10:

```text
six changed shared/native crate suites
  563 passed; 0 failed; 2 pre-existing GPU proofs ignored
Mono asset replacement smoke
  epochs 0 -> 1 -> 2; 64 sections; 9 drawn
  restored vanilla pixel difference 0.000%
synthetic stereo asset replacement smoke
  epochs 0 -> 1 -> 2; 24 drawn sections
  49,723 differing eye pixels; 2 eye UI composites
invalid-pack failure smoke
  failed with active epoch 0 retained
WASM check for changed shared crates
  passed
```

The Mono baseline, first-party middle, and restored captures under `/tmp` were
inspected. The first and final vanilla images are identical; the middle image
shows the conspicuous generated material codes in the unchanged camera view.
The inspected XR image shows two distinct vanilla eyes after the round trip.
Commit reports prove session/camera and command/update counts did not change.
The retired-result test proves old compiler results and in-flight state are
discarded when the fresh catalog-bound compiler instance becomes active.

## Slice 4 Evidence

Focused validation on 2026-07-10:

```text
assets + UI + app-runtime + scene library suites
  409 passed; 0 failed
native client/CLI suite
  130 passed; 0 failed
UI-driven desktop-flat selection smoke
  Original -> Hybrid -> Fallback -> Vanilla
  epochs 1 -> 2 -> 3 -> 4 in one active world
WASM check for assets/UI/app-runtime/web client
  passed
```

The final staged-Hybrid screen was rendered at 480x320 and inspected at
`/tmp/mclone-asset-packs-ui.png`. All three rows are legible with stable ids,
origin, Enabled/Active state, and locked fallback. The active Vanilla
provenance summary reports 1,252 Minecraft resolutions, while the staged
Hybrid warning is explicitly local/proprietary and Apply remains a separate
enabled action. Widget tests cover title/pause entry, pointer activation,
locked/unavailable rows, Apply/Cancel, preparation gating, failure text, and
Escape. The shared action enum remains copyable and no hotkey was added.

## Non-Goals

- No asset-pack hotkey in the initial product.
- No drag/drop pack ordering or arbitrary pack import in the first tactical.
- No server-mandated/downloaded pack protocol.
- No invocation of texture-lab authoring code from the runtime.
- No runtime or shader-time missing-texture synthesis.
- No claim that Hybrid Authoring is proprietary-free.
- No requirement that placeholder geometry visually match Minecraft models.
