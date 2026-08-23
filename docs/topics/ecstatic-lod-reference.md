# Ecstatic LOD Comparative Reference

Topic: `ecstatic-lod-reference`

Status: exact Ecstatic Fabric 1.3.0 artifact and authorized source mirror
preserved and inspected as of 2026-08-23; compared at source level with the
current Mclone procedural-horizon clipmap, Distant Horizons, and Voxy.

## Scope and evidence

This is implementation research, not a proposal to port Ecstatic or reopen the
retired chunk-based Far LOD. Unqualified Mclone LOD still means the shared
geometry clipmap described by [`lod.md`](lod.md) and
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md).

The Ecstatic evidence is:

- the exact CurseForge Fabric 1.3.0 JAR, file ID `8656500`, with SHA-256
  `08a01766ce01915db8e818a786a4ffa1d258c486354987abea5685cfffb68412`;
- a CFR 0.152 decompilation of that exact artifact; and
- the authorized `Wizardtastic/Ecstatic` `1.20.1` source mirror at
  `92818f439ad9162e478c8148599049006dbae2a8`.

See [`../../reference/ecstatic/README.md`](../../reference/ecstatic/README.md)
for local paths, acquisition provenance, the release/source distinction, and
the CC-BY-NC-SA-4.0 boundary.

The comparison checkouts were refreshed before inspection:

- Distant Horizons `main` at
  `88ad4417c7638d607210707dd7da3e4ac34de680`, with `coreSubProjects` at
  `14068289789a93fd68f266e51d21829610a02833` (2026-08-22); and
- Voxy `dev` at `337b919d6638cce3d65264efb10b0d20cd060010`
  (2026-08-10).

The source mirror is useful for readable Minecraft names and shader resources,
but it is not a tagged copy of release 1.3.0 and its inspected tip contains
work-in-progress cleanup inconsistencies. Release claims below come from the
exact artifact unless explicitly identified as later source-tip work.

## Conclusion

Ecstatic is similar to Mclone at the **source and representation thesis**:
both can reconstruct untouched distant terrain directly from a seed-compatible
procedural source, sample it on power-of-two grids, retain only a surface-scale
summary, and synthesize distant vegetation rather than materializing every
near-world object.

It is not similar to Mclone's current **streaming architecture**. Ecstatic
assigns one resolution to each fixed 512-by-512-block region inside concentric
distance bands, generates and persists that region, and builds a CPU vertex
buffer for it. Work, stored records, and tracked region identity grow with the
covered area. Mclone keeps a fixed 4-by-4 toroidal residency at each enabled
clipmap level, prepares only entering strips, and atomically admits complete
levels behind an exact/procedural frontier certificate.

The closest one-line classification is:

> Ecstatic is a direct-generator, surface-only, persistent region LOD cache;
> Mclone is a direct-generator, surface-and-semantic, fixed-residency geometry
> clipmap.

Ecstatic therefore reinforces the decision to query a cheap procedural source
instead of generating distant chunks, but it does not provide a stronger
replacement for Mclone's clipmap, movement, seam, or ownership machinery.

## Ecstatic system shape

```text
integrated Overworld seed + generator + biome registry
                         |
             Minecraft finalDensity query
                         |
      downward 8-block search + binary surface refine
                         |
       height + biome ID + RGB + tree-presence bit
                         |
 six append-only, memory-mapped per-LOD region files
                         |
      fixed 512 x 512 region assigned one LOD level
                         |
       worker-built CPU terrain/tree/water buffers
                         |
 render-thread upload -> AFTER_TRANSLUCENT draw + fog
```

### Host and integration boundary

The exact Fabric artifact is a client mod for Minecraft 1.20.1 and Java 17.
It starts its coordinator from the integrated server's Overworld generator,
random state, and biome registry. Rendering exits when there is no integrated
single-player server or the client is outside the Overworld. There is no
remote-server terrain-source protocol in 1.3.0.

Fabric integration is deliberately narrow: server start/stop events, a world
render callback after translucent terrain, and a fog mixin. Ecstatic extends
the projection/fog range and owns separate OpenGL-era Minecraft render types
and shaders. Iris/Oculus handling is reflective and defensive; the project
page itself warns that shader-pack support is not working reliably.

### Spatial organization

