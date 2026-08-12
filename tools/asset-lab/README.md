# Mclone Asset Lab

Disposable TypeScript/Three.js lab for AI-authored box-and-card figures.
Rounded primitive sources are retained only under `legacy-examples/` for
explicit A/B and schema-compatibility review.

Install once:

```sh
pnpm --dir tools/asset-lab install
```

Common commands from the repo root:

```sh
pnpm asset-lab:typecheck
pnpm asset-lab:test
pnpm asset-lab:figures:check
pnpm asset-lab:geometry:check
pnpm asset-lab:surface:check
pnpm asset-lab:export
pnpm asset-lab:smoke
pnpm asset-lab:sheet
pnpm asset-lab:video
pnpm asset-lab:batch
pnpm asset-lab:preview
pnpm asset-lab:compare
```

## Interactive web catalogue

The production-quality, read-only creature catalogue is another Asset Lab
display path. It discovers every canonical `examples/*/figure.ts`, crosses the
same serialize/reparse validation boundary as preview and review commands, and
generates semantic JSON, SHA-256 metadata, and deterministic thumbnails before
building the React/Three.js application.

```sh
pnpm asset-lab:web
pnpm asset-lab:web:build
pnpm asset-lab:web:test
```

The local development command serves `/animals/`. The production build writes
to the ignored `tools/asset-lab/dist/web/` directory with a Vite base of
`/animals/`; the normal native-web bundle copies that exact subtree into the
deployed site. The live entry point is:

```text
https://mclone.kzahel.com/animals/
```

The catalogue browser never imports `figure.ts`. TypeScript sources execute on
the build host, while visitors fetch parsed, validated, hash-checked semantic
JSON. Deprecated rounded sources under `legacy-examples/` are excluded, and
catalogue presence does not imply promotion into the runtime asset pack.

Promoted static semantic props use the same generated JSON, validation,
thumbnail, viewport, and pack path without entering creature counts or
classification filters. Open the deliberately separate review mode with
`/animals/?view=props`; adding a source under `props/` is rejected unless the
checked promotion registry declares its exact `world_prop`, `item_prop`, or
`trace_prop` use and matching `ground`, `item_center`, or `surface_trace`
anchor. Props may have zero clips; the viewer presents them explicitly as
static assets instead of requiring a fake idle animation.

Promotion also records `live_gameplay` or `review_only` instantiation status.
Packing a review candidate is not proof that ordinary gameplay creates or
renders it. The prop review summary reports live instantiation separately, and
an asset may move to `live_gameplay` only with ordinary runtime evidence.

The generated manifest derives runtime-promotion status from
`src/first-party-figures.ts`, the checked mapping that owns promoted semantic
JSON. The UI shows the promoted count, offers a runtime-status filter, badges
promoted rows, and displays their runtime asset ID, use, anchor, and packed
JSON path. React
does not maintain a second promotion list.

Canonical sources can declare typed creature classification through the DSL's
`metadata()` helper. `groups`, `bodyPlans`, and `habitats` are nonempty tag
sets; `disposition` and `scale` are single typed values; and optional `themes`
are lowercase search tags. Multiple values are intentional: a Gargoyle can be
both `fantasy` and `monster`, both `biped` and `winged`, and both `land` and
`air`. The semantic JSON round-trip validates and preserves explicit metadata.
Living growths use the same contract: `plant` and `fungus` are groups, while
`rooted` and `colony` describe body plans without forcing a growth to be an
animal or monster.

Catalogue generation deterministically infers conservative metadata for older
sources that do not yet declare it, so classification did not require a
flag-day rewrite of the existing roster. New creatures, and existing creatures
when materially edited, should author metadata explicitly. The catalogue
manifest always carries the resolved result, and the UI exposes group, body
plan, habitat, disposition, and metadata-aware search filters.

