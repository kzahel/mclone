# Embedded Worlds

Topic: `embedded-worlds`

Design north-star for showing another world or another region of the active
world inside the current one: a lobby diorama, a tabletop seed explorer, a
live model of a distant or nearby active-world location, a "palantir" window
into a network-hosted world, and the shrink-and-fall transition between nested
worlds.

[`universe-product-shell.md`](universe-product-shell.md) now records the wider
product/navigation interpretation of these mechanisms. Universe may use the
current retained preview and activation seam, but it is not itself an
authoritative lobby world and does not broaden this topic's bounded
active-plus-optional-standby ownership into an N-world live registry.

The durable realm/dimension/server-topology contract now lives in
[`realm-dimension-runtime.md`](realm-dimension-runtime.md), with its bounded
implementation sequence in
[`185-realm-dimension-and-observer-runtime.md`](../tactical/185-realm-dimension-and-observer-runtime.md).
The current warm destination starts its independent realm connection as a
non-player observer. It receives bounded chunk/entity/player facts without
loading or saving the local profile's player record. Covered activation
promotes that connection to one ordinary player, demotes and saves the source
player to an observer, waits for authoritative safe arrival, and then exchanges
the retained slots. The same exchange runs in reverse for A-to-B-to-A.

Tactical [`201`](../tactical/201-lobby-content-simplification.md) completed on
2026-07-20. The embedded-world proof and product remain, but the persistent
managed-content installer, native staging/publication, browser provisioning
Worker, and TypeScript workflow are gone. The primary is a fresh transient
Rust-authored lobby; the destination is the most recent compatible catalog
world or a stable ordinary app-private persistent fallback.

Status: **a closed, measured scene-owned warm-world milestone, a live-diorama
tactical through completed local Slice 6, a completed native menu-launched
protected lobby productization milestone, completed shared Web/WASM lobby
parity, and a completed composable-world-presentation and live-preview actor
milestone through Slice 7, completed Tactical 185 observer adoption, plus
completed configurable catalog-world destinations, supported-arrival
acceptance, and completed lobby-content simplification. Transient authored
lobby bootstrap,
shared immutable second-slot terrain and actor resources, per-world actor
presentation/cache state, slot-targeted startup/readiness/activation/swap policy,
ordinary catalog/app-private destination persistence, one compiler broker,
dual browser runtime ownership, production desktop/mobile preview and activation,
persistence, adversarial lifecycle rejection, and final performance/platform
closeout are landed.**
`McloneSceneHost` now submits a bounded region of its retained persistent local
world as placed opaque/cutout geometry in the active world's mono, per-eye, and
multiview frame ordering. It owns one direct active `DrawableWorldSlot`, one
optional detached standby, and exactly one mutually exclusive runtime-only
`WorldGate` or `EmbeddedWorldPreview`. The two
integrated hosts remain concurrent and independently drawable. The standby
acknowledges its seed-dependent spawn, resolves a terrain-relative endpoint,
and incrementally admits its retained CPU seed through the normal upload path.
It becomes `Switchable` only after mapped exit terrain and renderer topology
are ready. A host-scoped `OpaqueWorldGateRenderer` draws the active endpoint as
a two-sided, no-blend, reversed-Z depth-writing surface in mono, per-eye stereo,
and full-frame multiview. Signed hysteresis uses the mono eye or stereo eye
midpoint; warming/failed gates clamp locomotion, and a successful crossing
atomically exchanges the complete slots before drawing the frame. The previous
active remains the ready return world. Deterministic walking and synthetic
stereo A-to-B-to-A smokes prove next-frame drawing with no runtime
reconstruction, switch-boundary upload, or lazy renderer creation. The retained
preview now also proves a bounded authoritative mutation through persistence
and globally orders its ocean with active-world water in physical composition
space. Flat and XR Use target the placed preview through a scene-owned ray,
close a short shared blink, exchange the complete slots under full cover, and
retarget the old active world onto the paired return table. Deterministic flat
and stereo A-to-B-to-A receipts prove that the first uncovered frame already
draws the destination with no boundary compile, upload, runtime creation, or
renderer materialization. There is still no N-world registry or remote
preview, but simultaneous shared-depth terrain composition and local activation
are now concrete. The first product surface is also live: shared native and web
profiles expose `Enter Lobby`, and separate tokened primary/destination starts
make a fresh transient protected lobby playable before its persistent
destination is ready. Shared Rust constructs the authored lobby in an ordinary
memory store, selects a recent compatible catalog world or an app-private
fallback, and owns external-runtime readiness plus complete-slot activation.
The destination renderer shell is prepared under startup cover and appears
through the existing exact readiness gate. Flat, synthetic-stereo, desktop
browser, and mobile browser lanes complete A-to-B-to-A activation. One host
compiler Worker serves both browser world sessions, while both draw stores
share the compatible atlas/pipeline owner. A server-owned behavior profile
travels with each slot: the lobby authoritatively denies player break/place
while the destination remains mutable, including after complete-slot
exchanges. Browser IndexedDB preserves destination edits across complete
two-runtime shutdown and relaunch. Shared operation tokens and world identity
reject late starts; deterministic receipts cover Back during primary startup,
Quit during destination startup, destination-only failure, asset replacement,
renderer-resource rebuild, and visibility suspension/resume. TypeScript starts
Workers and executes storage/transport mechanics from Rust-owned tickets; it
contains no lobby content or sequencing state machine.
Separately,
`mclone-app-runtime/tests/dual_integrated_hosts.rs` retains two native
integrated-server runners, connection adapters, and client replicas at once.
Tactical 174 now also classifies all 81 flattened scene-host fields, locks the
current startup/reset/asset-epoch contracts, and records renderer-shell,
thread/cadence, process-memory, offscreen, frame-budget, and synthetic-stereo
baselines. An optional standby-only cadence removes the measured idle CPU
increment on the current M4 Pro while restoring ordinary cadence as each world
becomes active. A maintainer manually confirmed the interactive gate works in
both directions. Contemporaneous release A/B runs find no significant
single-world regression. This records the larger vision, the fidelity ladder,
concrete engine seams, and the warm-world burn-down so future slices do not
have to re-derive them. The bounded first implementation milestone is
[`174-warm-world-hot-swap.md`](../tactical/174-warm-world-hot-swap.md): retain
two drawable local worlds and switch through an opaque gate, with an explicit
stop before simultaneous rendering. The next bounded composition milestone is
[`175-live-hosted-world-diorama.md`](../tactical/175-live-hosted-world-diorama.md):
draw one live local or remote hosted region as scaled geometry on a block-built
table, then use only a simple blink around the already-proven activation. The
completed first productization milestone is
[`177-menu-launched-protected-lobby-scenario.md`](../tactical/177-menu-launched-protected-lobby-scenario.md):
enter the fixed local scenario from the shared title menu before broadening the
destination source. Its no-scenario closeout remains faster than its pre-change
baseline, while scenario-on renderer, CPU, memory, and thread costs remain
separately recorded. The completed parity follow-up is
[`178-shared-web-lobby-scenario-parity.md`](../tactical/178-shared-web-lobby-scenario-parity.md):
replace native-shaped managed-content and slot-start ownership with portable
contracts, add only IndexedDB/Worker adapters, share compatible immutable
renderer resources, and make the same scenario actionable on web without
regressing the one-world path. Its final native direct-path median is
2.470/4.262 ms average/P95 versus the 2.492/4.338 ms control. Browser
feature-off remains within the attributed noise envelope. Desktop and 2x
CPU-throttled mobile scenario receipts reached the lobby in 306/315 ms and the
preview 1.234/1.602 s later, with about 10.3 ms maximum measured frame gaps.

