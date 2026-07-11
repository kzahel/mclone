# 171: Convergence And Parity Closeout

Status: active coordinating parent 2026-07-11. Milestones A–D and the first
step of Milestone E have landed. The
native and browser scene-host convergence series and runtime asset-pack
implementation are closed, and production browser far LOD now uses the shared
resident-tile worker/runtime/render path. Tactical 166 — Shared Resident-Tile
Substrate is complete through Slice 4 multi-level rings. Tactical 172 — Far
LOD Settle Contract And Detail Modes is active on the LOD thread: Slice 1A's
pull-only exact-set/ledger foundation and Slice 1B's first executable
spawn/fly-up settle probe have landed. Slice 1C1's validated waypoint scripts
and permanent smoke lane have also landed, along with Slice 1C2a's projected
coverage-pixel and exact revisit assertions. Slice 1C2b1's desired-level and
movement matrix is complete; Slice 1C2b2's shared-policy toggle/range mutations
and full 14-waypoint lane are also complete. Slice 2's coverage-gap correctness
burn-down is next, followed by detail modes. Tactical 162 —
Real-Chunk LOD Reduction Draft Slice 4 resumes after 172 Slice 2's gates are
green. This parent owns the
cross-thread burn-down and final status reconciliation, not duplicate
implementations of its child tacticals.

Topic: `convergence-and-parity-closeout`

Workstream: cross-platform shared Rust architecture, render residency/LOD,
browser feature parity, runtime asset replacement follow-ups, and documentation
truth. Implementation remains shared-first with desktop validation first and
the platform/device gates required by each child tactical.

## Why This Parent Exists

Several individually sensible sequences converged on
`mclone_scene::McloneSceneHost`:

```text
shared startup contract ------------------+
native scene-host convergence ------------+-- one scene-policy owner
browser scene-host adoption --------------+
runtime asset-pack epoch replacement -----+

synthetic far LOD ---------------------------- shared resident-tile substrate
reduced-real LOD -----------------------------/

one scene-policy owner + one tile substrate --> web feature-parity burn-down
```

That dependency graph made closed work look active and made later prerequisites
look like continuations of older tacticals. This tactical is the single macro
answer to four questions:

1. Which implementation series are closed?
2. Which open tactical owns the next code change?
3. Which deferred gaps do not reopen an otherwise completed tactical?
4. What evidence empties the remaining browser exception ledger?

Child tacticals continue to own design details and execution evidence. This
parent owns ordering, status, and final cross-sectional closeout.

## Source-Of-Truth Rule

When an older tactical's “next step” conflicts with this parent or with a newer
explicit re-scope:

- the newer re-scope controls implementation order;
- the older tactical is updated to say paused, superseded, or complete;
- completed tacticals are not reopened for adjacent follow-ups;
- every browser feature promotion removes exactly one tested exception in the
  same change that lands its complete runtime and evidence.

## Thread Ledger

| Thread | Owning tactical | State on 2026-07-11 | Exit or next action |
|---|---|---|---|
| Resident CPU terrain mesh removal | Tactical 163 — Render Section CPU Mesh Eviction | **Closed.** Slices 0–5 landed; the startup-remesh consequence was resolved by Tactical 167 — Shared Native Session Startup Contract. | Preserve the compact-metadata and transient-upload-payload contract in later residency work. |
| Native startup convergence | Tactical 167 — Shared Native Session Startup Contract | **Closed.** Slices 0–5 landed. | No follow-up in this parent. |
| Native feature availability | Tactical 165 — Native Feature-Parity Baseline And Exception Burn-Down | **Closed for native.** The native exception ledger is empty and thin-adapter enforcement is executable. | Keep browser exceptions in the separate ledger below. |
| Native live host convergence | Tactical 168 — Unified Native Scene Host | **Closed.** Slices 0–10 landed. | Do not recreate app-local frame/session/render policy. |
| Runtime asset-pack selection and replacement | Tactical 169 — Runtime Asset Pack Selection | **Closed.** Slices 0–6 landed across native and web. | Track optional worker/bootstrap gaps below without reopening the tactical. |
| Browser host adoption | Tactical 170 — Web Scene-Host Adoption | **Closed.** Slices 0–6 landed; the old browser orchestrator is deleted. | Burn down only the exact web feature exceptions below. |
| Existing synthetic and future reduced-real LOD | Tactical 162 — Real-Chunk LOD Reduction Draft | **Paused at Slice 4.** Slices 0A–2 landed; Slice 3 was superseded by the substrate; Slice 4 is re-scoped behind Tactical 172's harness and correctness gates. | Resume Slice 4 (first reduced-real producer) after Tactical 172 Slice 2 is green. |
| Shared real-section/LOD lifecycle | Tactical 166 — Shared Resident-Tile Substrate | **Closed. Slices 1–4 and the production browser proof landed.** | Preserve its worker, admission, residency, transition, seam, and render gates for later producers. |
| Far LOD settle contract, correctness, and detail modes | Tactical 172 — Far LOD Settle Contract And Detail Modes | **Active; Slice 1 is complete:** exact-set/ledger access, smoke/C2/revisit, movement levels, shared-policy mutations, and the full 14-waypoint lane landed. The high fixture pins 165 sky-clear pixels across seven columns plus three paintable-frustum, graph-culled columns still suppressing LOD. | Run Slice 2 correctness burn-down, then the detail/residency slices. |
| Cross-thread status and browser parity | Tactical 171 — Convergence And Parity Closeout | **Active; Milestones A–D and the first Milestone E step landed.** | Coordinate Tactical 172 Slice 2 next; resume Tactical 162 Slice 4 after 172 Slice 2; close only after the milestones below and explicit deferred decisions are recorded. |

