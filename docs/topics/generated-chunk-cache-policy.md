# Generated Chunk Cache Policy

Topic: `generated-chunk-cache-policy`

Status: accepted direction 2026-07-28; implementation not started.

## Scope

This topic owns the server policy for persisting deterministic generated chunk
state that has no durable world changes. It does not replace the bounded
request, cancellation, fairness, and durability work in Tactical
[`275`](../tactical/275-bounded-persistence-streaming.md), and it does not
change the physical SQLite or IndexedDB executor contract in
[`unified-persistence-interface.md`](unified-persistence-interface.md).

The motivating use case is storage-constrained hosts such as Quest. Sustained
fast travel can visit enough terrain that caching every generated chunk grows
the world indefinitely even when the player never changes it. Regeneration is
a reasonable space-for-CPU trade when the authority can prove that a record is
derived rather than durable world state.

## Accepted Product Direction

Add a binary per-world server policy, provisionally named
`GeneratedChunkCachePolicy`:

| Value | Player-facing label | Meaning |
|---|---|---|
| `Store` | Cache unedited terrain: On | Lazily persist eligible generated-clean chunk records for faster revisits |
| `Regenerate` | Cache unedited terrain: Off | Do not start new writes for eligible generated-clean chunk records; regenerate them after a cache miss |

The initial default is `Store`, matching current behavior and minimizing
surprises for existing worlds. `Regenerate` is the explicit storage-optimized
choice. A later `Auto` mode may enforce a measured byte budget or recency
policy, but it should not be invented before record provenance, deletion, and
quota evidence exist.

This is a world/server option, not a graphics option. The authoritative host
decides whether its world records are written; a renderer or remote client
cannot safely make that choice.

The intended settings surfaces are:

- a host-side default for newly created worlds under **Options → Storage**;
- the persisted value for an existing world under **World Options → Storage**;
- the same shared menu row on desktop, Android, Android XR, desktop XR, and
  web, writable when that client owns or administers the authority and
  read-only as **Server controlled** for an ordinary remote client; and
- a dedicated-server configuration/admin path using the same shared policy.

The per-world value is canonical after creation and belongs in authoritative
world metadata. An integrated host may initialize it from the local user's
new-world default; a dedicated host may initialize or explicitly change it
from server administration. Client preference storage must never silently
override an existing world's value.

## Persistence Contract

Turning the option off changes future write election only:

- generated-clean cache candidates created after the server acknowledges the
  change are not submitted;
- already accepted or in-flight cache writes may finish;
- existing stored chunk records remain readable;
- existing records are not deleted; and
- turning the option back on resumes lazy cache writes for future candidates.

Therefore switching to `Regenerate` does not immediately reduce an existing
world's size. A separate **Reclaim generated terrain cache** operation may be
added after stored records carry trustworthy provenance. It must report what
it can remove and must never infer discardability merely from the absence of a
currently loaded edit.

The policy must be applied where the authoritative scheduler elects a
generated-clean chunk write. It must not be implemented as a blanket filter
for every `SaveDurability::Cache` request. `SaveDurability` currently describes
write scheduling and shutdown obligations; it is not durable proof that every
byte in the record is reproducible. In particular, a later status promotion
can produce a newer chunk record containing earlier durable edits.

The existing persistence guarantees remain unchanged in both modes:

- durable chunk changes save before eviction and world close;
- newer records containing durable changes are not suppressed even if the
  newly added portion is generated state;
- entity-chunk records remain separate and durable, including empty records
  that prevent generation-time actors from respawning;
- player records, world metadata, dimension records, saved data, and future
  block-entity records remain durable; and
- bounded queues, cancellation, fair write service, revision precedence,
  same-key pending-write visibility, flush, close, and save-unhealthy behavior
  remain active.

## Conservative Meaning Of Generated-Clean

“No player block edits” is not a sufficient classifier. A chunk is eligible
for suppression only when the server can prove that the entire omitted record
is reproducible from the world's persisted definition and generator inputs.

Durable treatment wins if the chunk contains or depends on any state that
cannot be safely reconstructed, including:

- block or fluid changes made by commands, players, actors, or simulation;
- scheduled block/fluid work whose live progression is not reproducible;
- block entities, inventories, one-shot state, or inhabited-time-like state;
- nondeterministic or externally authored generation results;
- durable edits carried forward through a later status promotion; or
- any future field whose provenance has not been classified.