The completed renderer milestone is
[`179-composable-world-presentation-and-live-preview-actors.md`](../tactical/179-composable-world-presentation-and-live-preview-actors.md).
Its first six slices characterized actor ownership, normalized the
existing placed-terrain path around distinct placement, source-bounds, and
composition-clip contracts, and landed opt-in portable half-space terrain
clipping. One shared renderer fixture now draws complementary `x <= 0` and
`x >= 0` sources with solid, cutout, and translucent phases through native and
production browser WebGPU; inspected pixels show zero wrong-side leakage. The
ordinary and placed-unclipped shaders stay unchanged, while clipped pipeline
and uniform topology remains lazy. Mutable actor uniforms and mesh caches now
move with each `DrawableWorldSlot`, while compatible immutable atlases,
figures, layouts, and pipeline topology remain shared. A standby actor state is
created only when that retained slot becomes switchable; native and browser
receipts record two immutable owners, 8,320 known atlas bytes, and 1,345,024
bytes of per-slot mutable state. Exact active actor pixels and the direct
single-world fast path remain unchanged within the measured noise envelope.
The actor renderer now also consumes the same placement, source-selection,
composition clipping, fog, and frustum semantics as terrain through opt-in
mono/per-eye and full-frame multiview paths. One shared native/browser fixture
draws a cow, chicken figure, item, remote-player figure, and source-local
player figure from two independent world-local caches with source, clip, and
frustum rejection before mesh preparation. Production browser WebGPU uses the
same Rust fixture through a target-only adapter. At the Tactical 179
checkpoint, the product scene also submitted the retained slot's synthetic
source-local full-body player. Tactical 185 Slice 7 removed that body when the
standby connection became a real observer; current previews submit only
destination entities and real remote players after opaque/cutout terrain and
before globally ordered translucency. Native flat/stereo and production
browser desktop/mobile captures show correctly scaled, source-lit actors with
shared depth and zero source-local preview players. The direct actor path
remains byte-identical and within its measured noise envelope, while preview
diagnostics separately
expose selection, rejection, cache, GPU-capacity, and placed-pipeline costs.
Persistent destination entity records load, tick, publish, persist, and move
through ordinary authority and AI. Native and production browser
desktop/mobile receipts observe stable world-qualified actor identity and
source/composed displacement, then publish movement to a visible frame in one
frame. A-to-B-to-A and full reopen retain destination actors without
duplication, while TypeScript remains only a storage/Worker adapter. The native
and browser direct paths remain within their measured noise envelopes.
Tactical 179 Slice 7 closed the joined-player receipt at that
historical checkpoint: shared integrated-server tracking pairs local and
dedicated players symmetrically,
and a shared Rust-authored destination script drives one
ordinary dedicated player through normal appearance, teleport, and
`MovePlayer` admission. Native and production browser desktop/mobile receipts
draw the two creatures, remote upright-bear player, and the then-retained
source-local player together. The remote player moves 0.725 source blocks and
exactly 0.090625 composition blocks at 1:8, advances its walk animation, retains source
light, and becomes visible in one frame. A-to-B-to-A and full browser shutdown/
relaunch retain world-qualified identity without duplication. TypeScript only
forwards the enable bit and shared diagnostic facts; it owns no pose, protocol
update, or visibility implementation. Feature-on receipts characterize the
CPU-baked actor mesh upload storm while the final native/browser feature-off
controls, direct image hash, Android/Android XR packaging, flat AVD, synthetic
stereo, and desktop XR compile lanes remain accepted.

The completed destination milestone is
[`180-configurable-lobby-world-destinations.md`](../tactical/180-configurable-lobby-world-destinations.md).
The product no longer teleports from the lobby into the single authored island
fixture. Shared policy selects the most recently actively played compatible
catalog world without opening it or changing its recency during preview
warmup. A successful outbound complete-slot activation records play exactly
once; empty or incompatible catalogs use a persistent app-private ordinary
Overworld with curated seed `12345`. The old island remains only a retained
deterministic fixture.

Preview size is now data: an inclusive rectangular chunk/section region
resolves around the destination runtime's accepted entry independently of
table placement and activation. The product default executes 2x2 and the same
path is acceptance-tested at 4x4, with adjustable bounds retained in the
path-free launch contract. Only that crop is composed on the table; selection
still exchanges the already-running full destination world, whose normal
chunk tracking remains unbounded when active.