The development preview and catalogue both use `src/viewport.ts` over the same
`src/scene.ts` semantic renderer. React owns navigation and controls only; it
does not create geometry or reinterpret animation keys.

The smoke command writes screenshots under `/tmp/mclone-asset-lab/` by default.
The exported figure JSON is also written under `/tmp` unless `--out` is passed.
The sheet command writes a larger review image with front, side,
three-quarter, side animation, and three-quarter animation captures. Sheets use
the clip locomotion metadata to scroll the floor at sampled frames.
The video command captures deterministic Playwright frames and uses `ffmpeg` to
write a multi-cycle MP4 animation review at
`/tmp/mclone-asset-lab/chicken-walk.mp4`. Walk clips keep the figure centered and
move the floor backward by the authored cycle distance so foot sliding is easy
to see.

The batch command discovers `examples/*/figure.ts`, exports each asset, renders
each sheet, and writes MP4 reviews under `/tmp/mclone-asset-lab/`. New example
directories are included automatically; [`ANIMALS.md`](ANIMALS.md) is the
current creature roadmap and conversion queue. The deprecated rounded sources
in `legacy-examples/` are deliberately excluded unless passed to a command by
their explicit path.

The compare command accepts a canonical TypeScript source or semantic JSON and
renders the same serialized asset through Three.js and the shared native
startup-prepared renderer. TypeScript inputs are exported into a temporary
asset root under the review output; they are never written into the repository.
The command writes corresponding raw panels, an unscaled labeled sheet, the
shared review contract, and diagnostic receipts under
`/tmp/mclone-figure-compare/` by default. The receipts record source/compiler
identity, framing, topology, atlas, preparation, and immutable upload counts;
they are review output, not a persisted asset format.

## Source and generated JSON

Asset files use the DSL from `src/dsl.ts`. A `figure.ts` file is the only
human- or AI-authored source for a promoted actor or prop. Its schema-v1 JSON is a
generated semantic snapshot and must not be edited directly.

Canonical sources use `figure()` with boxes and fixed finite planes. Boxes
remain the ordinary volumetric vocabulary. `plane({ size: [width, height],
sidedness: "front" | "double" })` is reserved for genuinely planar details
such as petals, leaves, fins, wings, or cloth; it is fixed in the part rig and
is not a camera-facing sprite. `legacyFigure()` plus its sphere, capsule, and
cylinder helpers exist only so retained rounded A/B sources and schema-v1
compatibility fixtures remain executable. The first-party drift gate rejects
any promoted source containing a primitive other than a box or plane.

Every Asset Lab display path crosses that snapshot boundary. When previewing a
`figure.ts`, the tool executes the DSL, serializes canonical JSON, reparses and
validates it, and gives only that parsed result to Three.js. Preview, sheet,
smoke, and video commands also accept a `figure.json` path directly. The viewer
never renders the live module object through a shortcut.

The checked runtime actors and props are mapped to their sources by
`src/first-party-figures.ts`. Regenerate them and review the diff with:

```sh
pnpm asset-lab:figures:write
pnpm asset-lab:figures:check
```

The check fails for stale or missing output and for any promoted
`assets/mclone/figures/*.figure.json` without a declared TypeScript source.
After a checked semantic asset changes, refresh and verify the normal
first-party asset packs. `asset-lab:test` discovers and executes every Asset
Lab creature example plus checked prop sources through the serialize/reparse,
geometry, ground, and surface boundaries. The public Creature Catalogue still
discovers the full creature roster independently of prop review.

Canonical figures also pass a rest-pose geometry connectivity gate. The check
transforms every box and an analysis-only epsilon-thick bound for every plane
into figure space, expands pairs by a small `0.06`-unit attachment tolerance,
builds connected components, and rejects every component outside the largest
one. It runs while `figure()` constructs a canonical
source, during catalogue builds, and from `asset-lab:test`; run it directly
with `pnpm asset-lab:geometry:check` for a catalogue summary.

