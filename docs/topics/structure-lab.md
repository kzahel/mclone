# Structure Lab

Topic: `structure-lab`

Status: **vertical slice active 2026-07-21. The TypeScript DSL and generated
JSON drift gate, strict Rust loader, exact cottage/barn family parity canaries,
Rust preview compiler, original first-party farmstead materials, and read-only
Three.js catalogue are implemented. Runtime promotion and the first
Structure-Lab-native outbuilding remain. In-browser block editing is
indefinitely deferred.**

Last reconciled: **2026-07-21**.

## Current Implementation

- `tools/structure-lab/examples/cottage_standard/structure.ts` is the sole
  editable source for the promoted standard cottage. Its generated JSON is
  checked byte-for-byte under `assets/mclone/structures/`.
- `mclone-worldgen::structure_json` reparses and strictly validates canonical
  records. The current canary matches the accepted Rust template exactly.
- `mclone-structure-compiler` meshes canonical records through the production
  first-party visual catalogue and terrain mesher into deterministic GLB,
  shared-atlas, and receipt artifacts.
- The initial cottage artifact contains 923 placed blocks, 3,539 visible
  faces, 14,156 vertices, 21,234 indices, 15 build layers, and seven named
  component groups. The 599,400-byte GLB references one 1024x256 shared atlas.
- The public receipt resolves 18 first-party and 126 generated asset paths,
  one optional missing path, and zero Minecraft-reference or unknown paths.
- Twelve finite family members are externalized: all six cottage depth/entry
  combinations plus short, standard, and long barn cores and lean-tos. Every
  generated template matches its accepted Rust constructor exactly.
- Shared-helper provenance hashes the complete local TypeScript import graph,
  so editing family vocabulary or DSL behavior invalidates generated JSON as
  reliably as editing a leaf `structure.ts` file.
- The catalogue shell uses React, Zustand, Vite, Three.js, one retained canvas,
  stable URL state, build-layer clipping, component/marker/bounds controls,
  material and provenance facts, and responsive light/dark layouts.
- The original farmstead texture recipes cover plaster, cobblestone, mossy
  stone, dressed stone, oak and spruce logs and planks, brick, painted red
  clay, bound hay, flowers, and wall torch.
  Texture Lab review sheets and full desktop/mobile catalogue captures were
  inspected before accepting the presentation baseline.

## Scope

This topic owns the proposed **Mclone Structure Lab**: the source-authoring
contract for reusable authored structures, initially buildings; its canonical
generated records; build-time preview compilation; the public structure
catalogue and build-guide experience; promotion into runtime structure
content; and the validation boundary joining those pieces.

It does not own terrain site selection, grading, structure starts/references,
settlement composition, resident spawning, or world persistence. Those remain
with [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md),
[`../structures.md`](../structures.md), and their implementation tacticals.

## Naming Decision

Use **Structure Lab** for the tool and public product. It matches the engine
and file-format noun while retaining the established Asset Lab relationship:

- `tools/structure-lab/` — proposed authoring and catalogue package;
- `/structures/` — proposed public route;
- `StructureTemplate` — engine object;
- `*.structure.json` — generated canonical semantic record; and
- structure palette, block, marker, socket, transform, and placement receipts.

The existing `mclone-server::structure_lab` module and persisted galleries from
Tacticals 208–210 are the current proof environment, not the website
implementation. The live module and Rust API were renamed from the historical
`building_lab` vocabulary when this topic was accepted; completed tactical
filenames remain execution records. The galleries may become migration
fixtures without making their Rust-authored bodies permanent.

## Product Decision

Structure Lab should be a high-quality, read-only structure catalogue and
build guide with the same product care as the deployed Asset Lab animal
catalogue. Its normal authoring workflow is agents editing checked-in
TypeScript DSL sources. Visitors browse generated structures and recipes; they
do not mutate blocks in the browser.

The web application should use TypeScript, React, Zustand, Vite, Three.js, and
the proven catalogue interaction patterns where they fit. Sharing in the
initial product means stable URLs for a structure, named family member,
palette, camera, and guide layer. It does not mean accepting arbitrary public
uploads.

An in-browser structure editor, live WASM remeshing, user accounts, public
uploads, collaborative editing, and hosted user-content persistence are
**indefinitely deferred**. They may be reconsidered only after the source-first
agent workflow and public catalogue demonstrate a concrete need. The
architecture must not require them, reserve UI for them, or describe them as
the automatic next phase.

## Authoritative Source And Generated-JSON Contract

A canonical structure has exactly one human- or AI-authored source:

```text
tools/structure-lab/examples/<structure>/structure.ts
```

That source uses a typed DSL and is the only editable authority. Canonical JSON
is generated output. It must never become an independent authoring surface.