Arrival admission now requires loaded body clearance and loaded solid support.
The accepted entry, immediate post-swap camera, first uncovered frame, and
delayed stability sample are retained in diagnostics. The browser fall was a
browser adapter relocating the camera after shared accepted-pose reconcile;
that relocation is gone. A later catalog acceptance lane also found renderer
shell preparation could race primary-lobby replacement, so shared scene policy
now prepares the shell at post-lobby destination start. Neither correction
forks gameplay or rendering by platform. Native flat/stereo and production
browser WebGPU desktop/mobile lanes prove 2x2/4x4 app-private and catalog
journeys, recency semantics, A-to-B-to-A, persistence, and stable supported
landing.
Direct single-world pixels remain byte-identical and native/browser controls
remain within the accepted performance envelope.

Boundary-face resolution is now the next bounded renderer problem. Collision
and one-active-authority handoff remain separate later split-world work.

Tactical 174's available-lane lifecycle closeout now flushes both retained
worlds, drops a standby before asset-epoch or device-resource replacement, and
proves both persistent roots survive independent edits. Capable-device
multiview execution remains a named receipt gap. Last reconciled: 2026-07-20
(Tacticals 178, 179, 180, 185, and 201 complete; the current Metal adapter did
not expose `MULTIVIEW`, and no headset was attached for a new real-device
scenario receipt).

## Motivation

For XR immersion, spawning into a menu is weak; spawning into a built room is
strong. From there the idea grows: a room whose tabletop holds a live preview
of another world (zoo-diorama style), a command-center that is really a
worldgen/seed console, a texture room that exposes engine internals as
manipulable objects, and eventually a portal into a *live, network-hosted*
world — "the nether-portal idea, but better." The signature effect is diving at
the diorama and continuously shrinking, falling from its sky into the real
world.

The unifying primitive under all of it is: **render a bounded source region's
content with a second placement inside the physical frame.** The source may be
a different world/runtime or the active world itself. Everything else is a
flavor of that.

## Two techniques (pick per feature)

These are genuinely different, with different physics. Do not conflate them.

1. **RTT / portal-to-texture** — world B renders in its own pass with its *own*
   depth buffer, is flattened to a texture, and that texture is drawn on a
   surface. The worlds never share space. Correct for a **window into a
   disconnected place** (a palantir on a wall showing a remote server). Cannot
   give occlusion against world A (no hand-in-front, no walk-around); it is a
   picture. Requires new GPU work the engine lacks today: a sample-able
   offscreen target and a scene-color-sampling composite shader (see Seams).

2. **Shared-coordinate embedding** — world B's *actual geometry* is transformed
   (scale + offset, e.g. ÷16 onto a table) and submitted into the **same render
   pass, same depth buffer, same view-projection** as world A. Correct for
   anything that shares physical space: diorama, seed explorer, shrink-and-fall.
   Strictly more physical *and* less bespoke shader code than RTT — no offscreen
   target, no sample-back. It is "draw more chunk sections with a different
   model transform."

Rule of thumb: same physical space → embed; disconnected window → RTT. A
"reach-through" palantir is embedding, not RTT.

Shared-coordinate geometry is the primary architectural target, especially in
XR. RTT remains a deliberately limited presentation tool for a literal screen
or disconnected window; it must not become the representation of a spatial
world, diorama, walkable boundary, or traversable portal. Those cases require
real geometry submitted into the eye targets with correct stereo and depth.

## Fidelity ladder (a world need not be a full live runtime)

Each embedded world picks a tier. Cheaper tiers exist precisely so low-end and
XR budgets can host the effect, and so most instances cost almost nothing.

- **T0 — baked static mesh.** Snapshot world B's chunks, mesh once, draw with a
  model transform. No second runtime, no simulation, no updates. Cheapest;
  ideal for "last-visited world on the table" and for degrading higher tiers
  under budget pressure. Self-lights correctly for free (see below).
- **T1 — live-local read-only.** A local worldgen preview: run
  `NoiseBasedChunkGenerator` for a small radius on a scrubbable seed, re-mesh on
  change. No server, no network, no tick. This is the **seed explorer / worldgen
  console** — the dials are the real noise parameters.
- **T2 — live remote spectator.** A second, read-only runtime observes a remote
  server around a point of interest and renders its updates live. This is the
  **palantir into a network world**. It is a target tier, not a claim that the
  current command-coupled wire protocol already supports observer
  subscriptions; that gap is recorded under Concrete Seams.

- **T3 — live joined warm world.** A complete second local or remote runtime is
  retained while another world remains primary. It continues to pump updates
  and hold a ready client replica, but only the primary world receives physical
  player movement, interactions, and authoritative camera corrections. This is
  the foundation for quick world switching, a warm Nether/sky world, and later
  traversable boundaries. T3 has two distinct future levels: one-active-
  authority handoff is tractable; simultaneous or federated authority requires
  explicit server cooperation and state-transfer rules.

T0-T2 avoid the truly hard thing (two *authoritative* simulations). T3 permits
multiple live joined simulations but initially keeps exactly one authority for
the physical player. Inventory, health, velocity, identity, and persistence do
not cross unrelated hosts merely because both connections are warm.

## Immediate Foundation: Warm Worlds Before Composition

The first useful target does not need multi-world pixels. Keep two integrated
hosts alive, warm both client replicas, select one as primary, and switch which
already-ready runtime drives the existing single-world renderer. This attacks
vanilla-style abrupt dimension/world loading without first taking on portal
clipping or cross-world physics.

The first proof deliberately lives below `McloneSceneHost`: two real
`NativeIntegratedServerRunner`s feed two independent
`IntegratedRunnerConnection`s and `SingleViewRuntime`s concurrently. The test
uses different seeds and disjoint chunk views, waits for both hosts to become
idle/warm, verifies neither replica receives the other's chunks, changes the
logical active selection without reconstructing either runtime, and relies on
ordinary owned-value drop for independent shutdown.

This proves instanceability and isolation below the scene owner. Slice 6 keeps
the production scene's direct active `DrawableWorldSlot` plus one
optional detached slot with independent runtime, camera, lifecycle, storage,
terrain draw/upload state, and provisional endpoint. Persistent-root isolation,
authoritative pose acknowledgement, budgeted standby GPU admission, and exact
entry/topology readiness are proven. The scene command now swaps both complete
slots, and the paired gate invokes that command from visual-midpoint crossing.