Do not use a rig parent link as evidence that boxes touch. If a component is
intentionally separate, acknowledge the exact current component and explain
why:

```ts
figure("wisp", ({ box, geometryException, mat, part }) => {
  mat("glow", "#88ccff");
  part("body", box({ size: [0.5, 0.5, 0.5], material: "glow" }));
  part("halo", box({ at: [0, 0.8, 0], size: [0.3, 0.05, 0.3], material: "glow" }));
  geometryException({
    rule: "disconnected-component",
    parts: ["halo"],
    reason: "The magical halo intentionally floats above the body.",
  });
});
```

The reason must be nonempty, the part set must exactly match the reported
component, and an exception becomes an error when later geometry reconnects or
changes the component. This keeps exceptions narrow, reviewed, and removable.
The gate is an oriented-bound rest-pose check, not collision detection or an
animated-pose proof; visual sheet/video review remains required.

Land figures also pass a sampled-pose ground-penetration gate. It evaluates
rest, land locomotion, idle, and action poses; transforms every box or plane
corner; and
reports non-contact parts more than `0.02` units below figure-space `y=0`.
Explicit `metadata.habitats` containing `land` admits a figure even when its
motion is entirely custom; older sources remain admitted through land
locomotion inference.
Swim/flight clips and exact boxes declared as locomotion contacts are excluded.
Run `pnpm asset-lab:ground:check -- --verbose` to inspect the ratcheted catalog
warning inventory as well as any failures.

New penetrating parts fail. Existing exact figure/part findings live in
`src/ground-baseline.ts` and become stale when corrected. An intentional new
relationship must name exactly one part and explain why it crosses the ground:

```ts
geometryException({
  rule: "ground-penetration",
  parts: ["burrowing_claw"],
  reason: "This action deliberately pushes the named claw into loose soil.",
});
```

Do not declare a tail or decoration as a locomotion contact to hide a defect.
Contacts are reserved for genuinely load-bearing pads; their own contact-depth
quality remains a separate procedural-animation concern.

Canonical figures also pass a sampled-pose surface-stability gate. It evaluates
rest, clip keys, key midpoints, and a bounded uniform cadence; transforms every
box and plane face; and reports differently rendered, same-facing surfaces
with meaningful projected overlap that is not immediately covered by another
box. Animated
heads also require a safe lateral margin from a `neck`, `neck_*`, `throat`, or
`body` socket throughout the sampled motion. Run
`pnpm asset-lab:surface:check -- --verbose` to see the retained catalog warning
inventory as well as any failures.

Existing findings are exact, ratcheted warnings in `src/surface-baseline.ts`,
not accepted geometry. Newly authored face pairs fail. Removing an old finding
makes its baseline key stale until the key is deleted, so cleanup cannot be
silently forgotten. An intentional new relationship must name both exact faces
and explain why it is acceptable:

```ts
surfaceException({
  rule: "coplanar-overlap",
  faces: [
    { part: "glass_inset", face: "north" },
    { part: "frame", face: "north" },
  ],
  reason: "The flush inset is intentional and uses the same final pixels.",
});
```

Exceptions are order-independent, require a nonempty reason, and become stale
when the pair no longer triggers. The animated socket rule uses
`rule: "articulated-seam-margin"` with the same exact-face contract. Prefer a
deliberate width, height, or depth step. Do not use draw order, polygon offset,
or a whole-part suppression to hide a source-geometry conflict. General
near-plane and between-sample crossing diagnostics remain future extensions,
so sheet and video review are still required.

Three.js remains the semantic preview implementation, not the source format.

Figure materials default to `{ alphaMode: "opaque", opacity: 1 }`. Authors may
select `mask`, `blend`, or `additive` explicitly. Mask materials retain normal
depth writes and use either a configurable cutoff or object-stable dithered
coverage; blend materials use a figure depth prepass followed by premultiplied
source-over color; additive materials contribute color without writing depth.

