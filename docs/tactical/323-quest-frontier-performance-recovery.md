# Tactical 323: Quest Frontier Performance Recovery

Status: implementation authorized 2026-08-20; constant-time suppression
accepted by measurement, inactive-transition shading measured, and compact
support drawing in progress.

Topics: `procedural-horizon-clipmap`, `performance`,
`quest-frontier-performance`

## Originating Request

After accepting the generation-coherent exact/procedural frontier from
Tactical 321, recover standalone Quest 3 frame headroom without reopening the
accepted seam topology. Start with the three pixel-preserving opportunities
identified in review: constant-time support suppression, inactive-transition
shading elimination, and compact support drawing. Measure each candidate on
the physical headset rather than inferring mobile behavior from desktop.

## Current Evidence

The accepted Low/RD8 production per-eye sample is close to, but not within,
the 72 Hz budget:

- `13.889ms` frame budget;
- `14.524ms` app-work p50 and `18.480ms` p95;
- `6.183ms` thread-CPU p50 and `8.196ms` blocked p50; and
- `58.9%` of sampled frames over period.

The one 9.6-second device/driver outlier makes averages unusable, but does not
invalidate the median or percentile miss. Full-frame multiview is not the
first optimization candidate: its settled sample measured `16.866ms` p50 and
`20.154ms` p95.

At the RD8 review site, the preferred frontier owns 20 support resources and
draws eight visible full `64 x 64` spacing-one tiles, adding approximately
200,000 non-indexed visible vertices. Every coarse fragment also scans the
fixed 32-record support array to decide whether the coarse base is suppressed.
The material path evaluates the near exact-transition response even when the
interpolated transition weight is zero.

## Objective And Non-Goals

Recover a stable production per-eye Low/RD8 path while preserving:

- the exact coverage and accepted preferred/fallback topology;
- the one-owner solid, water, vegetation, and connector rules;
- Low's six clipmap levels and spacing-one frontier character;
- the current material, lighting, fog, time-of-day, and stereo semantics; and
- the same shared renderer on native, WebGPU, flat Android, and XR.

The target is `<=12.5ms` app-work p50 and `<=13.889ms` p95 in a matched
stationary Quest sample. Merely moving p50 below budget is useful evidence but
does not qualify the feature as reliably locked to 72 Hz. This tactical does
not lower render distance, eye scale, LOD reach, material quality, or exact
coverage to manufacture a pass.

## Phase 0: Matched Attribution

Build one release APK and record alternating, settled, 20-second physical
Quest 3 samples with the same seed, pose, daylight, actors, render distance,
eye scale, foveation, and per-eye frame-overlap path:

1. exact-only / LOD Off;
2. Low/RD8 natural preferred support;
3. Low/RD8 forced bounded fallback; and
4. Low/RD8 natural preferred repeat.

Enable XR performance metrics and retain frontier admission, visible support,
connector, exact-section, clipmap-tile, thread-CPU, blocked-time, and app GPU
facts. Treat any sample that begins before preferred admission or settled
terrain readiness as invalid.

## Phase 1: Constant-Time Suppression

Replace the fragment-time linear search through 32 support records with a
bounded direct lookup keyed by the same lifted 64-block support-tile
coordinates. The representation must:

- be immutable for the admitted frontier certificate;
- represent every selected tile without hash collisions or false positives;
- preserve deterministic negative and periodic-coordinate behavior;
- bind once for mono, per-eye, and multiview rendering; and
- retain receipt facts sufficient to prove its bounds and active contents.

Add CPU lookup fixtures and shader-validation coverage. Compare native pixels
before and after, then measure the same preferred Quest lane.

### Phase 1 Evidence

Commit `ca8bc0ea` replaces the 32-record fragment scan with one collision-free
18-by-18 row-mask lookup. The mask spans every legal 65-chunk exact profile,
including negative coordinates, and shrinks fixed suppression storage from
512 to 96 bytes. The 143-test terrain-view suite, native and Wasm checks, and
thin-adapter gate pass. Inspected natural and forced-fallback RD8 captures are
seam-free:

```text
/tmp/mclone-t323-suppression/rd8-elevated-product.png
/tmp/mclone-t323-suppression/rd8-elevated-forced-fallback.png
```

The matched preferred Quest sample measured:

| Candidate | App work p50 / p95 | Thread CPU p50 | Blocked p50 | App GPU | Over-period |
|---|---:|---:|---:|---:|---:|
| record scan, preferred range | `15.072-15.232 / 18.633-18.789ms` | `6.880-6.921ms` | `8.154-8.237ms` | `9.089-9.526ms` | `66.4-66.8%` |
| direct row mask | `13.249 / 14.269ms` | `7.264ms` | `5.975ms` | `7.522ms` | `16.7%` |

This recovers approximately two milliseconds at p50, more than four at p95,
and roughly two milliseconds of app GPU time. It reaches 72 submissions per
second but does not satisfy the strict p95 gate, so Phase 2 remains warranted.
Raw evidence is `/tmp/mclone-t323-suppression-preferred.txt`.

## Phase 0 Evidence

Commit `1d38a647` exposes the existing frontier diagnostic as an Android-XR
launch-only control without changing product defaults. One release APK then
ran the requested alternating Quest 3 matrix at RD8, 72 Hz, 1680-by-1760 per
eye, render scale 1, foveation Off, fixed daylight, and the production per-eye
frame-overlap path:

| Lane | App work p50 / p95 | Thread CPU p50 | Blocked p50 | App GPU | Over-period |
|---|---:|---:|---:|---:|---:|
| exact / Off | `6.342 / 7.073ms` | `4.219ms` | `2.120ms` | `3.024ms` | `0.0%` |
| preferred A | `15.072 / 18.633ms` | `6.880ms` | `8.154ms` | `9.089ms` | `66.8%` |
| forced bounded fallback | `15.228 / 18.937ms` | `6.643ms` | `8.505ms` | `10.029ms` | `67.9%` |
| preferred B | `15.232 / 18.789ms` | `6.921ms` | `8.237ms` | `9.526ms` | `66.4%` |

All Low lanes settled with 96 ready slots, 289 exact columns, six drawn
levels, 47 regular terrain tiles, zero exact-center-not-ready frames, and no
pending vegetation work. The preferred repeat differs by only `0.160ms` at
p50 and `0.156ms` at p95. Fallback removes almost the entire preferred fine
belt but does not improve work or GPU time. That rules out support generation
and makes full support geometry an unlikely first-order explanation. The
common per-fragment 32-record suppression scan and shared material work are
the supported first targets.

Raw evidence:

```text
/tmp/mclone-t323-baseline-exact.txt
/tmp/mclone-t323-baseline-preferred-a.txt
/tmp/mclone-t323-baseline-fallback.txt
/tmp/mclone-t323-baseline-preferred-b.txt
```

## Phase 2: Inactive Transition Shading

Avoid evaluating the exact-side material response when transition weight is
exactly zero. Keep derivative evaluation uniform where WGSL requires it, and
preserve all water, texture, grass-tint, diagnostic, and albedo behavior for a
positive transition weight. Remove any branch made unreachable by the new
control flow.

Require byte-identical or explicitly explained pixel evidence at far and
frontier-focused native views, plus headed WebGPU shader validation, before
Quest measurement.

### Phase 2 Evidence

Commit `51890e91` leaves derivative evaluation uniform, preserves the exact
transition path for positive weights, and skips the exact-style tint and
active-pack sample when the interpolated weight is zero. All 143 terrain-view
tests, native and Wasm checks, the browser WebGPU probe, and the browser
terrain-horizon smoke pass. The inspected browser image is:

```text
/tmp/mclone-native-web-terrain-horizon-toggle-canvas.png
```

The RD2 native capture is byte-identical to the Phase 1 image. At RD8,
ImageMagick reports 143 changed preferred pixels (`0.024%`) and 5,533 changed
forced-fallback pixels (`0.938%`), localized to raster edges with no visible
seam or composition change in inspected images:

```text
/tmp/mclone-t323-transition/rd8-elevated-product.png
/tmp/mclone-t323-transition/rd8-elevated-forced-fallback.png
```

The matched Quest preferred sample measured `13.273 / 14.305ms` app-work
p50/p95, `7.344ms` thread-CPU p50, `5.940ms` blocked p50, `7.483ms` app GPU,
and `17.2%` over-period frames. That is within run variance of Phase 1's
`13.249 / 14.269ms`, while app GPU moves by only `-0.039ms`. The inactive
work was likely already eliminated effectively by the mobile shader compiler;
retain the simpler control flow, but do not credit it with recovered budget.
Raw device evidence is `/tmp/mclone-t323-transition-preferred.txt`.

## Phase 3: Compact Support Drawing

Stop submitting complete support-tile surfaces that are wholly discarded or
outside their certificate-owned visible region. Prefer compact index/range
metadata over a second terrain representation:

- derive draw coverage from the immutable exact mask and support ownership;
- preserve every support cell that can own a visible horizontal surface;
- preserve outer skirts and exact/fallback connectors independently;
- keep resource generation and admission atomic with the certificate; and
- expose candidate versus submitted support cells/vertices in diagnostics.

First prove ordinary square RD8 coverage, negative coordinates, irregular
connected exact shapes, water edges, and pool exhaustion. Inspect native,
headed WebGPU, flat Android, and stereo pixels at the first drawable
milestone. Any changed visible topology or shading pauses for human review.

## Phase 4: Acceptance And Optional Quality Lever

After the three pixel-preserving candidates, rebuild one release APK and run
the Phase 0 preferred/fallback matrix plus a settled Low/RD8 orbit. Accept the
renderer optimization only if topology receipts remain complete, inspected
pixels remain seam-free, and the per-eye path improves outside normal run
variance.

If p95 still misses 72 Hz, measure existing Quest fixed foveation at Low and
Medium levels as a separate, explicitly pixel-changing A/B. Do not silently
change the default. Human acceptance is required before promoting a
foveation level or any other quality tradeoff.

## Completion Gate

- [x] Matched exact/preferred/fallback/preferred Quest baseline is recorded.
- [x] Constant-time suppression is implemented, validated, and measured.
- [x] Zero-weight transition shading is eliminated and pixel-validated.
- [ ] Support submission is compacted without weakening the certificate.
- [ ] Native, headed WebGPU, flat Android, stereo, and Quest gates pass.
- [ ] Final stationary and orbit Quest measurements are recorded honestly.
- [ ] Any remaining foveation decision is separated behind human acceptance.
- [ ] Living clipmap and performance topics contain the settled result.
- [ ] The worktree is clean and implementation commits share the topic
      trailers.