The representative launch smoke (`12345` active, `67890` standby, render
distance 2) reaches GPU `Switchable` readiness in about 0.69 seconds flat and
0.84 seconds synthetic stereo. Its 64 initial lifecycle items drain in exactly
64 capped GPU advances with no false compile-grant release; the latest flat
run measured 0.670 ms worst GPU contribution, 23 GPU sections, 155,526 indices,
and no queue at readiness. The duplicate empty renderer shell still costs about
15 ms before presentation and the CPU startup seed is roughly 4.5 MiB. A
direct packed-section block lookup was required to make the bounded endpoint
search honest: the first
whole-section-unpacking implementation cost about 300 ms, while the corrected
allocation-free search costs about 1–1.5 ms. The latest no-request release batch
measured 2.676 ms average / 4.569 ms P95; a same-machine untouched-Slice-3
control measured 2.679/4.650 ms, clearing Slice 4 despite system-level drift
from the older clean anchors. These are desktop-host receipts. A
`MULTIVIEW`-capable device will materialize the lazy standby renderer before
readiness, but the current Mac cannot supply that final device receipt.

The Slice 6 walking smoke maps to each seed's paired surface-relative exit and
preserves the sequence `1/12345 -> 2/67890 -> 1/12345`. Both atomic exchanges
do no renderer construction or switch-boundary compile/upload work and draw the
destination on selected-world frame 1. `a-gate.png`, `b-first.png`,
`b-gate.png`, and `a-return.png` are retained under `/tmp`; the two endpoint
captures each contain 96.6% exact opaque-gate pixels plus occluding surrounding
world pixels, and A/B differ in 98.5% of pixels. Synthetic stereo crosses from
the eye midpoint and keeps both eyes on the same selected world. The Slice 6
no-standby release batch measured 2.525 ms median average / 4.400 ms median P95
with 6.5%/9.1% within-batch ranges, zero missed budgets, and zero accounting
violations. Those medians are only 0.8%/1.2% above the accepted Slice 5 batch,
clearing the single-world performance invariant.

The Slice 7 measurement harness now takes paired same-process samples after the
active world is idle and again after the detached world is switchable. Five
settled default-cadence runs measured a median 5.63 percentage-point increment
of one CPU core, six additional managed threads, and 36.5 MiB additional RSS.
Retained CPU seed and conservatively estimated GPU terrain storage ranged from
3.80 to 5.07 MiB as readiness retained 48-80 startup sections; the duplicate
renderer separately owns at least an 8 MiB base atlas plus mips, pipelines, and
driver-private storage. Applying the existing runtime cadence at `5/5/5` only
while standby reduced the five-run median paired CPU increment to measurement
noise (-0.15 points) without changing thread or memory ownership. Both gate
directions prove the selected destination returns to its authored active
cadence and the demoted source receives the standby cadence.

The same checkpoint records a 593.9 ms default median initial warm latency,
34.4 ms renderer-shell creation, 1.92 ms total startup-pump CPU, 3.39 ms total
GPU-admission CPU, and worst individual startup/GPU advances of 1.25/0.33 ms.
Atomic slot exchanges remain 0.0046-0.0185 ms with and without cadence changes
and do no boundary compile, upload, renderer construction, or runtime creation.
Five-run candidate/base batches plus three strictly interleaved pairs found no
single-world release regression; the interleaved candidate medians were 2.1%
lower in average frame cost and 3.4% lower at P95. These are desktop process
receipts, not portable hardware budgets. Available-lane lifecycle closeout now
attempts persistence flushes for both slots, cancels and drops old-epoch or
old-device standby ownership before replacement, and verifies both persistent
roots reopen with their independent edits. A combined asset replacement smoke
observed the old standby become non-switchable and its gate close before active
epochs `0 -> 1 -> 2`. Capable-device multiview execution remains open.

### Historical bounded milestone: one live hosted diorama

The next proof deliberately drops the staged shrink/fall transition from its
critical path. While arbitrary world A remains active, a bounded section-aligned
region from independently hosted world B is rebased around a source anchor,
scaled provisionally by `1/16`, and submitted into A's physical mono/stereo
color and depth targets above a block-built table. B may be a local integrated
runtime or an ordinary joined remote dedicated runtime. It keeps a fixed chunk
interest center, pumps live updates, and prepares terrain under background
budgets, but receives no physical camera, movement, interaction, actor, audio,
future distant-terrain, or underwater authority.

The deterministic first fixture uses two persistence-backed authored worlds:
a grass/table world A and a grass/stone-island world B. A new server-owned
`AuthoredOnly` generation profile makes true storage misses produce void/air
through the normal chunk-status/light/publication path instead of silently
running overworld generation. This is not presented as a partial vanilla
Superflat port. A general flat generator remains separate and should start from
Java 1.17.1 `FlatLevelSource` if product world creation later needs it.

Placed terrain gets separate mono/per-eye/multiview shader/pipeline variants so
the direct one-world shader and draw path remain unchanged. The first manual
checkpoint is opaque/cutout local B geometry with shared-depth table occlusion.
Later slices prove a live B block mutation, bounded-region enforcement, water
and cross-world translucent ordering, simple blink activation/return through
the existing whole-slot exchange, and a remote dedicated B. The local parts
of that ladder are now complete through activation; remote B remains next.
There is still
exactly one active slot and one optional preview; this is not an N-world
registry. The authored fixture's void ring/table rim closes its edge. Sealed
cuts through
arbitrary B terrain still require separate boundary-aware preview meshing.

Slice 0 locks the one-world fast path before that work begins. Ordinary terrain
still uses 128-byte per-view uniforms and unplaced mono/multiview shaders; mono,
per-eye, and multiview scene paths submit only the active draw store. Stored
light-status chunks now have an explicit test proving they publish without a
worldgen job, while a true miss still enters the default overworld feature
path. Fresh flat/stereo captures were inspected, and the accepted five-run
release batch measured 2.659 ms median average and 4.252 ms median P95 with
9.1%/5.6% within-batch spread, zero over-budget frames, and zero accounting
violations. That is the comparison anchor for every later no-preview gate.