## Asset-Pack Follow-Ups That Do Not Reopen The Closed Series

Tactical 169 — Runtime Asset Pack Selection delivered the requested product
contract. Two gaps remain independent follow-ups:

- Browser selected-pack CPU preparation currently pauses `requestAnimationFrame`
  in main Wasm. Move it to a dedicated worker only after a measurement proves
  the pause warrants another worker lifecycle.
- Clients may fetch or stage the Minecraft reference payload while constructing
  epoch zero before restoring a first-party preference. A distribution that
  must never fetch or stage proprietary assets needs a first-party epoch-zero
  bootstrap/package path.

Neither gap invalidates transactional replacement, selection persistence, or
strict active-epoch provenance. If implementation becomes timely, open a
focused child tactical rather than appending slices to the closed tactical.

## Exact Browser Feature Burn-Down

The executable `WEB_FEATURE_PARITY_EXCEPTIONS` ledger currently contains four
entries. Tactical 171 — Convergence And Parity Closeout is the coordinating
tactical for their removal; a focused child tactical is optional when one row
needs more than one bounded slice.

| Browser exception | Current reason | Promotion gate |
|---|---|---|
| `TravelAssist` | Browser input, UI, and movement behavior are not proven together. | Shared action/effect handling, desktop-equivalent browser input semantics, settings UI, movement smoke, and visible state evidence. |
| `FramePipelineOverlay` | Browser frame-accounting reports are not fully projected into the shared overlay. | Shared accountant inputs, honest unavailable fields, UI toggle, report conservation tests, and inspected browser capture. |
| `DebugDiagnostics` | Browser diagnostic presenter/panel wiring lacks complete proof. | Shared diagnostic facts, browser presenter wiring, UI toggle, focused tests, and inspected desktop/mobile browser captures. |
| `ServerSimulationCadence` | Browser local-runtime cadence control and diagnostics are not proven. | Shared cadence effect reaches the local worker/server, remote mode rejects or marks it unavailable honestly, diagnostics report the applied value, and a browser smoke proves behavior. |

Rules for every row:

- remove only that exact entry and its matching reason;
- change the capability to `Supported` in the same commit;
- exercise the settings action through the production browser scene host;
- retain local-worker, IndexedDB, and remote-WebSocket host-mode coverage;
- inspect pixels for every visible feature;
- do not weaken the default feature-profile or thin-adapter enforcement tests.

Audio output and teleport-preview rendering are absent browser services rather
than entries in this feature-axis ledger. Track them separately instead of
silently expanding or redefining these four rows.

## Ordered Closeout Milestones

### Milestone A — Documentation and substrate foothold

- [x] Reconcile completed startup, host, asset-pack, and browser-adoption
  tacticals.
- [x] Correct Tactical 162 — Real-Chunk LOD Reduction Draft so it no longer
  advertises its superseded Slice 3 as next.
- [x] Land Tactical 166 — Shared Resident-Tile Substrate Slice 1.
- [x] Make this parent and the tactical/topic indexes the macro entry point.

### Milestone B — Shared budget vocabulary

- [x] Implement Tactical 166 — Shared Resident-Tile Substrate Slice 2.
- [x] Add inert LOD decision families and ordering to the existing shared
  frame-budget panel without adding another scheduler or queue.
- [x] Prove existing real-section decisions and pixels remain unchanged.

### Milestone C — Synthetic far LOD onto the substrate

- [x] Implement Tactical 166 — Shared Resident-Tile Substrate Slice 3.
- [x] Move generation and meshing to the shared render-compile worker pool.
- [x] Replace monolithic remesh/re-upload with per-tile lifecycle and
  region-arena uploads/draws.
- [x] Pass desktop, per-eye XR, full-frame multiview, Quest movement, and
  far-LOD-off invariance gates.

