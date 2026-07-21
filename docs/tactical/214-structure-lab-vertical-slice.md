# Tactical 214: Structure Lab Vertical Slice

Status: active 2026-07-21; implementation started from `6b1881de`.

Topic: `structure-lab`

Parent concern: [`../topics/structure-lab.md`](../topics/structure-lab.md).

## Instruction Synthesis

Implement Structure Lab end to end and commit reviewable slices while doing
so. Agents author structures through a checked-in TypeScript DSL. The DSL is
the only editable authority; canonical JSON is generated, and any disagreement
between source and JSON is an error. Rust consumes reparsed JSON for runtime
templates and static preview compilation. The public application is a polished
read-only TypeScript/React/Zustand/Three.js catalogue. In-browser editing is
indefinitely deferred.

Use the accepted standard cottage as the first parity and visual canary. Drive
technical decisions without waiting for user input, but capture and inspect
the first drawable mesh and the production catalogue on desktop and mobile.

## Shared Ownership

- `tools/structure-lab` owns the authoring DSL, source discovery, deterministic
  generation, drift gate, catalogue assembly, and read-only web application.
- `mclone-worldgen::structure_template` owns the host-neutral canonical JSON
  loader because it already owns `StructureTemplate`, transforms, markers,
  semantic material roles, and placement.
- A native build tool may depend on `mclone-assets` and `mclone-mesh` to load
  first-party block visuals and compile the same textured terrain geometry used
  by production. It must not duplicate block-model interpretation in
  TypeScript.
- `mclone-server::structure_lab` remains the temporary Rust-authored migration
  oracle until external members pass exact equality and visual gates.
- The website owns presentation only. It loads generated semantic facts and
  baked artifacts; it does not execute authoring TypeScript, resolve blocks, or
  initialize the game engine/WASM runtime.

## Slice Plan

### Slice 1 — canonical source and drift gate

- Scaffold a strict TypeScript package with Asset Lab-compatible tool versions.
- Define schema-v1 structure, palette, block, marker, socket, component,
  family, provenance, and promotion records.
- Implement `structure`, `palette`, `set`, exclusive-maximum `fillBox`, axis
  line helpers, component scopes, markers, and sockets.
- Validate safe IDs and paths, finite positive bounds, integer coordinates,
  palette references, duplicate writes/identifiers, marker bounds, and
  deterministic ordering.
- Discover `examples/*/structure.ts`, execute it on the build host, serialize,
  reparse, and expose only the parsed value.
- Add `structures:write` and `structures:check`. The check must fail for stale,
  missing, manually edited, duplicate, or orphaned promoted JSON.
- Externalize the accepted standard cottage into TypeScript and check its
  canonical JSON under `assets/mclone/structures`.

### Slice 2 — Rust loader and exact parity

- Add schema-v1 deserialization and validation beside `StructureTemplate`.
- Resolve semantic roles directly and exact namespaced block-state keys through
  a bounded, explicit block-state mapping.
- Reject malformed schema versions, unsafe IDs, unknown roles/states,
  duplicate or out-of-bounds positions, and invalid markers before building a
  template.
- Load the generated standard cottage through `include_str!` and prove exact
  equality with the current Rust constructor: ID, size, block positions and
  states, markers, transforms, bounds, touched chunks, and placement receipt.

### Slice 3 — Rust-baked preview artifact

- Add a native build-time compiler using the shared first-party visual
  catalogue and `mclone-mesh` block-model mesher.
- Resolve the selected semantic theme, mesh the structure with production
  culling, UV, tint, AO, and render-layer semantics, then bake fixed review
  lighting into vertex color.
- Emit a deterministic glTF/GLB proof, shared atlas, layer/component metadata,
  material counts, hashes, dimensions, bounds, markers, and provenance.
- Render it through a minimal Three.js page, capture the first drawable
  milestone, and compare its silhouette/material intent with the accepted
  native cottage evidence.

### Slice 4 — read-only catalogue

- Generate a checked catalogue manifest and deterministic thumbnails from
  canonical sources and baked artifacts.
- Build `/structures/` with React, Zustand, Vite, and one retained Three.js
  canvas.
- Provide search, family selection, runtime status, camera presets, dimensions,
  bill of materials, vertical build layers, component/marker/bounds overlays,
  URL restoration, responsive desktop/mobile layouts, theme handling,
  keyboard access, and explicit loading/error states.
- Add production-subpath Playwright coverage, one-canvas/resource-disposal
  checks, desktop/mobile captures, and bundle/artifact size receipts.

### Slice 5 — family migration and new content

- Externalize the accepted cottage and barn family members one at a time,
  retaining bounded named variants rather than arbitrary numeric dimensions.
- Make canonical generated JSON authoritative only after exact and visual gates
  pass; delete replaced Rust-authored bodies without changing the typed family
  selection API unnecessarily.
- Author one chicken coop or shed entirely through the TypeScript workflow to
  prove Structure-Lab-native content needs no Rust edits.

### Slice 6 — closeout

- Reconcile the topic and farmstead dependency ledger with exact landed
  evidence, commands, captures, artifact sizes, and remaining limits.
- Run the affected Rust, TypeScript, generated-data, browser, deployment, and
  public-provenance gates.
- Keep terrain placement, structure starts/references/pieces, settlement
  composition, entities, and browser editing explicitly outside this tactical.

## Required Gates

```bash
pnpm structure-lab:typecheck
pnpm structure-lab:test
pnpm structure-lab:structures:check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen structure_template
cargo test --manifest-path native/Cargo.toml -p mclone-server structure_lab
cargo test --manifest-path native/Cargo.toml -p mclone-structure-compiler
pnpm structure-lab:web:build
pnpm structure-lab:web:test
pnpm host:check -- --probe-browser-webgpu
```

Every pixel-producing milestone must save captures outside the repository and
be inspected before the next visual slice. The public build must contain no
authoring TypeScript, Minecraft-reference texture payload, or unknown asset
provenance.

## Stop Conditions

Stop and record evidence rather than hiding a divergence if:

- the TypeScript member cannot reproduce the accepted Rust template exactly;
- canonical JSON can reach any downstream consumer without serialization and
  reparsing;
- the preview requires a second TypeScript block-model or lighting engine;
- the first-party pack cannot faithfully represent a required current block;
- glTF grouping or size fails the measured proof and a custom container would
  materially change the public contract; or
- a visual capture is black, transparent, materially corrupt, or obtained only
  through the known-invalid Linux headless WebGPU lane.

## Progress Evidence

### Slices 1–3 and catalogue drawable milestone

Implemented and inspected on 2026-07-21:

```text
TypeScript discovery, semantic validation, write/check drift gate
  5 tests passed; promoted standard cottage current
Rust JSON loader and accepted-template equality canary
  native and wasm checks passed
Rust preview compiler
  923 blocks; 3,539 faces; 14,156 vertices; 21,234 indices
  599,400-byte GLB; 1024x256 atlas with 143 sprites
Public preview provenance
  first-party=18; generated=126; Minecraft-reference=0; unknown=0
Structure Lab web unit/browser suites
  5 semantic tests and 2 Playwright tests passed
```

The first drawable screenshot exposed conspicuous generated-fallback material
tiles. Ten original farmstead runtime textures were therefore authored through
Texture Lab, packed through the ordinary first-party pipeline, and rebaked by
Rust before catalogue acceptance. Their block review sheets and the complete
desktop, sliced dark-mode desktop, and 390-pixel mobile catalogue captures were
inspected under `/tmp`. The mobile proof also caught and fixed absent URL layer
state incorrectly coercing to layer zero.