Slice 1 lands the server/content half of that shape. The serializable
`WorldGenerationProfile` defaults every old world and host to `Overworld` while
`AuthoredOnly` maps a true persistence miss to bounded void through the normal
status/light/publication/persistence pipeline with no overworld feature job.
The profile crosses native, dedicated, catalog/session, CLI/query, and browser
Worker/IndexedDB startup. Authored entry uses the configured chunk hint rather
than an unrelated seed-derived overworld biome search. A shared guarded fixture
builder now creates the persistent grass/table A and grass/stone-island B roots,
and ordinary native-runner and dedicated-TCP tests prove their spawn, lighting,
mutation/restart, missing-void, and zero-worldgen contracts. Independent table
and island pixels were inspected. The accepted no-preview release batch is
2.510 ms median average and 4.331 ms median P95, respectively 5.6% lower and
1.9% higher than Slice 0, with stable spread and no budget/accounting failure.
Slice 2 lands that static placed-terrain proof. A validated `f64`
`WorldPlacement` rebases source coordinates around an anchor before a positive
uniform scale, and `EmbeddedChunkRegion` filters stable prepared records by
inclusive chunk/section bounds. Opt-in solid/cutout mono, per-eye, and
full-frame multiview pipelines transform both clip and fog positions into
composition space without changing the direct 128-byte terrain uniforms or
ordinary shaders. Placed culling uses the shader's explicit rebase order even
at 30-million-block source anchors. A renderer-only shared-depth fixture proves
both occlusion directions (6,004 A-over-B pixels and 13,340 B-over-A pixels)
and stereo parallax (10,383 differing pixels); its mono and side-by-side images
were inspected. The current Mac lacks the multiview feature, so capable-device
execution remains open while the full-frame path and eager materialization
contract are present. Exact active-only flat/stereo hashes are unchanged, and
the accepted no-preview batch remains inside the Slice 0 performance gate.

Slice 3 connects that renderer to the retained persistent local slot. One
optional scene-owned `EmbeddedWorldPreview` publishes only after bounded source
records, source-anchor GPU/traversal coverage, asset epoch, and eager topology
are coherent. Mono, per-eye, and multiview composition insert B opaque/cutout
after A opaque/cutout and before A's actors/translucent phase, while B retains
no physical authority or second sky/distant-terrain/overlay stack. The launch
diagnostic exposes B root, region, anchors, scale, and standby cadence without
allocating preview state in the ordinary path. The deterministic smoke draws
two B sections / 5,112 indices in three table views and both stereo eyes;
151,898 pixels differ between eyes, and the requested `5/5/5` standby cadence
remains applied. The accepted no-preview medians are 2.724 ms average and 4.235
ms P95, still inside the Slice 0 gate. The first manual review rejected the
oversized elevated 5-by-5 table and exposed coplanar grass/brick depth fighting.
The fixture now uses a four-block ground-level display and maps the miniature
1/32 block above its top; regenerated mono/stereo views show clean separation.
Slice 4 can add mutation and budget instrumentation without preserving those
provisional fixture mistakes.

Slice 4 proves the preview remains live under bounded preparation. B keeps its
fixed authored interest while compile priority inverse-maps and clamps the
physical camera into B's source region. Per-preview diagnostics now separate
runtime poll, compile, acceptance, upload, placed cull, and placed draw, with
named one-item/one-request background grants and CPU/GPU retained-byte facts.
A smoke-only authoritative break progresses asynchronously through B at its
throttled cadence, changes 31 miniature pixels through two affected section
submissions/one accepted result/two uploads, does not change A, and remains air
after B's SQLite store is reopened. Non-authored hard boundaries warn that
canonical neighbor-culled faces may be exposed rather than silently inventing
preview cap meshes.

The 600-second post-mutation orbit held three bounded records, fixed interest
`(0,0)`, and zero pending compile jobs, upload items/bytes, or out-of-region
submissions across 6,000 frames while B-local priority changed around the
table. The accepted five-run direct single-world release batch is 2.388 ms
median average and 3.935 ms median P95 with at most 7.1%/5.3% deviation, zero
over-budget frames, and zero accounting violations. Slice 5 can now add authored
water and cross-world translucent ordering without reopening live-update or
background-bounds ownership.

Slice 5 adds that authored ocean and closes the first composition-order gap.
World A uses a bounded flat grass pad with separate front/back water-pool
sections; World B persists a two-block-deep ocean around its island. The scene
qualifies neutral direct/placed section records with world identity, sorts one
back-to-front list in physical composition space, and projects compact source
ordinals into the renderer. Frames now draw all A/B opaque and cutout terrain,
active A actors, then globally ordered translucent runs while retaining the
ordinary no-preview call path. Mono uses its physical eye, stereo shares one
eye-midpoint order between both eyes, and multiview consumes the same ordering
contract. Underwater state remains strictly active-world-owned.

Front/behind witnesses contain two A and two B records with two source switches
and reverse the exact A endpoint section keys when the camera crosses the
table. Synthetic stereo contains both sources and 159,119 differing eye pixels;
adding B changes 50,508 composition pixels, while the small live mutation
changes five miniature pixels and survives store reopen. A fresh 600-second
soak finishes with drained bounded queues. Five no-preview Slice 5 runs are
effectively identical to an isolated clean worktree at the exact pre-slice
`HEAD`: 2.508/4.363 ms median average/P95 versus 2.507/4.373 ms, with zero
over-budget or accounting failures. Intersecting or coplanar translucent
surfaces across worlds remain explicitly unsupported; section-level sorting is
the bounded contract for this non-intersecting fixture. Slice 6 can now add the
simple blink activation and return interaction.

### Same-world previews and non-recursive composition

Tactical 175 deliberately proves the harder ownership boundary first: a
bounded region from independently hosted world B appears inside active world A.
The placed-terrain contract should later permit A itself as the source without
requiring a second runtime or drawable slot. A same-world preview may show a
distant active-world location, the player's nearby settlement, or even the
exact region containing the physical table. Live mutations then naturally
appear in both the direct full-scale draw and the placed miniature draw.

Source identity is independent of fidelity tier. A future preview source may
be expressed provisionally as one of:

```text
PreviewSource
  ActiveWorld { source_anchor, bounded_region }
  StandbyWorld { world_id, source_anchor, bounded_region }
  BakedSnapshot { snapshot_id, bounded_region }
```

