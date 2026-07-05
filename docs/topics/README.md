# Topics

Durable subsystem progress indexes live here.

Topic docs are the current map for a subsystem: status, Java reference anchors,
native code map, active tactical links, latest validation notes, and recommended
next slices. Tactical docs remain the implementation records for individual
slices.

Use a topic doc when a subsystem spans multiple tacticals, has durable
reference-reading requirements, or needs an explicit "where are we now" answer
between sessions.

## Current Topics

- [`platform-parity.md`](platform-parity.md): cross-platform parity tracker —
  per-platform-class target state, the feature × platform matrix, the
  shared-contract × consumer reuse matrix, and the cross-cutting blockers that
  keep new features from re-forking across the client lanes and offscreen
  validation hosts.
- [`lighting.md`](lighting.md): native Java-shaped stored-light system,
  render handoff, solver/status/rendering gaps, and next slices.
- [`performance.md`](performance.md): native performance priority queue,
  baselines, and Java-shaped render/scheduling follow-ups.
  The broader frame/terrain/host accounting model lives in
  [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md).

## Update Policy

- Update a topic doc when a tactical lands, a recommendation changes, or a new
  validation result changes the next step.
- Keep detailed per-slice work in `docs/tactical/`; keep topic docs short
  enough to scan.
- Link reference Java files and native modules explicitly so future work starts
  from the right boundaries.
- Prefer Java-shaped module boundaries in the native implementation. A topic doc
  should call out when a subsystem is at risk of becoming a large catch-all
  module.