```ts
mat("spirit", {
  color: "#b7e9ff",
  alphaMode: "blend",
  opacity: 0.42,
});

mat("fading_cloth", {
  color: "#d8edf4",
  alphaMode: "mask",
  opacity: 0.65,
  alphaCoverage: "dither",
});
```

`alphaCutoff` and `alphaCoverage` belong only to `mask`; coverage defaults to
`threshold` and cutoff defaults to `0.1`. Opaque materials require opacity 1.

ASCII texture palettes accept `#RRGGBB`, `#RRGGBBAA`, and the exact literal
`"transparent"`. Texture alpha is interpreted by the material mode. For
backward compatibility, a texture with transparent palette entries used by an
otherwise opaque material is treated as a threshold mask.

```ts
asciiTexture("ribs", {
  palette: { ".": "transparent", "b": "#d8d0b5" },
  pixels: ["b..b", "bbbb", "b..b"],
});
```

Use `"transparent"` for a fully empty texel and `#RRGGBBAA` when a mask,
blend, or additive material needs authored per-texel coverage.

Boxes support Minecraft-style per-face overrides:

```ts
part("head", box({
  size: [0.7, 0.58, 0.58],
  material: "skin",
  faces: {
    north: { texture: "face" },
  },
}));
```

Animated parts can rotate around an explicit local pivot. The pivot is measured
from the part's unrotated local center, so a vertical box leg with height
`0.42` uses `pivot: [0, 0.21, 0]` to swing from its top.

```ts
part("leg_fl", box({
  parent: "body",
  at: [-0.42, -0.53, -0.26],
  size: [0.18, 0.42, 0.18],
  joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
  material: "skin",
}));
```

Use cycle helpers for common walk motion instead of hand-writing every keyframe.
`swing` rotates around an axis in degrees, `bob` translates along an axis, and
`phase: 0.5` makes a track move opposite another track. Track `frequency`
controls repeats per clip. Helpers export ordinary clip keys.

```ts
walkCycle("walk", {
  fps: 12,
  duration: 1,
  samples: 9,
  tracks: [
    bob("body", { axis: "y", amount: 0.0125, center: 0.0125, phase: 0.5 }),
    swing("leg_fl", { axis: "x", degrees: 20 }),
    swing("leg_fr", { axis: "x", degrees: 20, phase: 0.5 }),
  ],
});
```

Procedural tracks can constrain their evaluated scalar with `min` and `max`.
The limit is applied after the waveform is evaluated and before ordinary clip
keys are baked. A lower bound of zero turns the negative half of a bob into an
exact grounded interval without requiring hand-authored keys or a runtime
constraint solver:

```ts
walkCycle("hop", {
  duration: 0.82,
  samples: 33,
  tracks: [
    bob("body", { axis: "y", amount: 0.25, phase: 0.5, min: 0 }),
    swing("hind_leg", { axis: "x", degrees: 28, phase: 0.5, min: 0 }),
  ],
});
```

Constraints are available on `bob`, `swing`, `contactSwing`, and
`followThrough`. They limit that track's scalar output, not a part's final
world-space transform, and therefore do not perform collision detection or IK.

When a procedural pose must react to the transformed bottom of an actual box,
`walkCycle` also accepts authoring-time `groundContacts`. Tracks and
follow-through are sampled first. For each sample whose contact box would pass
below `groundY`, the baker numerically corrects one bounded ancestor hinge until
the box bottom reaches the plane. By default it counter-rotates the contact box
by the same amount so a flat pad stays flat:

```ts
walkCycle("hop", {
  duration: 0.82,
  samples: 33,
  groundContacts: [{
    contactPart: "hind_foot_l",
    solvePart: "hind_shin_l",
    axis: "x",
    minCorrectionDegrees: -65,
    maxCorrectionDegrees: 65,
  }],
  tracks: [
    bob("body", { axis: "y", amount: 0.25, phase: 0.5, min: -0.04 }),
  ],
});
```