This does not broaden Tactical 175. Its deterministic A/B fixture, activation
target, and acceptance gates remain unchanged; active-world sourcing is a
follow-up reuse of the placed-terrain path after the hosted-world milestone is
closed.

**Embedded previews are non-recursive.** Physical-frame composition may submit
one preview, but preview drawing submits terrain leaves directly and must never
invoke scene composition or discover further `EmbeddedWorldPreview`s. In
particular, if an active-world source region contains the physical block-built
table, the miniature may contain that table's ordinary block geometry, but the
miniature table is empty: it does not contain another live miniature.

The composition-depth invariant is:

```text
depth 0: direct active world + at most the configured physical-frame preview
depth 1: placed terrain/liquid leaf submissions only
depth 2+: prohibited
```

This prohibition applies even when recursive rendering would be visually
possible. Recursion is not required for a tabletop model, seed explorer, or
same-world location preview; it complicates visibility, budgets, translucent
ordering, and XR cost without advancing those experiences. A future portal
renderer would require a separate explicit design decision and recursion
budget rather than weakening this invariant implicitly.

The near-term product shape is:

```text
McloneSceneHost
  one physical input/UI presentation
  one direct single-world render path
  WarmWorldOwner
    active DrawableWorldSlot -> existing renderer
    optional standby DrawableWorldSlot -> prepare under background budgets
      optional EmbeddedWorldPreview -> placed bounded terrain submission
    future N-world registry only after the bounded milestone needs it
```

`SceneSessionRuntime` should remain the one-world leaf. A new host-level
owner should retain the active and optional standby leaf runtimes; a later
registry can generalize the same slot. Turning `SceneSessionRuntime` itself into
a bag of worlds would mix connection mechanics with composition and selection
policy.

Warm switching must define what transfers. A first local proof may preserve
camera pose and map it into the destination spawn while leaving each world's
inventory/player state independent. Seamless dimension-like travel eventually
needs an explicit transfer record for position, orientation, velocity,
inventory, health, appearance, and authority acknowledgements.

## N worlds, not two

Nothing about the design is limited to two. The host holds a small **registry**
of embedded worlds, each with a transform and a fidelity tier, under an explicit
**count/perf budget** (tighter on low-end and XR). Graceful degradation:
downgrade live tiers toward T0, then reduce count, before dropping frames. The
budget owner should live with the frame-budget machinery, not ad hoc.

## What is free vs. what is net-new

Confirmed against the renderer (see Seams for file:line):

- **Free from shared-coordinate embedding:**
  - *Occlusion* between worlds — single shared depth buffer, correct by
    construction (your hand/wall/table occludes the tiny world and vice versa).
  - *Self-lighting* — light is baked per-vertex (`packed_light`, block + sky)
    and colored in-shader independent of where geometry sits, so world B lights
    itself correctly at any position/scale with zero work.