`RegionCoord` is always 32 by 32 chunks, or 512 by 512 blocks. A region is
classified from its nearest and farthest distance to the camera and receives
one active level:

| Ecstatic level | Sample spacing | Samples/region | Stored payload/region |
|---:|---:|---:|---:|
| LOD1 | 2 blocks | 256 x 256 | 512 KiB |
| LOD2 | 4 blocks | 128 x 128 | 128 KiB |
| LOD3 | 8 blocks | 64 x 64 | 32 KiB |
| LOD4 | 16 blocks | 32 x 32 | 8 KiB |
| LOD5 | 32 blocks | 16 x 16 | 2 KiB |

Each stored sample is eight bytes: signed 16-bit height, a 15-bit raw biome ID
plus one tree bit, and 24-bit RGB in a 32-bit field. Fine target levels LOD1
through LOD3 also request a spacing-16 bootstrap placeholder, stored in the
separate `lod0.dat`, before the full region finishes.

The release ring widths are 28, 32, 64, 12, and 24 chunks before the user
scale, with four chunks of hysteresis. The shipped config defaults the scale
to 0.9 even though the CurseForge prose describes a 100-percent default. At a
12-chunk vanilla distance, 0.9 produces boundaries at 2, 27, 56, 114, 125,
and 147 chunks. Because the assignment unit is 32 chunks wide, the finest
level can be skipped or occupy a noticeably different footprint depending on
where the camera lies inside the fixed region grid.

This is the fundamental non-clipmap property. Raising distance increases the
number of regions scanned, sampled, stored, meshed, and later revisited. A
clipmap instead fixes the number of logical slots per level and changes the
world coordinates represented by those slots.

### Surface sampling

For each `(x, z)` grid point, `SurfaceSampler` queries Minecraft 1.20.1's
`randomState.router().finalDensity()` directly. It walks downward from the
world maximum in eight-block steps until density becomes positive, then binary
refines the first crossing. A previous-row height hint rejects a candidate
more than 24 blocks above the prior sample and continues downward, suppressing
some floating outliers.

The sampler then asks the biome source for the noise biome, stores its raw
registry ID, averages a five-by-five grass-color neighborhood, applies special
badlands handling, and records whether the biome's vegetation step contains
any configured tree feature.

That is why Ecstatic can reach far terrain without normal chunk generation.
It is also why the data is a prediction of the natural upper surface rather
than a compact copy of the realized world:

- surface rules, carvers, fluids, arbitrary overhangs, and vertical cave
  topology are not represented;
- ordinary block edits are not incorporated;
- feature and mod interactions can diverge from the direct density query; and
- the surface contract is coupled to Minecraft 1.20.1 internals rather than a
  versioned neutral generator interface.

### Persistence and update behavior

Ecstatic creates `world/ecstatic/<dimension>/lod0.dat` through `lod5.dat`.
Each is a flat append-only region store with an index and fixed-size records,
memory-mapped as the file grows. A completed region forces the file. Boundary
sampling also writes the positive-X, positive-Z, and diagonal neighbor's zero
edge so equal-resolution region meshes share border samples.

Format version 4 identifies the record layout and spacing, but the release
does not persist a seed, generator revision, mod/config fingerprint, or
semantic source identity. It stores biome raw IDs rather than stable names.
Consequently this is useful as a derived cache, but not a safe model for an
authoritative or indefinitely reusable cache across generator/registry
changes. Reclassification can also leave the same region represented in
multiple per-level files over time.

The exact 1.3.0 scheduler uses a priority blocking queue and a configurable
worker count. Lower level number wins first, then nearer distance. Sampling
and CPU mesh construction share this background work. Render-thread uploads
are drained under approximate 1.5 ms and 1 ms budgets for initial and fade
rebuild work; shader-pack activity divides those budgets by three.

The release does not cancel or validate queued sampling when the camera,
target level, world, or renderer state changes. A latest-dispatched-level map
prevents some stale meshes from becoming current, but obsolete sampling can
still run and persist. The later source mirror contains attempted validity and
thread-safety cleanup, which confirms that this was a live concern, but it is
not evidence about shipped 1.3.0 behavior.

### Geometry, materials, and transitions

