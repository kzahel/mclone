# Lay Of The Land Engine Research

Topic: `lay-of-the-land-engine-research`

Status: **initial public-source technical reconstruction recorded
2026-07-26.** The engine family, GPU-compute use, custom PhysX 5 integration,
10 cm voxel resolution, single-player scope, and on-disk world saves are
confirmed by first-party material. Exact chunk dimensions, data structures,
meshing algorithm, save granularity, support-analysis algorithm, entity
representation, and hard world bounds remain unverified.

Last reconciled: **2026-07-26**.

## Executive Answer

Lay of the Land is not a Unity/C# game. The developer publicly documented
moving the project from Unreal Engine 4 to Unreal Engine 5, primarily to use
Lumen. Its unusual voxel engine appears to be custom game technology hosted
inside Unreal rather than a stock Unreal terrain system.

The most useful confirmed technical ingredients are:

- a 10 cm voxel resolution, or 1,000 cells in the volume of one
  one-metre Minecraft-style block;
- GPU compute for voxel-data generation, voxel physics queries, mesh
  generation, and fire/water simulation work;
- a custom PhysX 5 integration in place of the Unreal 5 default physics path;
- smooth voxel level-of-detail transitions;
- layered procedural generation for terrain, rivers, roads, points of
  interest, and biome-specific caves;
- material simulations for support/gravity, fire, water, and sand; and
- one coherent voxel/material art language across terrain, vegetation,
  structures, props, effects, and visually block-built creatures.

Two scope corrections matter:

1. The game **does persist worlds to disk**. Official patch notes discuss old
   save formats, saving on exit, save performance, loading previous-version
   worlds, voxel-data chunks, and regions unloading. The unverified question
   is how saves are partitioned and whether generated data or edits are stored,
   not whether persistence exists.
2. The current product is single-player and behaves as a bounded world rather
   than a Minecraft-style indefinitely streamed world. The developer says
   multiplayer would require rewriting many systems and is too large an
   undertaking for one person. This is best understood as a deliberate
   complexity-budget trade, not evidence that the simulation ideas cannot
   coexist with persistence, streaming, or multiplayer in a differently
   structured engine.

Because this is Unreal, a purchased build is unlikely to yield clean C#
source through ordinary managed-code decompilation. It will more likely
contain optimized native C++ binaries, cooked Unreal packages and Blueprints,
and compiled shader bytecode. Those artifacts can still reveal package
layout, resources, dispatch dimensions, buffer formats, frame passes, file-I/O
patterns, and native control flow, but they are not equivalent to source.

## Scope

This topic owns:

- a dated, confidence-labelled reconstruction of Lay of the Land's voxel,
  simulation, physics, rendering, streaming, and persistence architecture;
- the practical inspectability of a lawfully obtained Unreal shipping build;
- a clean-room study plan that does not copy proprietary code or assets; and
- the architectural lessons that are promising or dangerous for Mclone.

It does not own:

- a plan to reproduce Lay of the Land's proprietary implementation, content,
  characters, or visual trade dress;
- Mclone's accepted persistence, multiplayer, rendering, or world-generation
  contracts;
- product pricing or market positioning; or
- legal advice about reverse engineering, licences, DRM, or local law.

The broader product comparison remains in
[`voxel-sandbox-competitive-landscape.md`](voxel-sandbox-competitive-landscape.md).
Mclone's own system decisions remain in the relevant architecture and topic
documents.

## Evidence Method

The confidence labels in this document mean:

| Label | Meaning |
|---|---|
| **Confirmed** | Stated or visibly demonstrated by the developer, the official store page, or official patch notes |
| **Observed** | Directly visible in released footage or repeatable product behaviour, but not accompanied by an implementation statement |
| **Inferred** | The simplest architecture consistent with confirmed facts; useful as a research hypothesis, not a fact |
| **Unknown** | No adequate first-party or reproducible evidence was found in this pass |

Developer videos are strong evidence for the system being discussed at the
video's publication date. They do not prove that every prototype detail
survived unchanged into the 2026 shipping build. Patch notes and current store
metadata take priority for current product behaviour.

## Confirmed Technical Ledger