- **Net-new, and orthogonal to nesting:**
  - *Cross-world light and shadow* ("your shadow falls across the little
    world"). The engine has **no dynamic shadow system at all** — lighting is
    Minecraft baked lightmaps, faithful to 1.17.1. So world A's torch will not
    spill into world B and no shadow crosses the boundary. This is not a
    boundary to bridge; it is a dynamic shadow-mapping subsystem to *invent*,
    and a deliberate vanilla-parity divergence. Good news: it is independent of
    nesting — build dynamic shadows once for either world and, because geometry
    already shares coordinates and depth, they cross the boundary naturally.
    Track shadow work as its own concern, not a sub-task of this one.

## Concrete seams

- **Embed transform (cheap hook).** Terrain vertices are pre-baked to absolute
  world coords; the vertex shader multiplies by *only* view-projection
  (`native/crates/mclone-render/src/shaders/chunk_textured.wgsl:105`). There is
  **no model matrix today**. Add a `model` mat4 (or scale+offset) to the
  `Uniforms` block, change that line to `view_projection * model * position`,
  and bind a distinct `model` for world-B section draws (the draw loop already
  issues per-section `draw_indexed`). **No re-meshing** — world B meshes carry
  their own world coords, so `model = translate * scale` composes. Widen the
  fixed 128-byte uniform layout to fit the mat4 (`chunk.rs:64-68`,
  `mclone-render/src/uniform.rs`), and add the twin to the multiview shader
  (`chunk_textured_multiview.wgsl`) so XR stereo is correct.
  This is enough only for an unbounded opaque terrain proof. The shader must
  also publish the transformed position as `world_position`; otherwise fog is
  computed from world B's original coordinates. CPU culling, traversal,
  distance ordering, translucent sorting, chunk interest, and any future
  distant terrain currently use untransformed section/camera coordinates. They
  need a world-local camera derived with the inverse placement transform, or
  transformed bounds. Actors, particles, outlines, world UI, debug geometry,
  and audio need the same placement contract before this is a complete
  embedded world.
- **Shared depth / occlusion:** one `depth_view` per pass
  (`mclone-render/src/chunk.rs:376`, bound at `:2441` opaque / `:2539`
  translucent) — second-world draws added to the pass occlude for free.
  Separate render passes may load the same color/depth attachments and preserve
  opaque occlusion, but the compositor must own global phase ordering: sky,
  every world's opaque/cutout geometry, actors, then translucency. Current
  translucent sorting is per terrain store and is not globally correct for
  overlapping worlds.
- **Lighting is baked:** vertex carries `packed_light: u32`
  (`mclone-mesh/src/data.rs:54-60`), unpacked in
  `chunk_textured.wgsl:109-112`; CPU reference math in
  `mclone-render/src/light_texture.rs`. No shadow-map / depth-from-light pass
  exists anywhere in `mclone-render` or `mclone-xr-graphics`.
- **RTT primitive (for the window flavor) is new:** the only offscreen target is
  the headless-capture `OffscreenTarget`
  (`mclone-render/src/headless.rs:1042-1075`), created `COPY_SRC`-only for PNG
  readback — not `TEXTURE_BINDING`, so no shader can sample it. A palantir needs
  that flag plus a scene-color-sampling composite pass that does not exist
  today.
- **Second runtime is instanceable, not a rewrite:** ownership is encapsulated
  with no globals. `McloneSceneHost` now holds one concrete
  `active_world: DrawableWorldSlot`, one optional detached standby, and one
  session coordinator (`mclone-scene/src/lib.rs`), while
  `EngineRenderSession`/`SingleViewRuntime` each cleanly wrap one
  `ClientRuntime`. Keep those single-world leaf types. An N-world collection
  waits until the bounded milestone proves it is needed. Placement,
  lifecycle/readiness, authority role, and budget remain explicit additions;
  a plain `Vec<SceneSessionRuntime>` would still be insufficient.
- **Visibility is separate from placement:** a general submission needs a
  world-to-composition transform plus an unbounded, half-space, convex-volume,
  or portal-aperture visibility policy. A model matrix cannot implement the
  `x=0` split or a portal by itself. Current `Depth32Float` targets have no
  stencil component, so stencil portals would also require a target-format
  change; clip-volume techniques are another option.
- **Cut boundaries affect meshing:** fragment clipping can expose holes because
  the mesher may have omitted a face against a same-world neighbor that is
  later clipped away. Axis-aligned seams need boundary-aware remeshing or
  retained cap faces. Arbitrary moving cuts require true geometry clipping and
  cap generation or a deliberately constrained visual contract. The earlier
  "no re-meshing" observation applies to unbounded transformed dioramas, not
  clipped worlds.
- **Aggregated identities need a namespace:** independent replicas can retain
  plain `RenderSectionKey`, `EntityId`, and `RemotePlayerId`. Anything collected
  across worlds must qualify them with a client-side `WorldInstanceId`. The
  network protocol does not need that id when the connection/slot already
  supplies the namespace.
- **Remote spectator is a target, not current wire behavior:** local integrated
  runners now have a real non-player observer role, but native remote updates
  are still paired with commands rather than an independent observer
  subscription/server-push stream. A remote T2 needs an observer protocol or
  an ordinary joined connection that deliberately maintains view/keepalive
  commands. `ClientRuntime` alone is not sufficient; the connection, update
  pump, render session, compiler, and uploads must remain alive too. See
  `docs/session-network-architecture.md` and
  `docs/topics/multiplayer-networking.md`.
- **Fixed lobby/fixture room (no overworld generation):** the chunk pipeline is
  persistence-first, but current true misses are hardwired into overworld
  feature jobs. Tactical 175 adds a server-owned `AuthoredOnly` miss policy and
  a shared fixture builder that writes ordinary `ChunkRecord`s through
  `WorldStore`; runtime edits still use the authoritative
  `ChunkScheduler::set_block_at_world` path.
- **Disable block destruction (lobby mode):** one authoritative choke point —
  gate `handle_player_action_for_target` / `set_block_debug` in
  `mclone-server/src/integrated.rs`. No game-mode system exists yet; this would
  be the first per-world flag, natural to sit beside `movement_mode` in
  `StartupSceneOptions`. The client option may request a lobby profile, but the
  behavior must reach an authoritative shared server policy so local and
  dedicated hosts cannot diverge or be bypassed by a client.

## Runtime Ownership Target

The likely shared shape is intentionally above the existing leaf runtime:

```rust
struct WorldSlot {
    id: WorldInstanceId,
    runtime: SceneSessionRuntime,
    terrain: TexturedSectionDrawResources,
    traversal: TraversalReadySectionCache,
    uploads: RenderSectionUploadCoordinator,
    placement: WorldPlacement,
    lifecycle: WorldLifecycle,
    authority: PlayerAuthorityRole,
    budget: WorldBudgetClass,
}

struct WorldComposition {
    worlds: WorldRegistry<WorldSlot>,
    primary: WorldInstanceId,
    interaction_target: WorldInstanceId,
    transition: Option<WorldTransition>,
}
```

The composition camera is the physical flat/XR camera. Each slot receives an
inverse-transformed local camera for culling, traversal, interest, block/fluid
queries, and interactions. A portal derives a distinct destination view for
each eye; a scale/offset diorama normally reuses the physical view after model
placement. Per-eye mutable uniforms must remain distinct in both cases.

Only the primary authority consumes movement and interaction commands and may
reconcile the physical camera. Warm worlds continue to pump ordered updates and
may retain requested chunk views. If two unrelated servers both correct the
same camera, they will fight; selection and handoff are therefore explicit
runtime policy, not a rendering detail.

## Single-World Performance Invariant

The feature is not allowed to tax ordinary play significantly. The normal case
must stay structurally equivalent to today's direct path:

```text
one WorldSlot -> existing poll/cull/compile/upload/draw path
```

Composition is an optional outer scheduler used only when more than one slot is
warm or visible. In particular:

- do not add world-id hashing to per-section maps or hot draw loops;
- do not make every fragment evaluate dynamic portal/clip logic;
- keep specialized unbounded and clipped pipeline variants;
- keep `SceneSessionRuntime`, replicas, and render caches single-world;
- avoid per-frame allocation or virtual dispatch in the one-world path;
- give background worlds explicit update/compile/upload quotas rather than a
  full independent frame budget;
- share immutable assets, atlases, and worker capacity where practical, while
  keeping per-world dirty/cache state isolated;
- retain before/after frame-accounting and offscreen/XR performance evidence
  for every extraction that touches the existing hot path.

Tactical 174 fixes the flat release comparison anchor from five clean 240-frame
120 Hz frame-budget runs: 2.346 ms median average and 3.916 ms median p95. Their
within-batch ranges were 6.2% and 5.2% of the medians, respectively. One
isolated 9.765 ms maximum is retained in the evidence; the other maxima were
4.148–4.467 ms, and no run had an accounting violation. Ownership and
frame-path checkpoints rerun five release samples on the same machine. A range
greater than 10% is unstable, while a candidate median slowdown greater than
10% triggers investigation. Timedemo work counts remain useful, but its five
averages ranged from 1.889–3.699 ms despite an idle host and identical work, so
its frame timing is not currently a regression gate.

The first ownership proof adds only an integration test and no production-frame
collection, branch, namespace lookup, shader uniform, or render pass.

## The shrink-and-fall transition

Does **not** require two simultaneously authoritative simulations. Camera
scale-sweeps toward the diorama while the target world loads (as T0/T1/T2 or a
warm T3), then control hands off to that world's runtime and you "fall from its
sky." Watch depth precision: a large scale disparity compresses the nested
world's depth range — bounded and fine for a physically small tabletop, but the
transition (scale sweeping through orders of magnitude, world B filling the
view) is the z-fighting stress point; plan for a separate depth range or
reversed-Z there. A fully faked version (dive → fade → teleport into the single
real world spun up during the fade) buys most of the wow with zero nesting, and
is a valid first milestone.

## Feature → technique/tier map

| Feature | Technique | Tier |
|---|---|---|
| Lobby spawn room (+ no-destroy) | n/a | authored `WorldStore` + interaction gate |
| Last-world diorama on a table | embed | T0 baked |
| Live hosted region on a table | embed | T3 joined, preview-only authority |
| Distant active-world region on a table | embed, non-recursive | active runtime reuse |
| Current location on a table | embed, non-recursive | active runtime reuse |
| Seed explorer / worldgen console | embed | T1 live-local |
| Palantir into a network world | RTT window (or embed if reach-through) | T2 remote spectator |
| Warm Nether/sky/alternate generator | direct render after warm selection | T3 joined warm world |
| Walkable `x=0` seam | embed + complementary half-space visibility | T3 authority handoff |
| Traversable live portal | direct geometry + per-eye portal view/aperture | T3 authority handoff |
| Texture room (paint the atlas live) | n/a (atlas hot-reload) | orthogonal |
| Shrink-and-fall into a world | embed → runtime handoff | any; fakeable first |
| Cross-world shadows | requires new dynamic shadow subsystem | orthogonal, large |

## Recommended next direction

Ship independently valuable increments while keeping the one-world path direct:

The seed-explorer and shrink-and-fall product path now has a focused shared
navigation and authoritative arrival record in
[`world-view-navigation.md`](world-view-navigation.md). Its terrain horizon is
owned separately by
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). Reuse those
services for a future worldgen console; do not embed Terrain Lab's duplicated
TypeScript camera policy or make a preview URL authoritative player state.