Deterministic blocks, biomes, generated status progress, and other purely
derived fields may be cache candidates when their generator/profile contract
can reconstruct them. If a new field lacks a proven classification, it is
durable by default.

Entity records are never made optional by this setting. An empty entity record
is a durable tombstone-like fact: deleting it can resurrect killed or removed
generation-time actors even if the block terrain itself is regenerated.

## World-Generation Compatibility

`Regenerate` deliberately trades historical terrain freezing for storage.
After a generator or content update, an unedited omitted chunk may regenerate
differently, and a durable edited chunk can border newly generated terrain.
The UI should disclose that possibility rather than describe the mode as
lossless compression.

The current compatibility safety ledger in
[`world-generation-profiles.md`](world-generation-profiles.md#compatibility-safety-ledger)
still governs profile changes. All live profiles, including the legacy Java
1.17-shaped `overworld`, are internal and mutable. Before public release, the
product needs an explicit
generator freeze/migration contract if `Regenerate` is expected to preserve
seamless reopened worlds across upgrades.

`Store` also must not be advertised as an absolute compatibility guarantee.
Generated cache remains versioned and discardable and may be invalidated by a
schema or compatibility decision.

## Expected Tradeoff

With caching off:

- disk growth and generated-cache write/encoding work should fall sharply
  during one-way exploration;
- revisiting evicted terrain consumes generation CPU and may reacquire exact
  terrain more slowly, especially on Quest;
- the procedural horizon can continue providing fixed-budget distant visual
  coverage while exact chunks regenerate; and
- process-memory safety still depends on the bounds from Tactical 275. This
  policy addresses retained storage and write work, not every possible memory
  owner.

The choice should remain a world policy instead of a platform default so the
same dedicated world behaves consistently regardless of which client visits
it. Platform-specific recommendations may suggest `Regenerate` on constrained
local hosts without changing authority semantics.

## Implementation Direction

1. Add the shared server enum and persist it in versioned world metadata.
   Existing metadata decodes to `Store`.
2. Thread the resolved policy through integrated, dedicated, browser-worker,
   and test authority construction without adding platform-local save rules.
3. Make generated-clean eligibility explicit at the scheduler/record boundary
   and suppress only proven derived chunk candidates in `Regenerate`.
4. Add regressions proving edits, simulation changes, entity records, and
   empty entity records remain durable while clean generated records are
   omitted.
5. Expose the shared setting and authority/read-only state through shared UI
   contracts on every client target, plus dedicated-server administration.
6. Add diagnostics for policy-suppressed writes and estimated bytes avoided,
   distinct from cache writes skipped because the bounded queue was under
   pressure.
7. Add stored provenance and a separately confirmed reclamation operation
   before promising that a policy change can shrink an existing world.

The first implementation should remain binary. Budgeted `Auto`, LRU cache
reclamation, compaction, and browser persistence-grant UX are follow-ups that
need their own measured contracts.

## Validation And Acceptance

Acceptance requires shared tests and backend evidence that:

- a new `Store` world preserves current generated-cache behavior;
- a new `Regenerate` world can travel, evict, revisit, and regenerate a clean
  chunk without writing a clean chunk record;
- a block/fluid mutation survives eviction and restart in `Regenerate`;
- an empty entity-chunk record still prevents generation-time respawn when the
  corresponding clean block chunk is absent;
- policy metadata round-trips and old worlds default to `Store`;
- runtime toggles follow the future-writes/no-immediate-delete contract;
- local and remote authority state is represented consistently in the shared
  menu contract; and
- memory, SQLite, and IndexedDB agree on logical behavior.

Physical acceptance should repeat sustained high-speed travel on Quest with
world-size growth, generated-cache writes, policy suppressions, exact-center
reacquisition time, queue high-water marks, and process memory recorded. A
flat native or web soak should exercise the same authoritative policy so the
result is not mistaken for an XR-only fix.

## Related Documents

- [`loading-persistence.md`](../loading-persistence.md) — vanilla and current
  chunk lifecycle/save policy
- [`persistence-architecture.md`](../persistence-architecture.md) — durable
  state versus derived-cache architecture
- [`unified-persistence-interface.md`](unified-persistence-interface.md) —
  shared coordinator and backend boundary
- Tactical [`275`](../tactical/275-bounded-persistence-streaming.md) —
  bounded queues, cancellation, fair writes, diagnostics, and the original
  Quest travel observation