```text
structure.ts
  -> execute typed DSL on the build host
  -> serialize canonical semantic JSON
  -> reparse and validate the serialized bytes
  -> expose only the reparsed value to every downstream consumer
```

The following invariants are binding:

- The browser never imports or executes `structure.ts`.
- Rust never imports TypeScript or relies on TypeScript-only semantics.
- Preview, catalogue, mesh baking, runtime promotion, tests, and native
  comparisons consume reparsed canonical JSON, never the live DSL object.
- Every checked runtime `*.structure.json` declares one canonical TypeScript
  source through a checked first-party mapping.
- A `structures:check`-style gate regenerates expected canonical bytes and fails
  on stale, missing, manually edited, duplicate, or orphaned JSON.
- A `structures:write`-style command is the only normal way to refresh checked
  generated JSON. Review includes both the source diff and generated diff.
- Updating provenance fields inside JSON cannot hide a direct edit: exact
  regenerated-byte comparison is authoritative.
- Lab-only examples may generate JSON directly into ignored or deployment
  output. The build always replaces that output from TypeScript; it never uses
  an adjacent hand-edited JSON file as source.
- Source discovery defines the catalogue inventory. A prose list is not a
  second registry.
- Output order, palette order, block order, markers, and hashes are
  deterministic across clean builds.

Generated JSON should carry enough provenance to diagnose drift without being
trusted as proof of its own correctness: schema version, stable structure id,
source-relative path, source hash, generator/compiler identity, and semantic
hash. The external drift gate recomputes all of them.

This contract deliberately repeats the strongest Asset Lab lesson: generated
semantic JSON can be checked and promoted, but editing it directly is always
an error.

## DSL And Canonical Record

The TypeScript DSL should be concise for agent authoring while compiling to a
simple, vanilla-shaped resolved record. Initial source verbs are:

- `structure`, `palette`, and named metadata;
- `set`, `fillBox`, `lineX`, `lineY`, and `lineZ`;
- semantic material roles and exact block-state keys;
- `marker` and typed attachment/socket helpers;
- named component/module boundaries such as porch or lean-to; and
- ordinary TypeScript helpers and loops that resolve completely during the
  build.

Coordinates and sizes are integer block-grid values. Boxes use one documented
exclusive-maximum convention matching the Rust template builder. The DSL must
reject out-of-bounds writes, duplicate incompatible identifiers, unresolved
materials, unsafe paths, and markers outside declared bounds.

Canonical JSON should not preserve an executable expression language. It is a
resolved semantic snapshot containing at least:

- schema version, id, display metadata, size, and bounds;
- a deterministic palette of semantic roles and/or exact namespaced block
  states;
- palette-indexed local block positions in deterministic order;
- typed markers and sockets;
- named component membership needed for review and build guides;
- family/variant identity and compatibility facts; and
- source and compiler provenance.

The initial family model remains bounded. `snug`, `standard`, and `deep` are
authored cottage plans; `short`, `standard`, and `long` are authored barn
plans. A raw `depth: 137` control is not introduced merely because the source
is TypeScript. Named variants may share helpers, but every exported canonical
member is finite, deterministic, independently reviewable, and given honest
bounds.

## Dual Artifact Pipeline

Canonical JSON is the source boundary for both runtime structure content and
static website presentation. A Rust build-time compiler should consume it and
produce derived catalogue artifacts using shared engine code:

```text
structure.ts
  -> canonical *.structure.json
       -> Rust StructureTemplate loader
            -> runtime/promoted structure record
            -> baked preview mesh
            -> material/layer/marker metadata
            -> deterministic thumbnails and review receipts
```

The public catalogue should normally load the baked mesh, not initialize the
game engine or compile blocks in the visitor's browser. JSON parsing is cheap;
the important avoided work is WASM initialization, block-model resolution,
atlas construction, lighting, face culling, and mesh generation.

The first mesh interchange candidate is glTF/GLB because Three.js loads it
well and it can carry positions, UVs, normals, indices, vertex colors,
materials, and named groups. This choice remains subject to a measured proof;
a compact engine-specific binary is allowed later if glTF materially harms
size, fidelity, or layer interaction.

Rust build-time compilation should:

- use the same block visual catalogue/model resolution and mesh semantics as
  the engine rather than implementing Minecraft blocks in TypeScript;
- use distributable first-party Mclone textures for public artifacts;
- compile a fixed, documented preview environment and midday light state;
- bake final preview illumination/AO into vertex data where practical;
- split solid, cutout, and translucent primitives honestly;
- preserve named vertical layers and components as independently hideable mesh
  groups;
- emit dimensions, bounds, material counts, markers, source hash, compiler
  version, atlas hash, and mesh hash in a manifest; and
