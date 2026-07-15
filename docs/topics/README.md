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

- [`multiplayer-networking.md`](multiplayer-networking.md): wire protocol,
  transports, session lifecycle, server tick/publication cadence — current
  state, structural gaps (lockstep request/response wire, tick-per-command
  dedicated server, no session layer), and the phased plan toward a
  vanilla-shaped push protocol with configurable tick rates.
- [`client-prediction.md`](client-prediction.md): player movement authority —
  vanilla-shaped client-authoritative interim, planned server validation
  checks, the preserved sequenced-input-replay path, and remote-actor
  interpolation divergence.
- [`platform-parity.md`](platform-parity.md): cross-platform parity tracker —
  per-platform-class target state, the feature × platform matrix, the
  shared-contract × consumer reuse matrix, and the cross-cutting blockers that
  keep new features from re-forking across the client lanes and offscreen
  validation hosts.
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md): completed browser
  adoption of the shared `McloneSceneHost`, including landed service and
  typed-async boundaries, preserved worker/compiler topology, production
  cutover evidence, the subsequently proven browser Far LOD producer, and the
  exact remaining four-row browser feature ledger. Macro sequencing now lives
  in Tactical 171 — Convergence And Parity Closeout.
- [`far-lod.md`](far-lod.md): synthetic far-terrain LOD status — the landed
  resident-tile producer mechanism, the settle contract, the confirmed
  coverage-defect ledger (altitude graph-cull voids, suppression/painted
  mismatch), planned settle-state validation lanes, and the tactical 172 →
  162 ordering.
- [`lighting.md`](lighting.md): native Java-shaped stored-light system,
  render handoff, solver/status/rendering gaps, and next slices.
- [`performance.md`](performance.md): high-priority known performance issues,
  low-hanging pickup guidance, native baselines, the broader priority queue,
  and Java-shaped render/scheduling follow-ups.
  The broader frame/terrain/host accounting model lives in
  [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md).
- [`asset-pack-profiles.md`](asset-pack-profiles.md): shared UI asset-pack
  selection, pack-time generated missing assets, runtime provenance, and the
  standalone first-party boundary.
- [`falling-tree-physics.md`](falling-tree-physics.md): Dynamic Falling Tree
  and Sable reference investigation for future tree felling, moving voxel
  assemblies, and impact effects.
- [`embedded-worlds.md`](embedded-worlds.md): design north-star for rendering a
  second world inside the current one — lobby diorama, seed-explorer console,
  network "palantir" window, warm world switching, walkable seams, and
  shrink-and-fall nesting. Records geometry-first XR composition, the
  baked→live-local→remote-spectator→joined-warm fidelity ladder, N-world budget,
  runtime/authority/render seams, and the protected single-world fast path. A
  first non-rendering app-runtime smoke retains two isolated integrated hosts;
  composition and product switching remain unbuilt. Tactical
  [`174`](../tactical/174-warm-world-hot-swap.md) owns the bounded opaque-gate
  hot-swap milestone.
- [`visor-vr-reference.md`](visor-vr-reference.md): Visor VR architecture
  research, including reusable lessons for world-space UI surfaces, pointer
  focus/capture, virtual-keyboard text entry, input contexts, render staging,
  lifecycle, addons, settings, and remote XR pose replication.
- [`vanilla/networking.md`](vanilla/networking.md): Minecraft Java 1.17.1
  vanilla networking reference — connection pipeline, login/join sequence,
  50 ms tick loop, chunk/entity sync cadences, movement validation and
  teleport acks, interpolation, and a constants quick-reference table.
- [`vanilla/weather.md`](vanilla/weather.md): Minecraft Java 1.17.1 vanilla
  weather reference notes, including rain, snow, thunder, lightning/fire,
  randomness, biome-local precipitation, and non-vanilla atmosphere boundaries.

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
