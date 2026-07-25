# Texture Material Profiles

Topic: texture-material-profiles

Status: accepted for end-to-end implementation 2026-07-25. The product model
and profile semantics below are settled; implementation evidence will be
recorded here as the shared engine, Terrain Lab, and Texture Lab adopt them.

Implementation update (2026-07-25): the first shared slice has landed. The
legacy-id `mclone-generated-fallback` archive now declares
`first_party_provisional` provenance and contains label-free deterministic
material art. A separate `mclone-diagnostic-missing` archive owns the checker,
magenta border, short codes, and missing registry. `mclone-assets` defines the
five named source orders plus textured/flat presentation vocabulary and exact
provisional/diagnostic provenance. The canonical build stages all three
first-party archives and still succeeds without reading Minecraft content.

Implementation update (2026-07-25): the shared runtime slice is implemented
across native, Android, and browser hosts. Asset preparation accepts a named
profile plus presentation, persists both fields, and sends all four source
archives plus the selected contract to browser render workers. The in-game
screen is now **Visual Profiles**: one row selects one complete source order,
and a separate control cycles Textured or Flat Colors. First-party Coverage
forces Textured so its numbered diagnostics stay legible. Flat Colors is
derived from alpha-weighted sprite averages while preserving transparent and
cutout pixels.

Native rendered evidence in `/tmp/mclone-profile-original-textured.png`,
`/tmp/mclone-profile-original-flat.png`, and
`/tmp/mclone-profile-coverage.png` confirms the three visibly distinct
presentations. `/tmp/mclone-visual-profiles-ui.png` confirms the compact
five-profile UI. The headed Wayland browser asset-replacement probe also
passes with four worker-loaded sources, persists Mclone Original, reloads it
at asset epoch 1, and records its screenshot and receipt under
`/tmp/mclone-native-web-asset-pack-ui-probe*`.

Implementation update (2026-07-25): Terrain Lab now consumes the same shared
profile resolver in both its exact-chunk and CPU/GPU LOD renderers. URL state
separately records terrain generation (`profile`), visual materials (`visual`),
texture representation (`texture`), and the optional exact comparison
(`compareVisual`). All normal panes share the primary visual selection.
Enabling comparison creates a second exact pane with shared seed, center,
scale, camera, navigation, checkpoint, and visibility.

The local Minecraft archive is an optional, read-only fourth source. Terrain
Lab disables its dependent menu entries when absent and renders an explicit
unavailable surface for a stale shared URL instead of changing profile
semantics. Headed Wayland evidence at
`/tmp/mclone-terrain-lab-desktop-chrome-material-profiles-ui.png` confirms the
synchronized Mclone Original/Minecraft Reference view;
`/tmp/mclone-terrain-lab-desktop-chrome-flat-colors.png` confirms the derived
flat representation. The focused Playwright contract also asserts the two
source renders differ and Coverage Debug forces textured presentation.

Implementation update (2026-07-25): Texture Lab now has one committed
Candidate/Provisional/Curated lifecycle manifest. The primary UI replaces the
old Status and Queue filters plus Select/Apply/Freeze action cluster with one
Lifecycle filter and **Use as Provisional**, **Accept as Curated**, and
**Return to Candidate**. Its material comparison grid keeps Minecraft
Reference explicitly local and read-only.

Promotions validate image dimensions and tint-source policy, write a
pack-local provisional or frozen PNG, and commit an explicit canonical runtime
material binding. Exact inventory matches may be suggested, while ambiguous
textures remain unpromotable. The initial accepted mapping makes the important
non-filename relationship `grass_block_top` ->
`mclone:block/grass_block` visible. Historical custom Far LOD tiles are
excluded from the active lifecycle. Browser promotion tests operate on an
isolated copy of the source pack and all ten headed Chrome tests pass.

Implementation update (2026-07-25): first-party pack construction now consumes
only lifecycle distribution roots. A full Texture Lab runtime-compatible
export writes accepted canonical ids into
`lifecycle-curated-root` or `lifecycle-provisional-root`; the broad authoring
and historical LOD export remains a review artifact and no longer enters the
authored pack.

The resulting authored archive has 46 payload files: the two actually curated
canonical textures (`grass_block` and `stone`) plus 44 repository-owned
non-texture assets. It contains no Minecraft block PNG and no static custom
Far LOD palette. The provisional archive still covers all canonical materials
with deterministic generated art and overlays any lifecycle-provisional
textures. The three-pack build, six deterministic pack tests, and strict
first-party preparation pass; preparation reports the two accepted materials
as first-party and the other 143 requested texture assets as provisional.

## Scope

This topic owns the meaning, selection, and authoring lifecycle of block
textures across the runtime and terrain tools. It covers:

- curated and provisional first-party textures;
- real local Minecraft 1.17.1 textures used for parity and comparison;
- conspicuous numbered missing-texture diagnostics;
- textured versus flat-color presentation;
- automatic mipmaps and Far LOD material summaries;
- the shared named profiles exposed by the engine and Terrain Lab; and
- the simplified promotion model exposed by Texture Lab.

