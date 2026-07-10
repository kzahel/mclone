# Topics

Focused, living records of continuing concerns live here.

Prefer the smallest coherent topic whose status, decisions, evidence, and next
work benefit from continuity across sessions or commits. A topic can cover a
contract, recurring problem, product decision, implementation campaign, status
question, or investigation; it does not need to represent an entire subsystem.
Split topics when their decisions or next work can evolve independently.

Adopt this convention incrementally. Existing architecture, reference, and
status docs do not need to move here solely for consistency. Create or update a
topic when current status is hard to answer, work spans multiple tacticals or
commits, important invariants or decisions need to survive the current session,
new evidence changes the direction, or the user explicitly asks for one. Do not
create a topic for every small standalone change.

Documentation roles:

- Architecture and reference docs own durable system shape and external facts.
- Topic docs own current truth, decisions, evidence, gaps, and direction for a
  focused continuing concern.
- Tactical docs under `docs/tactical/` own bounded implementation slices and
  execution records.

New topics should normally start with a crisp scope, a `Topic: <slug>` line,
and an honest status. Add only the sections the concern needs, such as
motivation, current state, contracts and invariants, code/documentation map,
evidence and validation, known gaps, or recommended next work. When a commit
series implements the same concern, normally reuse the document slug in its
`Topic:` trailers.

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

- Read the relevant topic before changing the behavior it governs.
- Update it when a tactical lands or its status, contract, evidence,
  validation, gaps, or recommended direction changes.
- Keep the main text as current truth rather than an append-only diary. Git and
  motivation-preserving commit bodies retain the history.
- Keep detailed per-slice execution in `docs/tactical/` and topic docs short
  enough to scan.
- Link relevant architecture/reference docs, Java files, native modules, and
  tacticals so future work starts from the right boundaries.
- Create a sibling topic instead of turning an existing topic into a catch-all.