- be deterministic or explicitly account for any non-byte-deterministic
  container metadata.

The preview mesh is never authoritative runtime structure data. It cannot
replace blocks needed for collision, edits, persistence, processors, entities,
or terrain placement. It is disposable generated presentation output.

## Texture And Provenance Policy

Public Structure Lab output must not distribute extracted Mojang textures or
unknown-provenance assets. The normal public compiler uses Mclone Authored and
Generated Fallback inputs and records the resolved provenance ledger. A public
build fails if any preview resolves a Minecraft-reference or unknown source.

A local developer comparison may opt into an installed Minecraft reference
pack when permitted by the existing asset-pack policy, but that artifact is
diagnostic, remains outside the deployment tree, and is visibly labeled as
non-distributable.

Prefer a content-hashed shared atlas across catalogue entries rather than
embedding duplicate textures into every mesh, provided the glTF/Three.js proof
can bind it without inventing a second material interpretation. Per-building
textures are a fallback, not the assumed steady state.

## Read-Only Website Contract

The proposed `/structures/` application should provide:

- searchable structure and family navigation;
- deterministic lazy thumbnails and one retained live Three.js canvas;
- orbit, pan, zoom, reset framing, and useful camera presets;
- named variant, component, rotation, mirror, and supported palette controls;
- dimensions, block totals, material bill of quantities, and provenance;
- a vertical layer slider and optional exploded/layer-isolation presentation;
- marker, entrance, socket, bounds, and component overlays;
- concise recipe/build-guide presentation derived from canonical facts;
- shareable URL state for selection and supported presentation choices;
- responsive desktop/mobile layout, keyboard access, light/dark presentation,
  and explicit loading/error states; and
- clear runtime-promoted versus Structure-Lab-only status derived from the
  checked promotion mapping.

React owns presentation, routing, and component composition. Zustand owns the
small serializable catalogue/selection/presentation state. Three.js owns only
display of baked artifacts, camera interaction, visibility groups, and GPU
resource disposal. Neither React nor Zustand reconstructs blocks, evaluates
the DSL, calculates material counts, or interprets structure placement rules.

Catalogue rows use static thumbnails and lazy loading. They must not allocate
one WebGL/WebGPU context per card. The selected mesh and its shared atlas are
loaded on demand and disposed or cached under an explicit bounded policy.

The likely deployment shape mirrors `/animals/`: Vite produces the exact
`/structures/` subtree, the aggregate native-web bundle stages it, and the
existing worker/deploy path serves it with correct subpath URLs and cache
headers. A separate site, bucket, deployment worker, or authentication system
is not justified initially.

## Runtime And Farmstead Relationship

Tacticals 208–210 currently define the accepted cottage and barn families in
Rust and prove ordinary persisted galleries. They are migration oracles for
Structure Lab, not content to discard immediately.

Migration should proceed member by member:

1. Generate a canonical external member from TypeScript.
2. Reparse it through the Rust loader.
3. Prove exact template id, size, blocks, markers, and placed receipt equality
   against the current Rust constructor.
4. Compare baked Three.js and production native renders.
5. Make the TypeScript source authoritative and the checked JSON promoted.
6. Remove the replaced Rust-authored body only after the drift, loader, and
   visual gates are green.

The family selection API can continue exposing typed Rust enums while loading
the selected canonical member. The public family manifest maps named members
and optional modules; it does not make website state part of generation policy.

Structure Lab enables content authoring but does not satisfy `FS-04` structure
starts/references/pieces or the later farmstead overlay/site-plan work. Its
records should feed those systems without owning them.

## Explicit Deferrals

- Browser-side block placement, deletion, paint tools, or drag handles.
- A WASM engine/editor bundle in the public read-only catalogue.
- Dynamic browser lighting, block-model resolution, or remeshing.
- User accounts, uploads, moderation, collaborative editing, or hosted saves.
- Trusting user-supplied baked meshes or thumbnails.
- An unconstrained procedural architecture grammar.
- Full vanilla NBT import/export. The canonical record should keep this
  feasible, but it follows the native record and drift proof.
- Terrain grading, settlement planning, entities, machinery, and simulation.

These are not hidden requirements for the first website and should not inflate
its bundle or architecture.

## Validation Contract

### Source and semantic data

- Strict TypeScript with `noUncheckedIndexedAccess` and
  `exactOptionalPropertyTypes`.
- Discovery tests execute every canonical source and cross the
  serialize/reparse boundary.
- Exact regeneration checks reject direct JSON edits, stale/missing outputs,
  orphaned promoted JSON, duplicate ids/paths, and output outside allowed
  roots.
