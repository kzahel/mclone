# Tactical 275: Bounded Persistence Streaming

Status: implementation complete 2026-07-28; physical Quest travel soak pending.

Topic: `unified-persistence-interface`

## Originating Request

The first sustained high-speed Quest 3 distant-terrain session eventually
stopped reacquiring exact terrain and was killed by Horizon OS at its memory
limit. The persistent exact-world path could retain obsolete reads and
full-record generated-cache writes without a hard bound, and continuous
incoming reads could prevent the storage worker from advancing writes.

Implement bounded queues, cancellation, and fair writing now, independently of
the later product decision about whether generated chunks with no edits should
be persisted. Quest and other clients have limited memory and storage; durable
edits must never be discarded merely to satisfy a cache bound.

## Scope And Invariants

This tactical changes shared persistence coordination, not distant-terrain
rendering and not the on-disk clean-chunk policy.

- Foreground native storage work has a strict 256-request bound.
- Durable native writes have a strict 64-record / 64 MiB outstanding bound.
  Producers apply backpressure at the bound; durable writes are never shed.
  One individually oversized durable record is admitted so the byte bound
  cannot deadlock it.
- Discardable generated-cache writes have a strict 64-record / 32 MiB
  outstanding bound. Same-key revision coalescing still applies. New cache
  work beyond either limit completes as `SkippedCachePressure`; it can be
  regenerated later.
- The native request channel is a bounded 384-slot channel matching the sum of
  the lane limits.
- A worker services at most one incoming request before advancing one pending
  write. Continuous storage reads can no longer starve writes.
- Chunk and entity-chunk reads are cancelled as soon as their holder loses
  active interest, rather than waiting for the budgeted holder-unload pass.
  Work not yet started never reaches storage; an unavoidable in-flight result
  is discarded.
- The browser's Rust record mirror releases full chunk and entity payloads
  when scheduler residency ends. Cancelled late IndexedDB reads do not
  repopulate it.
- Queue counts, estimated owned bytes, high-water marks, cancellation count,
  skipped-cache count, and retained browser-record bytes are available through
  `ServerRunnerDiagnostics`, including across the web Worker boundary.

The byte counters are conservative owned-heap estimates, not process RSS.
They are intended to identify distance-proportional retention and pressure
events during a device soak.

## Implementation

The shared owner is
`native/crates/mclone-server/src/persistence.rs`.

`PersistenceMailbox` now exposes cancellation, residency-linked cache release,
and `PersistenceQueueMetrics`. The native actor uses bounded admission and an
out-of-band cancellation set so the scheduler does not need to enqueue a
cancellation command behind the obsolete read it is trying to avoid. The
worker alternates request service with pending-write progress while retaining
durable-first selection inside the write actor.

`ChunkScheduler` counts already-pending reads against its per-family admission
limit, cancels reads on interest loss or demotion below `Features`, and releases
record-executor caches when the holder is actually removed. This preserves the
existing budgeted unload lifecycle while removing its stale-read retention
window.

The generic record executor has a cache-release hook. Native SQLite does not
retain decoded full records and therefore needs no physical eviction action.
The browser executor removes the corresponding opaque decoded payload from its
Rust mirror; IndexedDB remains the physical source of truth.

## Automated Evidence

Shared regressions cover:

- distinct generated-cache writes stopping at both count and byte limits;
- the threaded cache lane remaining bounded while the physical store is
  blocked;
- durable writes applying backpressure at 64 records and all completing as
  `Written` after storage resumes;
- one pending write advancing before the next foreground read;
- native queued-read cancellation preventing any physical load;
- browser/external queued cancellation and discard of a late completion;
- browser/external full-record cache release with chunk residency; and
- a 10,000-chunk interest jump cancelling obsolete probes while keeping load
  admission bounded.

Validation completed on 2026-07-28:

- `cargo test -p mclone-server --lib`: 569 passed;
- `cargo check --workspace`: passed;
- `pnpm native:web:build`: passed for `wasm32-unknown-unknown`;
- `pnpm native:web:typecheck`: passed;
- `cargo test -p mclone-web-client --tests -- --skip
  test_only_coarse_operation_is_a_boundary_fixpoint`: 79 passed across unit
  and integration targets. The omitted source-shape pin is unrelated and
  already stale at the base revision: untouched `WebSceneHost` has 38
  mechanical exports while the test expects 37;
- `pnpm native:android:apk`: passed;
- `pnpm native:android-xr:apk`: passed; and
- Quest 3 `2G0YC1ZF93041Z`: the release APK installed and reached the
  session-ready marker through
  `pnpm native:android-xr:validate --skip-build --serial
  2G0YC1ZF93041Z --session-only --wait-seconds 30 --skip-assets`.

The normal validator's first attempt installed the APK but stopped before
launch because the repository's pre-existing default asset-pack lock reports
stale extracted-source and pack fingerprints. The successful rerun retained
the previously staged headset assets rather than rewriting that unrelated
lock.

Still required is a persistent Quest 3 8x-flight/churn soak with queue metrics,
exact-center reacquisition, RSS, and on-disk world growth recorded.

## Deliberately Separate Follow-Ups

This tactical bounds transient persistence ownership. It does not bound the
physical size of a world whose policy still elects to store every generated
clean chunk.

1. Decide and implement the generated-clean storage policy: always cache,
   storage-optimized dirty-only, a user option, or a quota/recency policy.
   Dirty chunks, entities, players, metadata, and other durable state remain
   mandatory in every mode.
2. Prune or separately bound completed scheduler job-history metadata during
   the same long-distance audit.
3. Run the physical Quest soak before claiming the observed low-memory kill is
   closed. The clipmap is fixed-residency, but this change does not prove there
   is no second distance-proportional owner.
4. Consider backend batching/region compaction only from measured write and
   storage evidence; it must not weaken the queue or durability invariants.

## Acceptance

The implementation slice is accepted when automated tests prove every bound,
durable writes survive pressure, obsolete reads cannot repopulate caches, and
native plus browser diagnostics report the live quantities. The device
incident is accepted as closed only after sustained persistent-world travel
keeps exact-center terrain reacquiring and both process memory and queue
high-water marks plateau.