[`asset-pack-profiles.md`](asset-pack-profiles.md) continues to own pack
discovery, composition, provenance, persistence, and transactional runtime
replacement. Texture generation mechanics remain documented by Tactical
[`114`](../tactical/114-ai-texture-pack-lab.md), while Tactical
[`115`](../tactical/115-first-party-texture-pack-integration.md) records the
original first-party overlay bridge.

## Product Decision

There are three content sources, one diagnostic visualization, and several
derived representations. They must not be presented as a flat list of
interchangeable texture packs.

### Content sources

1. **Curated Mclone** is manually accepted, distributable first-party art.
2. **Provisional Mclone** is deterministic, distributable first-party art that
   is usable but has not been manually accepted. It may be procedurally
   generated from first-party recipes or from non-copyrightable measurements
   recorded during authoring, but it must build without the Minecraft
   reference tree.
3. **Minecraft Reference** is the real local Minecraft Java 1.17.1 art. It is a
   private, non-distributable comparison/parity source and is never silently
   copied into a first-party pack.

### Diagnostic visualization

**Numbered Missing** is the deliberately ugly checker, border, and short-code
rendering used to expose coverage and routing mistakes. It is not provisional
art, not a product fallback, and not a normal player-facing source.

### Derived representations

- Ordinary textured rendering automatically builds and samples mip chains.
- Flat-color rendering derives representative colors from the resolved source
  texture.
- Far LOD derives its material summary from the resolved source texture and
  may add stable aggregate facts such as opacity or variation.

Mipmaps, flat colors, and Far LOD are therefore representations of a selected
source, not separately curated texture families. A custom hand-authored “LOD
texture” is not part of the normal material lifecycle.

## Named Visual Profiles

The shared contract exposes named profiles rather than independent source
toggles. Their resolution order is:

| Profile | Resolution order | Intended use |
|---|---|---|
| **Mclone Original** | curated, provisional | Default distributable game presentation |
| **Minecraft Reference** | Minecraft, provisional | Vanilla parity and direct comparison |
| **Hybrid Authoring** | curated, Minecraft, provisional | See accepted Mclone work over a complete vanilla baseline |
| **First-party Coverage** | curated, numbered missing | Expose every canonical material that still lacks curated first-party art |
| **Provisional Audit** | provisional | Review the generated baseline without curated art hiding it |

Rules:

- **Mclone Original** is the default when only first-party packs are present.
  It must look coherent enough to play without an opt-in diagnostic mode.
- **Minecraft Reference** may resolve engine-only materials from provisional
  Mclone, but the UI must identify that fallback and must not claim pure parity.
- **Hybrid Authoring** is explicitly proprietary/local while Minecraft content
  resolves.
- **First-party Coverage** intentionally ignores Minecraft and provisional art.
  It is the principal home of the numbered diagnostic texture.
- **Provisional Audit** ignores curated and Minecraft art so the provisional
  baseline can be inspected directly.
- Unavailable Minecraft content leaves the Minecraft-dependent profiles
  unavailable; it must not silently change their meaning.

The ordinary in-game Visual Profiles screen presents all five complete source
orders in one compact list, with the two audit profiles visibly named as such.
Applying any profile continues to use the existing transactional asset-epoch
replacement.

## Presentation Detail

A second, orthogonal setting selects:

- **Textured**: use the resolved texture and its automatic mip chain.
- **Flat colors**: derive and render a representative color from that same
  resolved texture.

This setting does not alter source precedence or provenance. For example,
Minecraft Reference plus Flat colors means colors derived from the real local
Minecraft textures. First-party Coverage forces Textured so its numbered
diagnostics cannot be averaged into plausible-looking colors.

The first implementation uses the existing representative-color material
pipeline. It should preserve alpha/cutout semantics where needed and must not
turn missing diagnostics into plausible product art.

## Shared Contract

The shared asset/runtime owner defines stable profile and presentation enums.
All consumers construct their source chain through the same resolver instead
of independently pushing authored, Minecraft, or generated packs.

Every resolved canonical material records:

- canonical material id;
- selected visual profile and presentation detail;
- source family: curated, provisional, Minecraft, or diagnostic;
- concrete pack/source provenance;
- whether a lower-priority fallback was required; and
- the derived representative color used by flat and Far LOD paths.

The current first-party pack split is refined as follows:

- the authored pack contains only curated runtime materials;
- the provisional pack contains coherent deterministic first-party materials;
- the diagnostic pack or diagnostic-generation path contains the numbered
  missing textures;
- the local Minecraft pack remains separately discovered and never enters
  first-party output.

Generated registries and strict validation must distinguish provisional from
diagnostic assets. A successful distributable Mclone Original validation may
contain curated and provisional provenance, but no Minecraft, unknown, or
diagnostic provenance. Coverage validation expects curated and diagnostic
provenance by definition.