- Two clean generation runs produce byte-identical semantic JSON, manifests,
  hashes, and any formats that claim determinism.
- Rust rejects malformed schemas, out-of-bounds positions, unresolved
  palettes, duplicate positions where forbidden, invalid markers, and unsafe
  sizes before allocating unbounded volumes.

### Engine and baked artifacts

- The first accepted cottage and barn migrations prove exact equality with
  their Tactical 210 templates and placement receipts.
- Rotation, mirroring, semantic themes, partial blocks, glass, bounds, markers,
  and touched chunks retain focused parity tests.
- Mesh receipts bind structure, compiler, asset selection, atlas, lighting
  environment, group inventory, vertex/index counts, and output hashes.
- Material totals and layer groups derive from canonical blocks and reconcile
  exactly across the JSON manifest and displayed UI.
- Production native and baked Three.js captures are compared from matching
  camera/environment contracts. Pixel identity is not presumed across
  renderers, but visible geometry, materials, transparency, lighting intent,
  and silhouette must agree.

### Website and deployment

- Production-subpath Playwright tests cover load, selection, search, URL
  restoration, camera interaction, variant/component/layer controls, material
  facts, keyboard navigation, mobile layout, one-canvas ownership, replacement,
  disposal, failed requests, console errors, and page errors.
- Desktop and mobile captures are inspected at the first drawable milestone
  and after meaningful UI expansion.
- The deployment bundle contains only generated catalogue data, baked meshes,
  shared distributable textures, thumbnails, and hashed frontend assets—never
  executable authoring TypeScript or reference-pack payloads.
- The public provenance gate reports zero Minecraft-reference and unknown
  resolutions.
- Initial-load, selected-mesh, shared-atlas, JavaScript gzip, and GPU resource
  sizes are recorded before adding more catalogue complexity.

## Implementation Direction

No tactical number is reserved by this topic. The recommended sequence is:

1. **Canonical-source proof:** scaffold `tools/structure-lab`, define the typed
   DSL and schema-v1 JSON, implement `write` and mandatory `check` commands,
   and externalize one accepted cottage with exact Rust-loader equality.
2. **Baked-mesh proof:** compile that JSON through Rust engine asset/mesh/light
   code into one measured glTF/GLB candidate and compare it with the native
   gallery render.
3. **Catalogue foundation:** build the polished React/Zustand/Three.js shell,
   generated manifest, thumbnails, production `/structures/` subpath, and
   desktop/mobile visual acceptance around the one proven artifact.
4. **Guide and family breadth:** add the cottage and barn families, component
   toggles, layer slicing, material bills, markers, promotion status, and URL
   sharing without duplicating semantic interpretation in the frontend.
5. **Runtime promotion:** move accepted members from Rust-authored bodies to
   checked generated JSON one at a time, retaining exact and visual regression
   gates.
6. **First new Lab-native building:** author and review a chicken coop, shed,
   or similarly bounded outbuilding entirely through the source-first pipeline.

The first implementation tactical should remain narrow enough to reject the
architecture cheaply: one cottage, one canonical record, one Rust load/equality
gate, and one baked static mesh. A general editor and the complete catalogue
are not acceptance conditions for that proof.

## Open Decisions For The First Proof

- Whether glTF with an external shared atlas is sufficient, or a compact
  custom mesh container is measurably preferable.
- Whether fixed preview lighting should be baked directly into vertex color or
  preserve compact block/sky-light channels for a tiny Three.js shader.
- The smallest shared engine boundary for build-time structure mesh
  compilation without depending on a live server/session.
- Whether named layer/component groups fit glTF cleanly without excessive
  primitive count or draw overhead.
- Which accepted cottage member is the best migration canary; the standard
  member has the strongest visual baseline, while the snug stoop is smaller.

Resolve these through the first vertical proof rather than speculative generic
infrastructure.

## Code And Documentation Map

- Asset Lab source/generated/display precedent:
  [`../../tools/asset-lab/README.md`](../../tools/asset-lab/README.md)
- Deployed catalogue product precedent:
  [`animal-catalogue.md`](animal-catalogue.md)
- Current building and family implementation:
  [`../../native/crates/mclone-server/src/structure_lab.rs`](../../native/crates/mclone-server/src/structure_lab.rs)
- Current structure-template kernel:
  [`../../native/crates/mclone-worldgen/src/structure_template.rs`](../../native/crates/mclone-worldgen/src/structure_template.rs)
- Farmstead vision and integration sequence:
  [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- Structure lifecycle architecture:
  [`../structures.md`](../structures.md)
- Asset selection and public provenance:
  [`asset-pack-profiles.md`](asset-pack-profiles.md)
- Current native-web deployment contract:
  [`../native-web.md`](../native-web.md)
