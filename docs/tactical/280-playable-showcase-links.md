# Tactical 280: Playable Showcase Links

Status: **complete 2026-08-12**

Topic:

- `playable-showcases`
- `habitat-driven-creature-ecology`

## Instruction Synthesis

Make visual feature reviews pair an inspected screenshot with a deployed Mclone
Web URL that opens the same seed and location as a temporary playable world.
For stateful examples such as the mallard family, use a small readable data
recipe that behaves like a resettable tiny save game on the viewer's device.
Keep the framework bounded, document how to add showcases, prevent showcase
behavior from leaking into the product runtime, and record whether every
showcased fact has a real live-game instantiation path.

## Decision

Add one shared typed showcase compiler in `mclone-server`. Checked-in JSON owns
composition only: base terrain, entry pose, world presentation defaults,
block patches, persisted entities and relationships, player inventory, and
durable observations. Rust owns schema validation, stable identity derivation,
persistence records, and the allowlist of real entity/item/observation kinds.

A showcase is never a gameplay mode or scripting language. It may establish
only state that the ordinary persistence layer can hydrate. Once started, its
integrated server, client replication, movement, interaction, saving policy,
and rendering are the ordinary game systems. Browser launch always uses a
fresh in-memory store; reload discards the session and recompiles the recipe.

Each non-terrain datum must name a `liveInstantiation` identifier registered by
shared code. The registry describes the ordinary gameplay producer and points
to an enforced test/contract label. Recipe validation fails for unknown or
incompatible evidence. This proves that a showcase demonstrates shipped
mechanics rather than becoming the only way content can exist. It does not
pretend to prove the feature's balance or natural frequency; those remain live
world tests and review concerns.

## Scope

- Add a versioned, deny-unknown-fields showcase schema and the first
  `mallard-ecology` JSON recipe.
- Compile a recipe through the existing authored-terrain builders and ordinary
  `WorldStore` records, on native and Wasm.
- Support symbolic entity IDs and validated parent references without exposing
  raw persistent IDs in content data.
- Support a bounded set of player inventory and mallard observation values.
- Expose a shared catalogue lookup by stable showcase ID.
- Add `?showcase=mallard-ecology` Web launch. It implies direct entry and
  transient storage and rejects persistent storage, remote destinations, and
  conflicting seed/profile/camera overrides.
- Route the Web Worker to the same shared compiler already used by native.
- Replace the bespoke mallard acceptance authoring with the shared recipe.
- Add a deterministic browser capture and verify it against the native capture
  using recipe identity, seed, entry pose, entity kinds/counts, guide progress,
  and inspected pixels.
- Push the completed commits, wait for the repository deploy hook, and verify
  the public URL before sharing it.
- Update topic docs, Web docs, and `AGENTS.md` only after the implementation and
  deployment contract have passed.

## Guardrails

- No update loops, triggers, arbitrary commands, scripts, timers, goals,
  dialogue, or platform paths in showcase JSON.
- No showcase-specific branches in entity ticking, gameplay interaction,
  rendering, protocol, or UI.
- No entity/item/observation kind may be introduced only for a showcase.
- A recipe cannot use raw block palettes as a substitute for landing a real
  generator/structure/feature contract; base terrain and bounded review patches
  must be explicit.
- A live-instantiation registry entry must exist before recipe validation can
  accept a gameplay fact. Its ID is stable review vocabulary, not executable
  dispatch.
- Platform adapters may parse a URL and select a compiled recipe; they do not
  author or mutate showcase content.
- The first schema stays intentionally narrow. A request for dynamic behavior
  must land as a real gameplay system or a separately reviewed scenario
  framework, not expand this save-state format into a programming language.

## Acceptance

- Schema tests reject unknown fields, duplicate IDs, invalid references,
  unsupported kinds, excessive counts, invalid coordinates, conflicting
  player state, and missing/wrong live-instantiation evidence.
- Compilation tests produce deterministic chunk/entity/player records and
  hydrate them through the ordinary integrated server.
- Existing live-game tests continue to prove natural mallard spawning, egg and
  feather production, nest placement, hatching, observation unlocks, and
  persistence independently of the showcase.
- Query tests prove generic transient seed/location launch remains available
  and named showcases are strict, transient, and direct-entry only.
- Native and Web captures report the same recipe revision, seed, entry pose,
  entity composition, and field-guide state; both pixel outputs are inspected.
- Refreshing the showcase starts from the recipe again and creates no IndexedDB
  world records.
- Workspace, all-target, thin-adapter, Web build/smoke, desktop offscreen, and
  affected focused tests pass.
- The pushed revision deploys successfully at `mclone.kzahel.com`; the public
  showcase URL is opened and verified before handoff.

## Execution Record

Implemented in the `playable-showcases` commit series and integrated with the
concurrently published XR/terrain work at `ccc711fd`.

- Added schema-v1 `mallard-ecology.showcase.json`, the shared typed compiler,
  deterministic stable entity identities, and ordinary persistence-record
  output. The recipe is limited to one authored island, 256 block patches, 64
  entities, narrow fact allowlists, and deny-unknown-fields parsing.
- Added the typed live-instantiation registry. Every lily pad, mallard life
  stage, nest, inventory item, and observation in the recipe must identify a
  compatible ordinary producer and contract anchor; unknown and mismatched
  evidence fails validation.
- Added strict `?showcase=mallard-ecology` browser entry, worker-side shared
  compilation, transient storage enforcement, and conflict rejection.
- Replaced bespoke native mallard capture authoring with the same recipe and
  added exact native/Web receipt assertions plus a public deployed smoke.
- Preserved the shared multiview/fog/render and newer persistence/scheduler
  contracts when merging the concurrent upstream history. The showcase added
  no entity-tick, gameplay, renderer, protocol, audio, or UI mode.

Acceptance evidence:

- `cargo test -p mclone-server playable_showcase`: 4 passed.
- `cargo test -p mclone-server frozen_day_time`: 1 passed.
- affected `cargo check`: passed for server, app runtime, render, scene, Web,
  and UI crates.
- `pnpm native:mallard-ecology:capture`: passed; flat and synthetic-stereo
  pixels inspected under `/tmp`.
- `pnpm native:web:showcase-smoke`: passed; local Web pixels inspected.
- Cloudflare deployed `ccc711fd` as version
  `24430e36-a47d-4c45-8102-eae90a6f4e3f`.
- `pnpm native:web:showcase-deployed-smoke`: passed against
  `https://mclone.kzahel.com`; public pixels inspected.
- Local and deployed Web screenshots were byte-identical with SHA-256
  `ebe04116774cdf19cfa8c9fb092f4e1a0a432bee918a111ab6c5d3f6157836ea`.
- Both Web receipts reported recipe revision 1, seed `17502`, eye
  `8.5,67.62,7.5`, target `8,65,1.5`, four entities, three mallards, one nest,
  six field notes, two eggs, and one feather. All eight IndexedDB world-store
  counts remained zero.

The verified clean review URL is:

```text
https://mclone.kzahel.com/app.html?showcase=mallard-ecology
```

The durable workflow and guardrails now live in
[`../topics/playable-showcases.md`](../topics/playable-showcases.md) and
`AGENTS.md`.
