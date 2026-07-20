# Mclone Asset Lab

Disposable TypeScript/Three.js lab for AI-authored box-only figures. Rounded
primitive sources are retained only under `legacy-examples/` for explicit A/B
and schema-compatibility review.

Install once:

```sh
pnpm --dir tools/asset-lab install
```

Common commands from the repo root:

```sh
pnpm asset-lab:typecheck
pnpm asset-lab:test
pnpm asset-lab:figures:check
pnpm asset-lab:export
pnpm asset-lab:smoke
pnpm asset-lab:sheet
pnpm asset-lab:video
pnpm asset-lab:batch
pnpm asset-lab:preview
pnpm asset-lab:compare
```

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
current animal catalog and conversion queue. The deprecated rounded sources in
`legacy-examples/` are deliberately excluded unless passed to a command by
their explicit path.

The compare command renders the same canonical semantic JSON through Three.js
and the shared native startup-prepared renderer. It writes corresponding raw
panels, an unscaled labeled sheet, the shared review contract, and diagnostic
receipts under `/tmp/mclone-figure-compare/` by default. The receipts record
source/compiler identity, framing, topology, atlas, preparation, and immutable
upload counts; they are review output, not a persisted asset format.

## Source and generated JSON

Asset files use the DSL from `src/dsl.ts`. A `figure.ts` file is the only
human- or AI-authored source for a promoted figure. Its schema-v1 JSON is a
generated semantic snapshot and must not be edited directly.

Canonical sources use `figure()` and boxes exclusively. `legacyFigure()` plus
its sphere, capsule, and cylinder helpers exist only so retained rounded A/B
sources and schema-v1 compatibility fixtures remain executable. The
first-party drift gate rejects any promoted source containing a non-box part.

Every Asset Lab display path crosses that snapshot boundary. When previewing a
`figure.ts`, the tool executes the DSL, serializes canonical JSON, reparses and
validates it, and gives only that parsed result to Three.js. Preview, sheet,
smoke, and video commands also accept a `figure.json` path directly. The viewer
never renders the live module object through a shortcut.

The three checked runtime figures are mapped to their sources by
`src/first-party-figures.ts`. Regenerate them and review the diff with:

```sh
pnpm asset-lab:figures:write
pnpm asset-lab:figures:check
```

The check fails for stale or missing output and for any promoted
`assets/mclone/figures/*.figure.json` without a declared TypeScript source.
After a checked figure changes, refresh and verify the normal asset-pack lock.
`asset-lab:test` also discovers and executes every Asset Lab example through
the serialize/reparse boundary, including examples that are not promoted into
checked runtime JSON.

Three.js remains the semantic preview implementation, not the source format.

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