## Texture Lab Lifecycle

Texture Lab becomes a canonical runtime-material dashboard rather than a
generic pack toggling tool. Each canonical material has one first-party
lifecycle state:

1. **Candidate**: a work-in-progress recipe or render, not runtime eligible.
2. **Provisional**: deterministic, validated, and allowed in the default
   first-party profile.
3. **Curated**: manually accepted first-party art that shadows provisional.

Minecraft Reference is a separate read-only comparison pane, not a lifecycle
state. Numbered Missing is a diagnostic preview, not a promotable candidate.

The primary material page shows:

- canonical runtime id and affected block states/faces;
- candidate, provisional, and curated previews where present;
- optional real Minecraft reference preview with local-only labeling;
- representative flat color and mip preview derived from each source; and
- explicit **Use as Provisional** and **Accept as Curated** actions.

Legacy overlapping concepts such as draft/reviewed/accepted tags, art-source
kind, “Select for Pack,” and “Apply Pack” should converge on this lifecycle.
Pack building consumes the committed lifecycle manifest; it is not a separate
curation state.

A texture can be demoted or replaced deliberately, but promotion must remain a
reviewable manifest change. Procedural generation must be deterministic, and a
provisional first-party build must not require Minecraft artifacts.

## Terrain Lab

Terrain Lab exposes the same visual-profile and presentation-detail selectors
as the runtime and stores them in shareable URL state.

For parity work it also offers a synchronized comparison view:

- comparison is off by default;
- enabling Minecraft Reference adds it beside the primary Mclone Original
  pane;
- camera, world seed, generator profile, and inspected position are shared;
- either pane may choose any available visual profile; and
- Minecraft-dependent views clearly report when the local reference pack is
  unavailable.

The Terrain Lab must not own a parallel meaning for “original,” “vanilla,”
fallback order, or flat color. Its native/Wasm asset preparation calls the
shared profile resolver.

## Current Truth and Migration

Before this implementation:

- `mclone-generated-fallback.pbp` contains the numbered checker/code textures
  and is always last in the runtime chain.
- Mclone Original therefore resolves uncurated materials to diagnostic art.
- The first-party runtime catalogue requests canonical single-material ids
  such as `mclone:block/grass_block`, while Texture Lab has accepted
  face-specific grass assets such as `grass_block_top`; accepting that texture
  again cannot fix the canonical runtime miss by itself.
- ordinary chunk textures already receive automatic mip chains in
  `mclone-render`;
- Far LOD consumes representative colors, while Texture Lab also emits a
  historical hand-defined LOD texture family; and
- engine, Terrain Lab native/Wasm entry points, and Texture Lab use overlapping
  but not identical selection terminology.

Migration must preserve useful accepted art without declaring all existing
procedural output curated. Existing accepted assets map to Curated after their
canonical runtime mapping is verified. Existing deterministic authored
recipes that are runtime-appropriate map to Provisional. Unmapped or invalid
materials receive a generated provisional baseline in Mclone Original and the
numbered diagnostic only in Coverage.

Grass, logs, and other face-dependent blocks need explicit canonical material
semantics rather than file-name coincidence. The first implementation may keep
the current single-material runtime contract where required, but must make the
chosen canonical mapping visible in Texture Lab and must not falsely report a
face-specific asset as active.

## Acceptance Invariants

- A fresh first-party-only Mclone Original session has no numbered diagnostic
  textures.
- First-party Coverage makes every uncurated canonical material unmistakable.
- Minecraft Reference uses real local 1.17.1 textures and reports
  Minecraft-reference provenance.
- Hybrid Authoring makes curated replacements easy to compare against the
  Minecraft baseline.
- Provisional Audit cannot accidentally resolve curated or Minecraft art.
- Engine and Terrain Lab resolve the same canonical material to the same source
  family for an equivalent profile.
- Textured, flat-color, ordinary mip, and Far LOD representations agree on the
  selected source.
- First-party packs build and validate with the Minecraft reference tree
  absent.
- No distributable artifact contains Minecraft texture bytes.
- Profile application remains transactional and failure retains the previous
  asset epoch.

## Implementation Sequence

1. Split provisional and numbered diagnostic generation and add shared profile
   and presentation contracts.
2. Adopt named profiles in runtime preparation, persistence, provenance, and
   the in-game UI.
3. Adopt the same URL-backed controls and synchronized comparison in Terrain
   Lab. **Completed 2026-07-25.**
4. Replace Texture Lab's overlapping pack/status controls with the canonical
   material lifecycle. **Completed 2026-07-25.**
5. Derive flat colors and Far LOD summaries from resolved textures, retain
   automatic mip generation, and remove the separate LOD-texture assumption
   from the active path.
6. Validate deterministic first-party builds, shared native/Wasm behavior, and
   rendered output for Original, Minecraft, Hybrid, Coverage, and Provisional
   Audit.
