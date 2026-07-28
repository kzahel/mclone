# Tactical 275: Bounded Persistence Streaming

Status: complete 2026-07-28, including physical Quest travel soak.

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

Physical closeout landed through Tactical
[`277`](277-quest-procedural-horizon-performance.md). The release Quest 3
composed-horizon lane completed consecutive `180s` / `6,191`-block and `300s`
/ `10,319`-block flights at `34.4` blocks/second:

- all `12,916 + 21,510` sampled horizon frames retained an exact-ready current
  center;
- live foreground persistence ownership peaked at 30 requests;
- live cache writes peaked at 15 requests and about `0.61MiB`;
- durable writes peaked at two requests and 720 bytes;
- late queues drained to zero except one `15KiB` cache write;
- process RSS oscillated around `1.03–1.22GiB` in the longer run rather than
  growing with distance; and
- the app completed both samples normally without low-memory termination.

The disposable guardrail world reached `504,496KiB` after the combined travel.
That is evidence for the separate generated-clean storage-policy decision, not
retained transient memory.

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
3. Keep the now-passing physical Quest soak reproducible when persistence or
   streaming ownership changes. The clipmap is fixed-residency; the 2026-07-28
   run found no second distance-proportional transient owner.
4. Consider backend batching/region compaction only from measured write and
   storage evidence; it must not weaken the queue or durability invariants.

## Acceptance

The implementation slice is accepted when automated tests prove every bound,
durable writes survive pressure, obsolete reads cannot repopulate caches, and
native plus browser diagnostics report the live quantities. The device
incident is accepted as closed only after sustained persistent-world travel
keeps exact-center terrain reacquiring and both process memory and queue
high-water marks plateau. The Tactical 277 Quest evidence satisfies that
device gate; generated-clean disk growth remains separate.