LOD1 and LOD2 are textured stepped terrain: each sample emits a top box-like
surface with downhill skirts, and LOD1 can quantize bilinear height to
half-block substeps. LOD3 through LOD5 are untextured, faceted triangle
heightfields with quantized per-cell color. Whole 512-block AABBs are frustum
culled.

Water is not sampled hydrology. Cells whose four terrain corners are not all
above sea level receive a separate surface at hard-coded Y 62.8, with extra
tessellation at coarse levels for the water texture. Ice follows biome
temperature. An optional opaque-water mode can cull submerged terrain.

Near the vanilla boundary, Ecstatic overlaps exact and LOD terrain and bakes a
radial alpha fade into the two finest meshes. Those meshes are rebuilt after
roughly 32 blocks of camera movement. Same-resolution seams use shared border
samples and skirts; cross-level boundaries rely on the fixed-region overlap,
skirts, fade, and fog rather than an explicit topology or single-owner
certificate.

Trees are procedural presentation proxies, not sampled tree positions. The
code estimates expected biome tree density from placement modifiers and uses
a deterministic hash to place box trees at LOD1/2 and crossed quads at LOD3.
It does not render trees at LOD4/5.

The advertised structure support is narrower than the terrain reach. A
`LodStructureIslands` path scans a band from vanilla distance to 12 chunks
beyond it, derives candidate starts, and only renders structures whose actual
chunk NBT already exists on disk. It extracts exposed voxel faces up to a
bounded height/footprint and substitutes those saved chunks for the procedural
heightfield. It is not seed-only procedural structure realization throughout
the far rings.

The included parallax terrain shader is presentation detail rather than
geometric refinement. It applies up to four steps of procedural triplanar
noise and self-shadow approximation, fading out between 64 and 512 blocks;
the source labels it “kinda sorta deprecated.”

## Comparison

| Property | Mclone current LOD | Ecstatic 1.3.0 | Distant Horizons | Voxy |
|---|---|---|---|---|
| Source of truth | Versioned, reconstructible Mclone worldgen/semantic source shared with exact terrain | Direct Minecraft density and biome query; narrow saved-NBT structure overlay | Realized chunks plus optional Minecraft world generation to surface/features/full-server stages | Loaded Minecraft chunk sections, region imports, or DH imports |
| Core representation | Procedural heightfield tiles plus water, material, and vegetation summaries; exact chunks nearby | One upper-surface height, biome ID, RGB, and tree bit per sample | 64 x 64 horizontal columns containing variable top-to-bottom material/biome/light spans | Full 3D 32 x 32 x 32 voxel sections at five mip levels |
| Spatial selection | Regular toroidal geometry clipmap; every enabled level is 4 x 4 | Fixed 512-block regions assigned one distance-band level | Distance-adaptive 2D quadtree of sections | 3D hierarchy traversed on the GPU by projected size, frustum, and Hi-Z |
| Resource behavior | Fixed logical residency and guard counts per preset; entering-strip refill | Active and persistent data grows with covered/travelled area | Persistent/full-data and render work grows with generated/imported area | Persistent volumetric data grows with ingested volume; visibility and mesh selection are adaptive |
| Far edits/content | Natural procedural base; exact ready coverage owns edits and actors, with future summaries needed farther out | Heightfield ignores edits; only a narrow saved-structure island path sees NBT | Chunk ingestion/update propagation retains realized block/biome/light columns and generated features | Chunk ingestion propagates block/biome/light voxel changes through parent mips |
| Exact transition | Focus-connected exact component, binary ownership, complete coarse fallback, preferred fine support, immutable frontier certificate | Radial alpha overlap, skirts, fog, and whole-region replacement | Quadtree render sections built from stored column data | Parent fallback while GPU traversal requests/descends into available children |
| Volumetric fidelity | Deliberately surface-first; caves, arches, and overhangs remain a known future boundary | Surface-only except saved near-boundary structure islands | Multiple vertical spans per column, reduced by vertical-quality policy | True 3D voxel hierarchy with greedy faces, fluids, materials, biomes, and light |
| Network posture | Procedural LOD only for compatible reconstructible local sources; otherwise Off | Integrated single-player Overworld only | Mature client/server full-data request, response, partial-update, split, generation, and rate-limit paths | Primarily client-side ingestion/import/storage; no equivalent direct seed sampler is required |
| Rendering/platform | Shared Rust/`wgpu`: native, WebGPU, Android, XR, mono/stereo/multiview | Minecraft Java renderer integration for 1.20.1 loaders; exact artifact inspected is Fabric | Minecraft Java/OpenGL renderer with a broad mature compatibility surface | Minecraft Java/OpenGL 4.6-style compute, subgroup, Hi-Z, and indirect-draw pipeline |