This is a deterministic one-plane, one-hinge contact projection. It preserves
the authored hierarchy and emits ordinary clip keys, but it is not a general
multi-joint IK or collision solver. Contact and solve parts must exist, the
contact must be a box below the solve-part ancestor, correction bounds must
contain zero, and an unreachable ground plane rejects the source.

Use gait macros when the anatomy is conventional. They are authoring shortcuts,
not runtime procedural animation. Walk macros export ordinary keyframes plus
`clip.locomotion` metadata describing cycle distance, speed, forward direction,
and stance/contact windows.

```ts
quadrupedWalk("walk", {
  legs: {
    frontLeft: "leg_fl",
    frontRight: "leg_fr",
    backLeft: "leg_bl",
    backRight: "leg_br",
  },
  body: "body",
  contactParts: {
    frontLeft: "hoof_fl",
    frontRight: "hoof_fr",
    backLeft: "hoof_bl",
    backRight: "hoof_br",
  },
  cycleDistance: 0.72,
  gait: "trot",
  stanceRatio: 0.56,
  swingDegrees: 20,
  bodyBob: 0.012,
});
```

```ts
bipedWalk("walk", {
  leftLeg: "leg_l",
  rightLeg: "leg_r",
  leftArm: "arm_l",
  rightArm: "arm_r",
  body: "torso",
  cycleDistance: 0.9,
  leftContact: "foot_l",
  rightContact: "foot_r",
  stanceRatio: 0.62,
});
```

```ts
wingFlap("fly", {
  leftWing: "wing_l",
  rightWing: "wing_r",
  body: "body",
  degrees: 38,
  frequency: 2,
});
```

Aquatic figures use `swim` to generate coordinated body counter-sway, a
primary tail stroke, an optional delayed child-tail stroke, mirrored pectoral
fin motion, and optional vertical body drift. Like the other macros, it emits
ordinary clip keyframes plus locomotion metadata rather than requiring runtime
procedural animation.

```ts
swim("swim", {
  body: "body",
  tail: "tail",
  tailTip: "tail_tip",
  leftFin: "fin_l",
  rightFin: "fin_r",
  bodySwayDegrees: 3,
  tailSwingDegrees: 18,
  tailTipSwingDegrees: 25,
  finSwingDegrees: 8,
  cycleDistance: 1.4,
});
```

Segmented legless figures use `slither` to generate a traveling lateral wave
across an ordered parented chain. Amplitude grows slightly toward the tail;
`phaseStep` controls how far the wave advances between neighboring segments.
The helper emits ordinary clip keys and optional `slither` locomotion metadata.

```ts
slither("slither", {
  body: "body_1",
  segments: ["body_1", "body_2", "body_3", "tail"],
  degrees: 9,
  phaseStep: 0.12,
  bodyBob: 0.01,
  cycleDistance: 0.8,
});
```

Use `contactSwing` when you need a lower-level planted/recovery leg curve
without the full gait macro. `phase` is the contact-start phase; `stanceRatio`
is the fraction of the cycle spent in planted motion.

```ts
walkCycle("walk", {
  duration: 1,
  locomotion: {
    kind: "biped-walk",
    cycleDistance: 0.9,
    direction: [0, 0, -1],
    units: "figure",
  },
  tracks: [
    contactSwing("leg_l", { degrees: 24, phase: 0, stanceRatio: 0.62 }),
    contactSwing("leg_r", { degrees: 24, phase: 0.5, stanceRatio: 0.62 }),
  ],
});
```

The video command accepts the same figure path plus timing options:

```sh
pnpm --dir tools/asset-lab exec tsx src/video.ts examples/chicken/figure.ts \
  --out /tmp/mclone-asset-lab/chicken-walk.mp4 \
  --clip walk \
  --fps 24 \
  --cycles 4
```
