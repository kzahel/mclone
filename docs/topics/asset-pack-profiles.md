# Asset Pack Selection and Provenance Topic

Topic: asset-pack-profiles

Status: active implementation. Tactical
[`169`](../tactical/169-runtime-asset-pack-selection.md) Slices 0-1 landed
2026-07-10; Slice 2 first-party visual catalog/prepared set is next.

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

## Current State (Verified 2026-07-10)

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
  and explicitly lists direct LOD, actor, figure, effect, colormap, and sound
  inputs. Constructing it performs no filesystem or Minecraft JSON reads.
- The `mclone-assets` focused suite includes a static native app/runtime source
  scan that rejects texture-lab TypeScript and pack-builder Python
  imports/invocations.
- `pnpm assets:pack:first-party` now produces deterministic standalone
  `mclone-authored.pbp` and `mclone-generated-fallback.pbp` artifacts without a
  Minecraft reference prerequisite. Their manifests declare stable identity,
  origin, roles, `mclone-visuals-v1`, and payload fingerprints.
- The authored pack currently contains 94 canonical namespaced PNGs, 21
  compatibility PNGs, far-LOD metadata, and three first-party figures (119
  payload files). Canonical authored `mclone:block/*` materials can shadow the
  generated layer. The fallback pack contains 139 labeled PNGs, the figures,
  209 block visual records, a short-code registry, coverage facts, and
  suppressed-audio policy (146 payload files).
- Generated PNGs use deterministic checker colors, alternating magenta border,
  and a checked-in 3x5 font. Prefix collisions extend deterministically and
  full-hash collisions fail. Both PNG encoders use stored-DEFLATE streams to
  avoid host-zlib byte drift. Representative material and actor PNGs were
  enlarged under `/tmp` and inspected for contrast/code legibility.
- The combined build stages byte-identical packs/sidecars plus a fingerprinted
  catalog under ignored
  `generated-assets/first-party-stage/first-party-packs/`. Platform installation
  and runtime selection remain later slices.
- `mclone-app-runtime::render_assets` discovers environment/platform paths and
  constructs one source chain at startup. `MCLONE_ASSET_OVERLAY_PACK` inserts
  one or more authored overlays before loose or packed Minecraft sources.
- The generated `mclone-default-overlay.pbp` is deliberately partial. It
  supplies authored texture-lab outputs but still needs local Minecraft
  blockstate/model JSON and missing textures from the later source chain.
- Terrain preparation couples a baked `TexturedMeshCatalog`, texture atlas, and
  far-LOD material palette in `TexturedMeshAssets`. Native render compile
  workers capture the catalog at construction, and draw resources own their GPU
  atlas. Replacing only source-chain state would leave stale derived resources.
- Actor loading still reads the Minecraft cow texture; the underwater effect
  reads a Minecraft texture; local landing sounds use Minecraft resource paths.
  First-party figure JSON is currently loose under repo `assets/`. A truthful
  global provenance claim therefore covers more than terrain textures.
- Web render/compiler sessions currently receive one packed byte payload and
  initialize resident worker catalog state from it. Native platforms discover
  filesystem/staged roots. Pack discovery is platform glue; selection and
  composition policy are shared behavior.

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
   effects, UI texture consumers, far-LOD palette/cache, and audio resolver as
   one logical commit.
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

- The shared catalog/selection/provenance contracts are not yet projected from
  platform discovery or wired into scene/UI state.
- Legacy reference/overlay builders do not emit the optional manifest
  provenance fields; their outputs remain `Unknown` until trusted discovery
  identifies them. The two new standalone first-party builders do emit them.
- The compatibility overlay remains partial; use the authored + generated
  standalone outputs for new first-party work.
- There is no first-party replacement/fallback visual catalog for missing
  blockstate/model JSON.
- Render compiler/catalog and GPU atlas replacement are startup-shaped.
- Web's resident compiler worker and single-byte-pack bootstrap need a
  replacement epoch/reinitialization path.
- First-party actor/effect/audio coverage and strict suppression are incomplete.
- Asset selection persistence has no shared cross-platform preference adapter.

## Recommended Next Work

Implement only tactical
[`169`](../tactical/169-runtime-asset-pack-selection.md) Slice 2 next. Add the
engine-native first-party visual adapter and prepare the authored + generated
stack into one CPU asset set with an exact resolution ledger. Prove the staged
packs produce a drawable offscreen first-party-only capture with zero
Minecraft/unknown resolutions before beginning transactional live replacement.

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

## Non-Goals

- No asset-pack hotkey in the initial product.
- No drag/drop pack ordering or arbitrary pack import in the first tactical.
- No server-mandated/downloaded pack protocol.
- No invocation of texture-lab authoring code from the runtime.
- No runtime or shader-time missing-texture synthesis.
- No claim that Hybrid Authoring is proprietary-free.
- No requirement that placeholder geometry visually match Minecraft models.