### Versus Mclone

The strongest commonality is direct procedural sampling. Ecstatic avoids the
cost of fully realizing far chunks, just as Mclone's horizon source is meant
to answer stable coarse queries without expanding the authoritative exact
world. Power-of-two spacing, coarse visual simplification, and vegetation
proxies are also recognizably aligned.

The differences are more consequential:

- Mclone's unit of residency is a toroidal presentation slot; Ecstatic's is a
  persistent world region. Ecstatic therefore resembles the lifecycle and
  area-scaling risks of Mclone's retired chunk-based Far LOD more than the
  current clipmap, despite using much larger regions and coarser samples.
- Mclone's source identity couples profile, seed, topology, exact generation,
  and procedural program. Ecstatic's flat files lack equivalent invalidation.
- Mclone commits complete levels and one exact-frontier ownership certificate.
  Ecstatic swaps regions independently and visually hides overlap.
- Mclone's source-side frequency filtering removes detail that a coarse
  lattice cannot represent. Ecstatic queries the final density at sparse
  points and uses shading/fog to disguise some resulting loss.
- Mclone's vegetation summaries are stable semantic source facts with bounded
  resident canopy cells. Ecstatic infers “tree-capable biome” and statistically
  synthesizes local proxies for every active region.
- Mclone's contract is host-neutral and multiview-aware. Ecstatic is tightly
  coupled to one Minecraft version's integrated server and renderer.

Ecstatic's coarse bootstrap is a sound progressive-delivery idea, but Mclone
already has the stronger form: retained parent coverage, coarse-first staged
refill, atomic level admission, and a complete exact-frontier fallback.

### Versus Distant Horizons

Distant Horizons is a richer realized-world data system. `FullDataSourceV2`
owns 64-by-64 horizontal data columns, and each column contains a variable
number of packed vertical spans. Each 64-bit full data point identifies a
block-state/biome pair, height, bottom Y, block light, and sky light. That full
data is explicitly the source of truth from which bounded vertical render
slices and optional texture palettes are derived.

DH can ingest already realized chunks or generate missing terrain to selected
Minecraft stages: pre-existing only, surface, features, or internal-server
full chunks. Parent sections are updated from children, SQLite-backed data is
compressed and migrated, a distance-adaptive quadtree selects render sections,
and current source includes multiplayer full-data request/update protocols.

That buys fidelity Ecstatic cannot provide: multiple exposed vertical
materials, lighting, imported edits, and structures/features when the chosen
generation path realizes them. It also explains the greater CPU, memory,
storage, compatibility, and update-system complexity. Ecstatic's statement
that it avoids loading far chunks is accurate for its surface path, but it is
not evidence that DH is architecturally misguided; DH is solving a broader
truth-and-update problem.

For Mclone, DH is most relevant if distant edited or server-authored terrain
must become durable and streamable. It is less relevant to the untouched
natural horizon, which Mclone can reconstruct more cheaply from its own
versioned source.

### Versus Voxy

Voxy is the most volumetric and GPU-driven of the group. It converts actual
16-cubed Minecraft chunk sections into block/biome/light voxel IDs, constructs
all local 2-by-2-by-2 mips, and propagates them through five levels of
32-cubed `WorldSection` storage. The coarsest level makes an entire 16-cubed
chunk section one voxel. It can also import Minecraft region files or DH's
SQLite full data.

CPU workers build greedy block/fluid faces for a selected 3D section. A GPU
compute traversal evaluates each node's projected screen area, frustum, Hi-Z
occlusion, render distance, child availability, and mesh availability. It
either renders a parent, descends, or feeds a bounded child/mesh request queue;
indirect draw paths consume the resulting list. Storage backends include
RocksDB, LMDB, Redis, and memory configurations.

