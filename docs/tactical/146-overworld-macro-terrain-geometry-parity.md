# 146: Overworld Macro Terrain Geometry Parity

Status: active parent; opened 2026-07-06. Slice A and Slice B landed: native
worldgen now has deterministic macro-geometry signal tests for baseline terrain,
eroded-badlands pillars, and stone-shore / steep-coast anchors.

Workstream: native Rust worldgen. Goal: deterministic Minecraft Java 1.17.1
overworld macro terrain shape parity before screenshot-led validation. This
tracks terrain geometry and carved landforms, not biome tint, not ordinary
feature breadth, and not structures.

## Purpose

Tactical `135` answers whether every overworld biome reads correctly by
identity, tint, surface material, and high-signal visible feature families.
This document answers a different question: whether the landform geometry itself
looks and matches like vanilla at the macro level.

Macro geometry includes:

- height envelope and local relief
- steep slopes and exposed vertical faces
- overhang / solid-over-air signatures from the 3D density field and carvers
- exposed cave mouths and ravine/canyon cuts
- shore, river-bank, and stone-shore height transitions
- badlands pillar columns and shattered-savanna extreme relief
- ice-spike / iceberg massing, even though those are implemented through
  feature/decorator steps

The first goal is not to chase screenshots. The goal is to attach seed/chunk
anchors, exact Java oracle fixtures where available, and derived deterministic
metrics so we know which macro shapes are actually represented.

## Scope Boundaries

In scope:

- Java 1.17.1 overworld `NoiseSampler` / `NoiseBasedChunkGenerator` terrain
  shape.
- Surface and bedrock material passes where they affect macro landform
  recognizability.
- Classic AIR and LIQUID carvers: caves, ravines, underwater carved floors, and
  exposed cave-mouth signatures.
- Surface-structure-like non-building features that affect macro silhouette:
  ice spikes, ice patches, icebergs, blue ice, and badlands pillars.

Out of scope:

- Villages, outposts, pyramids, monuments, shipwrecks, mineshafts, strongholds,
  fossils, desert wells, and other structure families. Track those in structure
  tacticals.
- 1.18+ terrain goals. Do not port `Aquifer`, `Cavifier`,
  `NoodleCavifier`, `OreVeinifier`, disabled deepslate replacement, or the
  1.18 density-function/spline stack for the 1.17.1 MVP target.
- Render LOD or physics terrain collider approximations. Those may use macro
  facts later, but they do not define worldgen parity.
- Screenshots as the primary acceptance signal. Screenshots remain useful
  sanity checks after deterministic probes pass.

## Reference Source