| Concern | Finding | Confidence and evidence |
|---|---|---|
| Host engine | Unreal Engine 5, migrated from Unreal Engine 4 primarily for Lumen dynamic global illumination | **Confirmed:** developer video, [Moved to UE5](https://www.youtube.com/watch?v=y1u9QCRDO5g) |
| Voxel implementation | Custom voxel-game systems hosted in Unreal | **Confirmed at the system level:** the developer consistently calls it a voxel engine and documents custom generation, physics queries, meshing, simulation, and LOD; exact module boundaries are **unknown** |
| Cell resolution | Each voxel is 10 cm; a one-metre cube contains 1,000 such cells by volume | **Confirmed:** developer answer in [system-requirements discussion](https://steamcommunity.com/app/2776090/discussions/0/800091475572429083/) |
| GPU work | GPU compute accelerates voxel-data generation and work associated with mesh generation, voxel physics, fire, and water | **Confirmed:** developer video, [Making my Voxel Engine Really Fast](https://www.youtube.com/watch?v=PYu1iwjAxWM) |
| Physics backend | Custom PhysX 5 integration; the developer was dissatisfied with performance, stability, and simulation on Unreal 5's default physics path | **Confirmed:** developer video, [Improving my Voxel Game's Physics Engine](https://www.youtube.com/watch?v=bAhYtaR1-RU) |
| LOD | Smooth transitions were added to reduce visible voxel LOD popping | **Confirmed:** developer video, [Seamless LOD Transitions](https://www.youtube.com/watch?v=WfxuCEyK16I) |
| World generation | Layered generation creates terrain and watercourses; roads connect locations and can bridge rivers; caves can vary by biome | **Confirmed:** [Steam description](https://store.steampowered.com/app/2776090/Lay_of_the_Land/), [UE5/world-generation video](https://www.youtube.com/watch?v=y1u9QCRDO5g), and [cave-generation video](https://www.youtube.com/watch?v=flvqTxpNsVM) |
| Support/gravity | Large voxel areas are queried to detect unsupported material and make detached voxels fall | **Confirmed at behaviour and query-purpose level:** GPU-compute video; exact connectivity algorithm is **unknown** |
| Fire | Fire spreads over flammable voxel materials with per-material burn behaviour; it is deliberately game-balanced | **Confirmed:** [fire-simulation video](https://www.youtube.com/watch?v=gVssv_e0qcQ) and PhysX 5 video |
| Water | Water propagates predictably and supports waterfalls; the developer deliberately favoured game behaviour over full realism | **Confirmed:** [water-simulation video](https://www.youtube.com/watch?v=RhII5ds4XFw) |
| Sand | Sand uses the voxel simulation system, can move, make sound, bury actors, and hurt the player or enemies | **Confirmed:** [sand-simulation video](https://www.youtube.com/watch?v=7QZvYfAwgvw) |
| Building | Placement can create more than axis-aligned cubes, including cylinders, cones, and slopes | **Confirmed:** current Steam description and footage |
| Product mode | Windows and single-player only in current store metadata | **Confirmed:** [Steam store page](https://store.steampowered.com/app/2776090/Lay_of_the_Land/) |
| Multiplayer posture | No multiplayer is planned; the developer says it is too large for one person and would require rewriting many systems | **Confirmed:** [developer response](https://steamcommunity.com/app/2776090/discussions/0/760681630846409294/) |
| Disk persistence | Worlds are saved and loaded; save formats have changed; save management, exit saves, and save performance have received patches | **Confirmed:** official patch notes described under [Persistence Is Present](#persistence-is-present) |
| World extent | Players report that the released world is small and traversable, not indefinitely extending like Minecraft | **Observed/community evidence:** [top-rated player reviews](https://steamcommunity.com/app/2776090/positivereviews/?browsefilter=toprated); exact dimensions and internal bound are **unknown** |

## Working Architecture Reconstruction

The following diagram is a hypothesis that organizes the confirmed public
facts. It is not recovered source architecture.

```text
 seed + biome fields + caves + POIs + road/river rules
                         |
                  voxel assets and edits
                         |
                region/chunk voxel state
                /          |            \
               /           |             \
 GPU generation,      local material     support /
 meshing, and LOD      simulations        connectivity queries
       |               /   |    \                 |
       |           fire  water  sand              |
       |               \   |    /          detached voxel bodies
       |                dirty regions             |
       |                     |              custom PhysX 5 bridge
       \_____________________|_____________________/
                             |
                   Unreal 5 render integration
                      materials + Lumen
```

The diagram implies four boundaries worth testing:

1. **Canonical voxel state versus derived representations.** A voxel region
   likely owns material occupancy while render meshes, physics shapes, and
   simulation frontiers are derived. The exact storage format is unknown.
2. **Batched GPU queries versus CPU orchestration.** Public footage explicitly
   contrasts CPU and GPU compute and discusses waiting for physics-query
   results. That suggests asynchronous batches with CPU-side dependency
   management, but queue ownership and readback strategy are unknown.
3. **Static terrain versus movable assemblies.** Unsupported material becomes
   falling physics content. It is unknown whether detached pieces retain a
   sparse voxel volume, become generated meshes plus collision shapes, or use
   another representation.
4. **Loaded simulation regions versus saved world records.** Current patch
   notes mention regions unloading and voxel-data chunks. Those may be runtime
   structures, persistence structures, or both; the public evidence does not
   establish a one-to-one mapping.

## Why The World Feels Coherent

The strongest product lesson is not merely “smaller voxels.” It is that one
material and shape grammar appears everywhere:

- terrain, cliffs, caves, and ore expose the same fine volumetric grain;
- trees, foliage, rocks, buildings, ruins, and props look assembled from that
  grammar instead of imported from a visually unrelated asset library;
- destruction and detached pieces preserve the same material identity;
- fire, water, sand, and gravity react to the world rather than appearing as
  isolated scripted effects;
- enemies are visually constructed from blocky voxel-like forms; and
- Unreal lighting, shadows, atmosphere, and Lumen unify all of those simple
  forms into one rendered scene.

That combination creates the Minecraft-like effect the user noticed:
creatures and terrain feel as if they belong to the same world ontology. The
important distinction is that visual coherence does not prove identical
runtime representation. Public footage does not establish whether enemies
are editable voxel volumes, generated meshes, skeletal meshes authored in a
voxel style, or hybrids. Treat “enemies use the same voxel data structure as
terrain” as an open question.

For Mclone, the transferable principle is:

> Share the block/material registry, scale vocabulary, palette, lighting
> response, damage language, and debris language across world and actors,
> even when their optimal runtime representations differ.

A rigged cuboid creature can belong to a voxel world without paying for a
fully mutable 10 cm volume at every animation frame.

## Simulation And Physics Model

### Fine Resolution Changes The Workload

A 10 cm grid has ten cells along each axis of a one-metre cube, hence 1,000
cells by volume. This enables slopes, thin walls, branches, detailed carvings,
and rounded construction while retaining discrete material cells.

It also multiplies:

- occupancy and material memory;
- generation work;
- neighbourhood queries;
- dirty-region tracking;
- mesh extraction;
- collision generation;
- support/connectivity analysis;
- simulation frontier size;
- save bandwidth; and
- edit replication cost in any future multiplayer design.

The developer's GPU-compute video shows an example query volume of roughly
`150 x 150 x 225`, about five million voxel samples, while discussing support
queries and edit stalls. This is evidence for aggressive batching, not enough
evidence to identify the exact data layout or algorithm.

### Support And Falling Pieces

The public description is consistent with a local support-analysis pipeline:

1. an edit marks a bounded area dirty;
2. a batched query classifies connected or supported voxels;
3. unsupported components are extracted from the terrain;
4. those components receive render and collision representations;
5. PhysX advances them as movable bodies; and
6. contact, fracture, settling, or collection reconciles their outcome.

Only the high-level purpose—finding floating voxels and making them fall—is
confirmed. Flood fill, union-find, graph connectivity, support columns,
coarse occupancy, GPU prefix operations, and component extraction are
candidate techniques, not known Lay of the Land implementation details.

### Material Simulations

The videos suggest multiple simulations share voxel occupancy, material
properties, dirty-region scheduling, and mesh invalidation:

- **fire** advances over eligible materials using material-dependent rates;
- **water** propagates under predictable game rules and waits on relevant
  physics/mesh work;
- **sand** moves as discrete simulated material and interacts with actors; and
- **gravity/support** converts unsupported world material into physics
  objects.

The design lesson is valuable: one bounded simulation substrate can support
several high-impact behaviours. The videos also expose the scheduling
challenge—one edit can enqueue support queries, fluid work, mesh rebuilds,
collision changes, sound, and rendering updates.

## Rendering And Meshing

Confirmed rendering facts are limited but useful:

- Unreal 5 and Lumen provide dynamic global illumination;
- the voxel engine generates meshes and uses GPU compute in that work;
- the project implements smooth LOD transitions; and
- the shipping requirements list DirectX 12, with hardware ray tracing
  recommended.

The following remain unknown:

- surface extraction method: greedy faces, surface nets, dual contouring,
  marching cubes, custom templates, or a hybrid;
- whether curves such as cylinders and slopes are voxel-filled primitives,
  meshing hints, procedural brushes, or separate geometry;
- chunk and meshlet dimensions;
- material-atlas and texture-data layout;
- LOD downsampling and seam construction;
- whether transition geometry, temporal fading, stochastic masking, or
  morphing implements the smooth LOD change;
- exact Lumen scene representation for voxel meshes; and
- how often simulation-generated geometry rebuilds collision and render data.

These questions are much easier to answer with frame captures and controlled
visual experiments than by starting with native disassembly.

## Persistence Is Present

The phrase “never persisted to disk” should not be carried into Mclone
planning. Official 2026 patch notes establish a real save/load system:

- [1.1.2](https://store.steampowered.com/news/app/2776090/view/1835871199308532)
  fixed a crash when loading world saves in older formats.
- [1.1.4](https://store.steampowered.com/news/app/2776090/view/1835871199310582)
  refactored save management for resilience and made application exit save.
- [1.1.6](https://store.steampowered.com/news/app/2776090/view/1836506165552428)
  avoided a redundant second exit save and improved voxel-simulation memory
  and performance.
- [1.1.7](https://store.steampowered.com/news/app/2776090/view/1836506165559189)
  improved saving performance and described a guard for a player outrunning
  world generation.
- [1.1.9](https://store.steampowered.com/news/app/2776090/view/1836506165561520)
  handled worlds saved by previous versions, cleaned flowing water on load,
  and partitioned voxel-asset instances at voxel-data chunk boundaries.
- [1.1.14](https://store.steampowered.com/news/app/2776090/view/1837955055365086)
  referred to simulation update range and fixed a crash involving regions
  stepping simulations while being unloaded.

These notes confirm disk persistence, versioned save compatibility work,
runtime spatial partitioning, and region lifecycle. They do **not** answer:

- whether a save contains all generated voxel occupancy, seed plus deltas, or
  a hybrid;
- whether world records are one monolithic file or separately replaceable
  region records;
- whether saves are incremental, journaled, copy-on-write, or rewritten;
- how entity state and voxel state are transactionally coordinated;
- whether inactive regions are evicted from memory and lazily restored from
  disk;
- whether “voxel-data chunk” is also the persistence unit; or
- whether the spatial key can extend indefinitely.

The correct competitive comparison is therefore:

| Capability | Lay of the Land evidence | Mclone requirement |
|---|---|---|
| Save/load | Present and actively patched | Present, durable, versioned |
| Runtime region lifecycle | Present | Present across server and clients |
| Infinite-style horizontal extension | Not demonstrated; released world is reported as bounded/small | Required by the intended Minecraft-like world contract |
| Multiplayer authority | Absent by design | Authoritative server and replicated clients |
| Cross-platform storage | Windows product only | Desktop, web, Android, XR, and server adapters behind shared policy |

### Why A Bounded World Helps This Engine

A bounded world can simplify:

- up-front generation and global feature placement;
- road and river graph planning;
- save indexing and compatibility;
- global progression and boss placement;
- maximum simulation-state estimates;
- navigation and content density;
- testing every reachable biome;
- memory and performance budgets; and
- avoidance of remote edit replication, ownership, and conflict.

It does not make the renderer, physics, or material simulations trivial.
Rather, it removes several unbounded-lifecycle problems from an already
ambitious solo engine.

## Single-Player Is An Architectural Choice

The store lists single-player only. In a 2026 Steam response, the developer
said multiplayer is not planned because it is too large an undertaking for
one person and would require rewriting many game systems.

That answer is technically plausible. Retrofitting multiplayer into this
simulation would require decisions about:

- authority for every voxel edit;
- deterministic versus server-resolved support queries;
- stable identity for detached voxel assemblies;
- replication of fracture, transforms, sleep, and collision outcomes;
- rollback or correction when local prediction differs;
- water, fire, and sand event ordering;
- interest management for fine-resolution changes;
- save ownership and join-in-progress snapshots;
- malicious edit and physics validation;
- bandwidth compression; and
- gameplay balance for variable player counts.

Mclone should not read this as permission to prototype equivalent behaviour
client-side and “network it later.” Its transferable simulation ideas must
enter through shared, server-authoritative contracts from the beginning, even
when an initial visual approximation is client-side.

## What A Shipping Unreal Build Can Reveal

### Why Unity-Style C# Decompilation Does Not Apply

Unity projects often ship managed .NET assemblies when using Mono, and those
assemblies can preserve enough type and method structure for high-level
decompilers. That premise does not fit this game.

Epic's [packaging documentation](https://dev.epicgames.com/documentation/en-us/unreal-engine/packaging-your-project)
describes C++ compilation into platform binaries and cooking assets and
Blueprints into runtime formats. Modern Unreal packages may use `.pak`,
`.utoc`, and `.ucas` containers. A typical Windows shipping build therefore
offers a different evidence surface:

| Artifact | What may be learnable | What not to expect |
|---|---|---|
| Native executable and DLLs | Imports, strings, RTTI remnants, call graphs, constants, allocation patterns, native pseudocode | Original C++, names stripped by shipping build, comments, templates, clean types |
| Cooked Unreal assets | Package names, dependencies, materials, meshes, some Blueprint structure or metadata when compatible and not encrypted | Editor source assets, complete implementation source, unrestricted redistribution |
| Shader bytecode | Pipeline stages, resource bindings, constants, thread-group dimensions, DXIL disassembly, sometimes debug names | Original HLSL/USF with comments and symbols when debug data was stripped |
| Save files | File boundaries, write patterns, compression clues, version changes, locality of edits | Meaningful schema without controlled experiments or sufficient metadata |
| Runtime frame | Pass order, render targets, dispatches, barriers, buffer sizes, timings | Direct proof of CPU-side data structures or gameplay authority |

Epic's [shader-development documentation](https://dev.epicgames.com/documentation/en-us/unreal-engine/shader-development-in-unreal-engine)
also makes clear that material and global shaders are compiled/cached for
runtime use. Microsoft's
[PIX shader-debugging notes](https://devblogs.microsoft.com/pix/using-automatic-shader-pdb-resolution-in-pix/)
explain why source/debug information is often stripped from shipped shader
artifacts. Compute work can remain very informative in PIX or RenderDoc even
when the original shader source cannot be reconstructed.

### Practical Tools

For a lawfully obtained, unmodified build:

- **FModel/CUE4Parse** can inventory supported, unencrypted Unreal archives,
  packages, names, dependencies, and compatible cooked assets. The
  [FModel project](https://github.com/4sval/FModel) is an appropriate starting
  point.
- **PIX** can capture Direct3D 12 GPU work, timings, resources, dispatch
  dimensions, and compatible shader debugging. See the
  [PIX overview](https://learn.microsoft.com/en-us/windows/win32/direct3dtools/pix/articles/general/pix-overview).
- **RenderDoc** can inspect frames and disassemble/debug supported D3D12
  shaders; source-level views depend on shipped debug information. See
  [RenderDoc releases](https://github.com/baldurk/renderdoc/releases).
- **Ghidra** can recover native pseudocode and call relationships from
  binaries, but optimized Unreal C++ is substantially harder and less
  semantically faithful than decompiled managed C#.
- **Process Monitor** can correlate save, load, region travel, exit, and edits
  with file opens, writes, offsets, sizes, and replacements.
- **NVIDIA Nsight Graphics** may add GPU workload, resource, and shader
  visibility on compatible hardware.
- NVIDIA publishes the
  [PhysX source](https://github.com/NVIDIA-Omniverse/PhysX) under BSD-3-Clause,
  so the general backend can be studied directly. Lay of the Land's custom
  bridge, voxel-shape construction, and scheduling remain proprietary.

Compatibility, package encryption, anti-tamper measures, stripped symbols, or
missing shader debug data can reduce what these tools expose. Do not bypass
DRM or encryption.

## Recommended Clean-Room Study Sequence

Start with the cheapest and most reproducible evidence. Native decompilation
is last, not first.

### Phase 0: Public Reconstruction

1. Archive the source register below with access dates.
2. Transcribe only implementation-relevant statements from developer videos.
3. Capture diagrams of edit, support, water, fire, sand, LOD, and generation
   dependencies.
4. Record which claims describe a prototype versus the current release.
5. Convert each unknown into an observable experiment.

Output: a versioned evidence table and no proprietary extracts.

### Phase 1: Controlled Product Experiments

If the project acquires a normal Steam copy, use disposable worlds and record:

- seed, mode, settings, game version, hardware, and save directory;
- creation time, first load, save time, exit time, and subsequent load;
- file tree, sizes, modification times, and hashes before and after one edit;
- the same edit near and far from the player;
- travel time and behaviour at the apparent world boundary;
- whether an edited remote region unloads and reloads accurately;
- whether fire, water, and sand advance while distant or unloaded;
- maximum unsupported dimensions and breakup behaviour;
- LOD transition distance and whether silhouettes morph or fade;
- detached-object collision complexity and sleep behaviour; and
- whether enemies can be damaged, fractured, or edited like terrain.

Output: black-box measurements and screenshots/video, not copied content.

### Phase 2: Non-Destructive Build Inventory

1. Record the app/build ID and hashes.
2. Inventory executable, DLL, plugin, archive, configuration, and shader-cache
   files.
3. Identify Unreal package/container versions and whether archives are
   normally readable.
4. List package and module names without redistributing assets.
5. Record save locations and file-I/O behaviour.

Output: a reproducible manifest referencing only the locally owned build.

### Phase 3: GPU Capture

Capture quiet, controlled frames for:

- untouched terrain;
- one small edit;
- one large support collapse;
- active water;
- active fire;
- active sand;
- a detached voxel object;
- an enemy at close range; and
- an LOD transition.

Compare compute dispatch counts, thread-group dimensions, resources, barriers,
readbacks, buffer growth, render-pass changes, and timings. A small matrix of
contrasts will reveal more than an unstructured “busy frame.”

Output: measured resource and scheduling hypotheses. Do not redistribute
captured proprietary textures, meshes, or shaders.

### Phase 4: Native Analysis

Use native inspection only to answer questions left unresolved by prior
phases, such as:

- whether a named module owns voxel state or PhysX bridging;
- which operations cause GPU/CPU synchronization;
- how save versions are dispatched; or
- whether runtime chunks and save records share keys.

Document observations as behaviour and architecture principles. Do not copy
implementation code into Mclone.

## High-Value Experiments

| Question | Minimal experiment | Distinguishing evidence |
|---|---|---|
| Seed plus deltas or full-volume save? | Create identical-seed worlds, make one local edit, compare save sizes and changed ranges | Small local changes favour delta/region records; broad rewrites may indicate monolithic serialization, compression effects, or metadata updates |
| Incremental region persistence? | Edit two distant regions separately and observe files/offsets written | Spatially separate records or bounded writes support regional persistence |
| Does simulation stop on unload? | Start slow fire/water/sand, travel far enough to unload, return after controlled time | State frozen, analytically advanced, or continuously simulated |
| Terrain and detached pieces share storage? | Detach a uniquely shaped piece, save/reload it, then fracture it again | Preserved editable cells suggest a voxel-volume assembly; fixed breakage or mesh-only behaviour suggests conversion |
| Are enemies true mutable voxel volumes? | Test identical local damage against terrain, props, and multiple enemy body regions | Persistent cell removal/material transfer versus authored hit reactions |
| Support algorithm scope? | Create beams, loops, diagonal contacts, thin necks, and multiple grounded components | Connectivity and support rules can be inferred without implementation access |
| LOD technique? | Record slow lateral motion against a high-contrast silhouette | Geometry morph, dither/fade, seam mesh, or abrupt topology change |
| Compute/readback bottleneck? | Compare GPU captures for edits with and without falling pieces or fluids | Extra copy/readback/barrier stages reveal CPU-dependent classification |
| Hard bound or content mask? | Approach boundary at multiple heights and directions; inspect generation activity | Fixed coordinates, invisible wall, water/island edge, absent chunks, or generator refusal imply different product constraints |

Compression and timestamps can mislead binary comparisons. Repeat each
experiment, change one variable, and retain untouched controls.

## Lessons For Mclone

### Transfer The Principle

- **One world ontology.** Use the same material identities, palette families,
  lighting response, sound categories, damage vocabulary, and debris language
  across blocks, structures, vegetation, items, creatures, and effects.
- **Predictable simulation beats maximal realism.** Lay of the Land's water
  and fire descriptions explicitly favour useful game behaviour. Mclone
  should seek legible rules that create stories and can be authoritative.
- **Batch spatial questions.** Support, dirty-mesh, light, fluid, and material
  work benefit from explicit bounded jobs rather than per-cell object logic.
- **Separate canonical state from render and physics derivatives.** This is
  necessary for server authority, persistence, and platform-neutral clients.
- **Use high resolution locally.** Fine voxel assemblies, authored shapes, or
  sub-block brushes may deliver detail without making the entire infinite
  overworld a 10 cm canonical grid.
- **Make LOD transitions part of the representation contract.** Fine-detail
  content needs a deliberate distance representation, not merely smaller
  chunks.
- **Bound active simulation, not the world.** Region update ranges and
  lifecycle are compatible with an indefinitely extensible saved world.

### Do Not Import The Scope Assumptions

- Do not adopt a global 10 cm canonical world without measured memory,
  generation, save, network, web, Android, Quest, and XR budgets.
- Do not make PhysX or any desktop-native backend the gameplay authority.
- Do not prototype support, fluids, fire, or sand as irreversible client-only
  policy.
- Do not rely on Lumen, hardware ray tracing, DirectX 12, or Unreal-specific
  cooked assets for the product's visual identity.
- Do not let a bounded map simplify away Mclone's required chunk persistence,
  eviction, regeneration, and server interest management.
- Do not treat visually voxel-built enemies as proof that every actor should
  be a fully mutable voxel volume.
- Do not copy proprietary assets, character silhouettes, code, shader
  bytecode, names, encounter design, or visual trade dress.

### A Plausible Mclone Adaptation

The promising adaptation is a two-scale system:

1. the Minecraft-compatible one-metre block world remains canonical,
   indefinitely streamable, persistent, and server-authoritative;
2. selected actors, props, structures, debris, or transient edit volumes can
   use a bounded local fine-resolution representation;
3. local assemblies reference the same shared material registry as world
   blocks;
4. server jobs decide support, fracture, and material state at a deliberately
   limited fidelity;
5. clients derive meshes, particles, and physics proxies appropriate to their
   platform; and
6. save and protocol formats record semantic assembly state rather than a
   desktop physics backend's opaque state.

This could produce Lay of the Land's coherence without inheriting its
single-player, Windows-only, finite-world assumptions.

## Current Conclusions

1. Lay of the Land merits focused technical study; it is more than a visual
   competitor profile.
2. The headline architecture is **Unreal 5 host + custom voxel systems + GPU
   compute + custom PhysX 5 integration**, not Unity/C#.
3. Its strongest design achievement is the combination of fine voxel scale,
   systemic material response, and one visual grammar across world and actors.
4. World saving exists. The important missing knowledge is storage granularity
   and streaming topology.
5. Bounded single-player scope likely made the engine feasible for a solo
   developer and should be recorded as an explicit trade, not dismissed as an
   oversight.
6. A shipping build is inspectable but not simply decompilable into original
   source. GPU captures and controlled persistence experiments are likely to
   provide higher-value early evidence than native reverse engineering.
7. Mclone should borrow the coherent ontology, local fine detail, batched
   spatial work, and predictable simulations while preserving shared
   authority, persistence, multiplayer, infinite-style streaming, and
   cross-platform renderer boundaries.

## Open Questions

- What are the actual world dimensions and boundary representation?
- Are regions generated lazily, pre-generated, or hybrid?
- What are voxel chunk, mesh chunk, physics query, and save-record dimensions?
- Is occupancy dense, sparse, compressed, hierarchical, or split by material?
- Which GPU work generates occupancy and which work only classifies or meshes
  CPU-generated data?
- What surface extraction and LOD seam algorithms are used?
- What information crosses back from GPU support queries to the CPU?
- How are detached voxel bodies represented, fractured, settled, and saved?
- Are enemies mutable voxel data or voxel-styled skeletal/procedural meshes?
- Do simulations share one scheduler and cell representation?
- How are update ranges, unloading, and save transactions coordinated?
- Are voxel-data chunks the persistence unit or only a runtime asset boundary?
- How much of the released build's architecture still matches the older
  development videos?

## Source Register

Checked **2026-07-26**.

### First-Party Product And Developer Sources

- [Steam product page](https://store.steampowered.com/app/2776090/Lay_of_the_Land/)
  — current platform, single-player feature, release date, requirements, and
  product description.
- [Steam news](https://store.steampowered.com/news/app/2776090) — current
  official patch stream.
- [Developer YouTube channel](https://www.youtube.com/@tooley1998) — public
  development record.
- [Moved to UE5](https://www.youtube.com/watch?v=y1u9QCRDO5g) — Unreal 5,
  Lumen, world generation, roads, bridges, and locations.
- [Making my Voxel Engine Really Fast](https://www.youtube.com/watch?v=PYu1iwjAxWM)
  — GPU compute, generation, meshing, physics queries, and simulations.
- [Improving my Voxel Game's Physics Engine](https://www.youtube.com/watch?v=bAhYtaR1-RU)
  — custom PhysX 5 integration and material physics.
- [Seamless LOD Transitions](https://www.youtube.com/watch?v=WfxuCEyK16I) —
  smooth voxel LOD work.
- [Flowing water](https://www.youtube.com/watch?v=RhII5ds4XFw),
  [fire](https://www.youtube.com/watch?v=gVssv_e0qcQ), and
  [sand](https://www.youtube.com/watch?v=7QZvYfAwgvw) — material simulation
  design and performance work.
- [Procedural caves](https://www.youtube.com/watch?v=flvqTxpNsVM) — cave
  entrances and biome variation.
- [First enemy](https://www.youtube.com/watch?v=XjR_GTP_XfU) — public enemy
  presentation; not sufficient to establish entity data representation.
- [Creative mode](https://www.youtube.com/watch?v=o-1cR8DpcHY) — flat world,
  flight, unlimited resources, and world options.
- [10 cm developer answer](https://steamcommunity.com/app/2776090/discussions/0/800091475572429083/)
  — voxel resolution and performance rationale.
- [Multiplayer developer answer](https://steamcommunity.com/app/2776090/discussions/0/760681630846409294/)
  — explicit solo scope and rewrite cost.

### Tool And Format References

- [Epic: Packaging Unreal projects](https://dev.epicgames.com/documentation/en-us/unreal-engine/packaging-your-project)
- [Epic: Shader development](https://dev.epicgames.com/documentation/en-us/unreal-engine/shader-development-in-unreal-engine)
- [Microsoft: PIX overview](https://learn.microsoft.com/en-us/windows/win32/direct3dtools/pix/articles/general/pix-overview)
- [Microsoft: shader PDB resolution in PIX](https://devblogs.microsoft.com/pix/using-automatic-shader-pdb-resolution-in-pix/)
- [RenderDoc](https://github.com/baldurk/renderdoc/releases)
- [FModel](https://github.com/4sval/FModel)
- [NVIDIA PhysX](https://github.com/NVIDIA-Omniverse/PhysX)

### Secondary Or Community Evidence

- [Top-rated Steam reviews](https://steamcommunity.com/app/2776090/positivereviews/?browsefilter=toprated)
  — released-world scale and player-perceived visual coherence. Reviews are
  product observations, not engine documentation.

## Related Documents

- [`voxel-sandbox-competitive-landscape.md`](voxel-sandbox-competitive-landscape.md)
  — broader competitive catalogue and inspiration ledger.
- [`distribution-go-to-market.md`](distribution-go-to-market.md) — market
  positioning and commercial-channel decisions.
- [`native-engine-architecture.md`](../native-engine-architecture.md) —
  Mclone's shared engine boundaries.
- [`unified-persistence-interface.md`](unified-persistence-interface.md) —
  Mclone's shared persistence port and platform adapter boundary.
- [`world-generation-profiles.md`](world-generation-profiles.md) — Mclone
  generator profile and compatibility policy.
- [`persistent-actor-identity.md`](persistent-actor-identity.md) — durable
  actor identity and save/network ownership.