### Milestone D — First browser exception removal

- [x] Run the production web far-LOD proof after Milestone C.
- [x] Remove only `FarLod` from `WEB_FEATURE_PARITY_EXCEPTIONS` after every gate
  passed.
- [x] Leave the other four reasons byte-for-byte intact.

Landed evidence (2026-07-11):

- `WebSceneRuntimeService` now owns the same `FarTerrainLodCache` and
  `LodCoverageCoordinator` used by native hosts. Browser frame preparation
  advances that cache from the projected monotonic clock, drains accepted
  tiles into the existing region-arena renderer, and clears coverage on asset
  epoch replacement.
- Far-LOD jobs use the resident `WebRenderSectionCompiler` worker and its
  `SharedArrayBuffer` input/result arenas. The worker accepts a compact
  deterministic tile request, calls the Wasm compiler with the active asset
  palette, and returns the packed tile mesh through the shared-result buffer.
  Real section work retains priority; no main-thread generation, generated
  view fallback, or private worker pool was added, and the existing result
  overflow diagnostic remained dormant.
- The shared Graphics UI action enables the production path. Browser reports
  expose desired/resident/visible tile counts, build and upload queue state,
  total upload bytes, per-frame region draws/uploads, and worker transport
  proof. `FarLod` is now `Supported`; the exact remaining four exception rows
  and their reasons are unchanged.
- Dedicated local-worker, IndexedDB local-world, and remote-WebSocket probes
  each reached 64 resident and 64 visible tiles, four region draws, and
  `317152` uploaded bytes. All three reported `workKind=far-lod`,
  `shared-result-buffer`, no generated-view fallback, and no result overflow.
  Final maximum frame gaps were `10.335ms` local, `10.320ms` IndexedDB, and
  `10.315ms` remote.
- Each probe captures the same paused production overview with Far LOD off and
  on, rejects incomplete WebGPU presentation readbacks, and observed `32930`
  changed pixels. The inspected local and remote images showed detailed
  spruce/mountain terrain with the coarse green horizon filling only when Far
  LOD was enabled; the IndexedDB PNG was byte-identical to the clean local
  capture. Captures and reports remain under
  `/tmp/mclone-native-web-far-lod-*`; none are committed.
- Focused validation passed app-runtime Far LOD and client-experience tests,
  all web-client tests, direct `wasm32-unknown-unknown` web check, browser
  typecheck, unchanged local/IndexedDB/remote/movement/asset-pack smokes,
  thin-adapter purity, formatting, and the inspected native desktop offscreen
  pixel canary. The first unchanged local app smoke hit a transient
  movement-settle timeout; its immediate unchanged rerun passed.

### Milestone E — LOD quality evolution

- [x] Implement Tactical 166 — Shared Resident-Tile Substrate Slice 4
  multilevel rings if the single-level substrate evidence is green.
- [ ] Resume Tactical 162 — Real-Chunk LOD Reduction Draft at Slice 4 for
  reduced-real tiles.
- [ ] Continue its edit dirtying and discardable persistence only after the
  reduced-real producer passes precedence, seam, and authority tests.

Tactical 166 — Shared Resident-Tile Substrate Slice 4 closed its prerequisite
with 4/8/16 rings, two-chunk hysteresis, capped replacement-before-suppress
double residency, cross-level neighbor rebuilds, and per-level diagnostics.
Native, all three production browser modes, per-eye Quest movement, and Quest
full-frame multiview gates passed; detailed evidence remains in that tactical.

### Milestone F — Remaining browser parity

- [ ] Burn down `TravelAssist`.
- [ ] Burn down `FramePipelineOverlay`.
- [ ] Burn down `DebugDiagnostics`.
- [ ] Burn down `ServerSimulationCadence`.
- [ ] Keep browser audio and teleport preview as separately named service
  work, not invisible additions to this ledger.

### Milestone G — Final closeout

- [ ] Decide or separately schedule both asset-pack follow-ups.
- [ ] Re-audit `docs/topics/platform-parity.md`, `docs/platforms.md`, and
  `docs/native-engine-architecture.md` against the live source gates.
- [ ] Confirm the native feature ledger is empty and the browser ledger is
  empty or contains only deliberately deferred, reason-bearing product gaps.
- [ ] Mark this parent complete and name the next independent product priority.

## Current Next Step

Continue Milestone E by resuming Tactical 162 — Real-Chunk LOD Reduction Draft
at Slice 4: add the first in-memory reduced-real tile producer from already
available authoritative chunk snapshots, using the completed shared worker,
admission, residency, replacement, upload, coverage-precedence, and
per-eye/multiview paths. Stop before edit dirtying or discardable persistence;
those remain later slices after the producer passes authority, seam, pixel, and
performance gates.

Topic: convergence-and-parity-closeout