Read before changing macro terrain generation or probes:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseSampler.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/carver/WorldCarver.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/carver/CaveWorldCarver.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/carver/CanyonWorldCarver.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/IcebergFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/IceSpikeFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/surfacebuilders/`
- `docs/reference-minecraft.md`
- `docs/worldgen-status.md`
- `docs/carver-status.md`
- `docs/worldgen-deterministic-order.md`
- `native/crates/mclone-worldgen/src/levelgen.rs`
- `native/crates/mclone-worldgen/src/carver.rs`
- `native/crates/mclone-worldgen/src/feature/ice.rs`

## Relationship To Existing Trackers

- `135-overworld-biome-palette-matrix.md` covers biome identity, tint, surface
  family, and high-signal visible feature families. It can provide seed/chunk
  candidates, but it does not prove macro geometry.
- `103-decorated-biome-fixture-matrix.md` covers exact decorated `FEATURES`
  parity for selected chunks. Use it when macro features are tied to exact
  decorator order or random-stream drift.
- `carver-status.md` owns classic carver implementation status and exact
  carved-stage fixture coverage. This doc consumes that status and asks whether
  exposed cave/ravine macro shapes are represented broadly enough.
- Structure docs own generated buildings and structure terrain interaction.
  This doc should not become a structure backlog.

## Current Coverage Snapshot

Landed:

- Core Java 1.17.1 terrain path exists in native Rust: PRNG/noise,
  `NoiseSampler`, `NoiseBasedChunkGenerator`-style density fill, bedrock,
  surface material pass, and runtime generation entry points.
- Exact terrain-only oracle anchor exists for seed `12345`, chunk `(0,0)`.
- Exact surface/bedrock oracle anchors exist for seed `12345`, chunk `(0,0)`,
  plus frozen-ocean, badlands, giant-taiga, shattered-savanna, and
  mushroom-field material-family chunks recorded by `carver-status.md`.
- Eroded badlands tall pillar surface has an exact Java surface oracle at seed
  `868`, chunk `(8,-6)`.
- Classic AIR and LIQUID carvers are implemented and integrated. Current
  carved-stage exact fixtures include plains, plains-neighbor cave, ocean,
  desert, badlands, giant taiga, shattered savanna, mushroom, and two liquid
  carved chunks.
- `135` already has seed/chunk anchors for the visible macro families:
  mountains, snowy mountains, gravelly mountains, modified gravelly mountains,
  taiga mountains, snowy taiga mountains, stone shore, shattered savanna,
  shattered savanna plateau, eroded badlands, ice spikes, frozen ocean, and
  deep frozen ocean.
- Stone-shore / steep-coast exact Java terrain and surface fixtures exist for
  seed `74739`, chunks `(0,0)` and `(6,8)`.
- `native/crates/mclone-worldgen/src/levelgen/tests.rs` has a reusable
  `MacroGeometrySignal` helper for test chunks and pinned values for seed
  `12345`, chunk `(0,0)`, eroded-badlands seed `868`, chunk `(8,-6)`, and
  stone-shore seed `74739`, chunks `(0,0)` and `(6,8)`.

Not yet landed:

- A broad macro-geometry matrix with deterministic pass/fail rows.
- Exact Java oracle coverage for representative mountain, shattered-savanna,
  and ice-spike chunks chosen for macro shape rather than palette coverage.
- A clear answer for which overhang signatures are density-field terrain,
  carver-created voids, or post-surface feature massing.

## Deterministic Metrics

Exact block diffs remain the parity bar whenever a Java fixture exists. The
metrics below are screening and progress tools: they prove a candidate chunk
actually exercises the intended macro shape and make broad coverage less
subjective.

Initial metric set:

- `top_y_min`, `top_y_max`, and `top_y_range`: top non-air world height envelope
  per chunk.
- `top_y_p05`, `top_y_p50`, `top_y_p95`: relief distribution without letting
  one spike dominate. The current helper uses a sorted nearest-rank sample.
- `neighbor_delta_ge_4`, `neighbor_delta_ge_8`, `neighbor_delta_ge_16`: adjacent
  top-column height jumps for slopes/cliffs.
- `vertical_face_columns`: columns with exposed solid side faces over a minimum
  contiguous height. The current helper uses in-chunk neighbors and a four-block
  contiguous exposed-side run.
- `solid_over_air_blocks`: solid non-fluid blocks with air-like block below;
  useful for overhang and ceiling signatures, especially after carvers.
- `surface_near_carved_air_columns`: top-surface columns with cave air or
  carved air within a small vertical window below or beside the surface. The
  current helper samples the same column plus cardinal in-chunk neighbors from
  `top_y - 6` through `top_y + 2`.
- `carved_air_volume`, `carved_air_y_range`, and `long_vertical_air_spans`:
  cave/ravine magnitude and shape. The current helper counts vertical cave-air
  spans of eight or more blocks.
- `water_land_edge_delta_ge_4` / `ge_8`: shore, river-bank, and stone-shore
  transition strength.
- `landmark_block_volume`: packed ice, blue ice, terracotta/red sand,
  stone/gravel, and other macro-signature block families.
- `landmark_column_count`: ice-spike, iceberg, and badlands-pillar columns above
  a biome-specific Y threshold.

Metric thresholds should be row-specific and source-backed where possible. Do
not tune thresholds from screenshots alone; prefer Java fixture metric values,
then assert native matches or stays within a deliberately documented tolerance.

## Macro Matrix

Legend:

- `Exact`: strict Java fixture/block diff already exists for the relevant
  generation stage.
- `Candidate`: good seed/chunk anchor exists, but macro metric probes or exact
  fixture coverage are still missing.
- `Needed`: the next deterministic proof to add before claiming the row.

| Macro family | Seed / chunk anchors | Existing coverage | Needed |
|---|---:|---|---|
| Baseline 1.17.1 noise terrain | seed `12345`, chunk `(0,0)` | Exact terrain-only and surface/bedrock oracle anchor; macro signal pinned at top Y `86..98`, p05/p50/p95 `86/88/93`, no steep, exposed-face, carved-air, shore, or landmark signal | Use as a control row when adding new macro metric helpers |
| Mountains | seed `33`, chunk `(0,0)` | Palette matrix covers mountain surface/tree family | Java terrain/surface fixture plus height envelope, relief, steep-face, and exposed stone/gravel metrics |
| Wooded / gravelly mountain variants | wooded seed `58`, `(0,0)`; gravelly seed `250`, `(0,0)`; modified gravelly seed `1831`, `(0,0)` | Palette matrix covers surface families and sparse tree family | Variant-specific gravel/stone surface volume plus relief comparison against Java fixtures |
| Snowy mountain variants | snowy mountains seed `326`, `(0,0)`; taiga mountains seed `6126`, `(0,0)`; snowy taiga mountains seed `12006`, `(0,0)` | Palette matrix covers snowy/spruce/fern rows | Snow/top-layer plus relief metrics; decide whether these reuse the mountain metric row or need separate snow-top acceptance |
| Stone shore / steep coast | seed `74739`, chunks `(0,0)` and `(6,8)` | Exact terrain/surface oracle anchors; `(0,0)` pins stone relief at top Y `62..78`, p05/p50/p95 `62/65/76`, deltas `11/0/0`, `9` exposed-face columns, `1163` above-sea stone/gravel blocks, and no water-edge signal; `(6,8)` pins shoreline relief at top Y `62..83`, p05/p50/p95 `62/62/81`, deltas `69/58/23`, `35` exposed-face columns, `117` solid-over-air blocks, water-edge deltas `57/57`, and `798` above-sea stone/gravel blocks | Use as the control row for future shore/coast metric helper changes |
| River and frozen-river banks | river seed `39`, `(0,0)`; frozen river seed `252`, `(0,0)` | Palette matrix covers water/seagrass or frozen-river sugar cane | Bank width, water-land delta, and boundary-shape metrics; later exact fixture if drift is found |
| Shattered savanna relief | shattered seed `68`, chunk `(-6,0)`; plateau seed `153`, chunk `(-8,-2)` | Palette matrix covers shattered surface and acacia family; carved-stage fixture exists for seed `12345`, chunk `(60,199)` | Extreme relief, vertical face, solid-over-air, and Java surface fixture coverage for the chosen palette anchors |
| Eroded badlands pillars | seed `868`, pillar oracle chunk `(8,-6)` | Exact Java surface oracle exists; macro signal pinned at top Y `67..122`, p05/p50/p95 `67/87/122`, neighbor deltas `154/74/41`, `108` exposed-face columns, `7023` badlands landmark blocks, and `144` landmark columns at/above Y `80` | Add a second eroded-badlands or badlands-plateau anchor only if future rows show this one is too narrow |
| Badlands plateaus | badlands seed `28`, chunk `(-2,-8)`; plateau seed `947`, chunk `(-6,-2)`; wooded seed `4764`, chunk `(6,-3)` | Palette matrix covers terracotta/red sand, dead bush/cactus, and wooded split | Plateau height envelope, terrace/block-family volume, and slope/edge metrics |
| Ice spikes | seed `59`, chunk `(0,0)` | Palette matrix covers packed-ice spike/patch presence | Exact spike/patch geometry or at least Java-derived spike column/height/volume metrics |
| Frozen-ocean icebergs | frozen seed `779`, `(0,0)`; deep frozen seed `1679`, `(0,0)` | Palette matrix covers packed/blue icebergs and blue ice | Iceberg volume/height/waterline metrics; later exact `IcebergFeature` mismatch buckets |
| Cave mouths and ravines | seed `12345` carved fixtures: `(0,0)`, `(0,1)`, `(96,-64)`, `(-320,99)`, `(-9,68)`, `(60,199)`, `(-446,387)` | Exact AIR carved-stage fixtures exist across several material families | Derived exposed-carved-air, long vertical air span, ravine-width, and cave-mouth metric rows |
| Underwater carved floors | seed `12345`, chunks `(117,-128)` and `(-129,-256)` | Exact LIQUID carved-stage fixtures exist | Underwater floor / magma / obsidian / pending-tick macro metrics; runtime liquid shape stays in liquid tacticals |
| Mushroom shore / low island transitions | mushroom shore seed `7056`, `(0,0)` | Palette matrix covers mycelium shore transition and huge mushrooms | Lowland island edge, waterline, and surface transition metrics if this becomes visually weak |

## Slice A: Metric Helper And First Anchors

Status: landed.

Implemented as focused native worldgen tests:

- `macro_geometry_signal_tracks_baseline_terrain_anchor`
- `macro_geometry_signal_tracks_eroded_badlands_pillar_anchor`

This slice is measurement-only. It does not change generation behavior. The
helper consumes `MutableChunkBlockBuffer` and produces deterministic metrics for
height envelope, relief distribution, adjacent top-height deltas, exposed
vertical faces, solid-over-air, near-surface carved air, carved-air volume/range,
water-land edges, and row-specific landmark massing.

Pinned values:

| Anchor | Top Y | p05 / p50 / p95 | Neighbor deltas ge 4 / 8 / 16 | Exposed-face columns | Carved / overhang signal | Landmark signal |
|---|---:|---:|---:|---:|---|---|
| seed `12345`, chunk `(0,0)`, terrain stage | `86..98` | `86 / 88 / 93` | `0 / 0 / 0` | `0` | none | none |
| seed `868`, chunk `(8,-6)`, surface stage | `67..122` | `67 / 87 / 122` | `154 / 74 / 41` | `108` | none | `7023` badlands blocks; `144` columns at/above Y `80` |

## Slice B: Stone Shore / Steep Coast

Status: landed.

Implemented as exact Java fixture tests plus macro signal tests:

- `fills_stone_shore_chunk_with_terrain_only_java_oracle`
- `fills_stone_shore_edge_chunk_with_terrain_only_java_oracle`
- `build_stone_shore_surface_and_bedrock_matches_java_oracle`
- `build_stone_shore_edge_surface_and_bedrock_matches_java_oracle`
- `macro_geometry_signal_tracks_stone_shore_steep_coast_anchor`
- `macro_geometry_signal_tracks_stone_shore_water_edge_anchor`

The first planned anchor, seed `74739`, chunk `(0,0)`, is a useful stone-shore
relief control but has no in-chunk water-land edge by the current metric. A
nearby deterministic scan found seed `74739`, chunk `(6,8)`, which keeps the
same seed family and provides the stronger shoreline transition.

Pinned values:

| Anchor | Top Y | p05 / p50 / p95 | Neighbor deltas ge 4 / 8 / 16 | Exposed-face columns | Shore / overhang signal | Landmark signal |
|---|---:|---:|---:|---:|---|---|
| seed `74739`, chunk `(0,0)`, surface stage | `62..78` | `62 / 65 / 76` | `11 / 0 / 0` | `9` | no water-edge or solid-over-air signal | `1163` stone/gravel blocks at/above sea level; `137` columns |
| seed `74739`, chunk `(6,8)`, surface stage | `62..83` | `62 / 62 / 81` | `69 / 58 / 23` | `35` | water-edge deltas `57 / 57`; `117` solid-over-air blocks | `798` stone/gravel blocks at/above sea level; `118` columns |

## Suggested Next Slice

Add the next visible macro row that is not already covered by an exact oracle.
The best next target is shattered savanna relief:

1. Generate Java terrain and surface fixtures for seed `68`, chunk `(-6,0)`.
2. Pin top-height envelope, extreme relief, exposed vertical faces,
   solid-over-air, and acacia/surface landmark metrics.
3. If that anchor is too narrow, add plateau seed `153`, chunk `(-8,-2)` before
   changing generator behavior.

Do not start by changing density math. The first pass should tell us what the
current native generator already does and where the Java fixture says it differs.

## Validation

Docs-only updates use:

```bash
git diff --check -- docs/tactical/146-overworld-macro-terrain-geometry-parity.md docs/tactical/README.md
```

Use focused native tests first:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen fills_chunk_zero_zero_with_terrain_only_java_oracle
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen build_surface_and_bedrock_matches_java_oracle
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen build_eroded_badlands_pillar_surface_and_bedrock_matches_java_oracle
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen stone_shore
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen macro_geometry_signal_tracks
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen overworld_air_carvers_match_java
```

Generate new Java fixtures with the existing oracle entry points, for example:

```bash
pnpm --silent oracle:gen terrain-chunk --seed <seed> --chunk-x <x> --chunk-z <z>
pnpm --silent oracle:gen surface-chunk --seed <seed> --chunk-x <x> --chunk-z <z>
pnpm --silent oracle:gen carved-chunk --seed <seed> --chunk-x <x> --chunk-z <z>
```

For any slice that changes generated pixels, run the native screenshot lane
after deterministic probes pass and inspect the image before moving on.