This is fundamentally unlike Ecstatic's heightfield and Mclone's current
surface clipmap. Voxy preserves overhangs, caves visible from outside,
structures, floating geometry, and arbitrary voxel edits at a cost that scales
with ingested three-dimensional content. Its screen-space adaptive selection
and Hi-Z traversal are sophisticated renderer lessons, but its OpenGL compute
requirements, full Minecraft material model, persistent volume, and importer
pipeline are not suitable as a wholesale Mclone replacement.

Voxy is most relevant to a future **sparse volumetric companion layer** for
Mclone—structures, arches, cliffs, cave mouths, floating land, or authored
edits that cannot collapse to a heightfield. The bulk natural horizon should
remain the cheaper clipmap unless evidence shows otherwise.

## Mclone direction

No current LOD architecture change follows from this investigation.

Keep:

- direct, versioned procedural queries for untouched distant terrain;
- fixed toroidal residency and entering-strip work;
- source-side scale filtering rather than sparse final-density aliasing;
- exact/procedural binary ownership with complete fallback;
- generation-safe stale completion rejection;
- semantic water/material/vegetation summaries; and
- one shared `wgpu` contract across native, Web, Android, and XR.

Do not copy:

- fixed world-region identity as the active streaming topology;
- raw registry IDs or un-fingerprinted derived files;
- a hard-coded sea plane;
- biome-presence-only vegetation as world truth;
- independent region swaps hidden mainly by alpha/fog;
- uncancelled sampling that can outlive its request; or
- renderer/version reflection as the source contract.

Three bounded ideas remain worth considering only when a concrete need and
measurement justify them:

1. **Derived cold cache.** Cache expensive procedural tile results only with a
   complete source descriptor, format revision, integrity check, bounded
   eviction, and safe regeneration. Ecstatic demonstrates the startup value
   and the invalidation hazards.
2. **Sparse volumetric companion.** Represent only facts that cannot be
   reduced to a surface, using real shared producers and summaries. Voxy is a
   better technical reference than Ecstatic's narrow saved-structure islands.
3. **Per-node projection/visibility adaptation.** Voxy's screen-space and Hi-Z
   decisions may help a future measured overdraw or zoom problem. They should
   remain downstream presentation policy and must not compromise the current
   clipmap's coverage certificate. Ecstatic's project page describes zoom
   promotion as future work; it is not an implemented 1.3.0 lesson.

## Code map

Ecstatic exact-release entry points:

- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/sample/SurfaceSampler.java`
- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/lod/RegionLodCoordinator.java`
- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/storage/LodRegionFile.java`
- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/render/LodRegionMesh.java`
- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/render/LodRenderer.java`
- `reference/ecstatic/decompiled-1.20.1-fabric-1.3.0/com/angryalchemist/ecstatic/render/LodStructureIslands.java`

Distant Horizons comparison entry points:

- `reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/fullData/sources/FullDataSourceV2.java`
- `reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/util/FullDataPointUtil.java`
- `reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/render/ColumnRenderSource.java`
- `reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/render/QuadTree/LodQuadTree.java`
- `reference/distant-horizons/coreSubProjects/api/src/main/java/com/seibel/distanthorizons/api/enums/worldGeneration/EDhApiDistantGeneratorMode.java`

Voxy comparison entry points:

- `reference/voxy/src/main/java/me/cortex/voxy/common/world/WorldSection.java`
- `reference/voxy/src/main/java/me/cortex/voxy/common/world/WorldUpdater.java`
- `reference/voxy/src/main/java/me/cortex/voxy/common/world/service/VoxelIngestService.java`
- `reference/voxy/src/main/java/me/cortex/voxy/client/core/rendering/building/RenderDataFactory.java`
- `reference/voxy/src/main/java/me/cortex/voxy/client/core/rendering/hierachical/HierarchicalOcclusionTraverser.java`
- `reference/voxy/src/main/resources/assets/voxy/shaders/lod/hierarchical/screenspace.glsl`

## External references

- [Ecstatic on CurseForge](https://www.curseforge.com/minecraft/mc-mods/ecstatic)
- [Ecstatic authorized source mirror](https://github.com/Wizardtastic/Ecstatic)
- [Distant Horizons official GitLab](https://gitlab.com/distant-horizons-team/distant-horizons)
- [Voxy official GitHub](https://github.com/MCRcortex/voxy)