[`Tactical 201`](../tactical/201-lobby-content-simplification.md) completed the
content-boundary cleanup: the lobby, preview, activation, and return behavior
remain on a transient authored primary plus ordinary catalog/app-private
destinations, with no managed installer or provisioning-shaped source locks.
Its post-completion handoff requires a fresh review of remaining semantic
TypeScript and duplicated native/web sequencing before any replacement actor
tactical is proposed.

Tactical 178 closed browser parity without a second scenario implementation;
Tactical 201 then removed its managed provision operation while retaining the
shared runtime-start boundary. The web host keeps the protected lobby plus
mutable destination alive in two independent integrated-server Workers. One
compiler Worker qualifies local request ids by stable world instance,
prioritizes active work, and owns per-world snapshot mirrors forked from one
immutable parsed asset template. The production desktop/mobile title reaches a
playable protected lobby, shows the shared-depth destination preview, activates
through mouse and touch in both directions, persists destination edits, and
rejects late completion across Quit, asset replacement, and resource rebuild.
Standby cadence travels through the typed runtime operation. Shutdown leaves
zero server or compiler Workers. Feature-off native/browser comparisons and
scenario-on CPU/frame/memory/Worker receipts are recorded in the tacticals.

Tactical 179 completed the bounded composition campaign. It kept that shared
runtime/platform foundation, extracted one terrain-led placement/bounds/clip
context without touching the direct path, proved complementary half-space
coverage through portable WebGPU shaders, and made actors the second renderer
consumer. Its product endpoint is live destination creatures and players on the
table on native and web. Its renderer-only half-space proof deliberately stops
before voxel boundary faces or traversal, which is now the next focused
renderer direction.

The warm-swap and opaque-gate proof lives in
[`174-warm-world-hot-swap.md`](../tactical/174-warm-world-hot-swap.md); the
bounded simultaneous-geometry plan lives in
[`175-live-hosted-world-diorama.md`](../tactical/175-live-hosted-world-diorama.md).

1. Keep the landed active-plus-optional-standby path, opaque A-to-B-to-A gate,
   optional standby cadence, lifecycle/invalidation contracts, and
   desktop/XR/no-request cost receipts green.
2. Keep Tactical 175's landed authored-only fixtures, separate placed-terrain
   pipeline, live mutation, water/translucent ordering, and local blink
   activation/return green without taxing the one-world path.
3. Keep Tactical 177's completed shared title launch, protected lobby,
   asynchronous fixed island, lifecycle, and no-scenario performance gates
   green.
4. Keep Tactical 178's dual-runtime compiler identity,
   activation/persistence lifecycle, and feature-off performance gates green
   with Tactical 201's managed web provisioning path permanently absent.
5. Keep Tactical 179's completed terrain normalization, half-space proof,
   per-world actor ownership, and live creature/player diorama green through
   one shared native and web path.
6. Keep Tactical 180's configurable 2x2/4x4 preview bounds, most-recent
   compatible catalog selection, read-only warmup, recency-on-activation,
   ordinary app-private Overworld fallback, and supported-arrival receipts
   green.
7. Prove the same bounded preview and activation behavior with a remote
   hosted source. Keep the dual-host/root-isolation smokes and materialized
   multiview contract green.
8. Generalize to an N-world registry only after the bounded live-diorama
   milestone is closed and a concrete multi-preview experience requires it.
9. T0 baked diorama and T1 seed explorer may reuse the same placement path as
   cheaper fidelity alternatives to the live T3 sample.
10. After the hosted-world milestone closes, allow a bounded active-world region
   to source the same non-recursive placed-terrain path without a second slot.
11. Add a real remote observer/subscription mode only when previews must stop
   consuming ordinary joined-player identities.
12. Follow Tactical 179's renderer-only half-space proof with boundary-aware
   meshing for an `x=0` scene proof before attempting traversal.
13. Add one-active-authority local handoff, then a traversable seam or portal.
14. Treat federated remote authority and dynamic shadows as separate later
   campaigns.

Keep dynamic shadows as a separate topic when that work opens; it is the biggest
and most novel piece and is orthogonal to embedding. Every new render feature
here must carry both a per-eye and a multiview path per the XR render-path
guardrail — an XR portal that is not per-eye correct breaks stereo and defeats
the immersion that motivates the whole feature.
