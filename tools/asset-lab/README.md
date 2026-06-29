# Mclone Asset Lab

Disposable TypeScript/Three.js lab for AI-authored primitive figures.

Install once:

```sh
pnpm --dir tools/asset-lab install
```

Common commands from the repo root:

```sh
pnpm asset-lab:typecheck
pnpm asset-lab:export
pnpm asset-lab:smoke
pnpm asset-lab:sheet
pnpm asset-lab:video
pnpm asset-lab:preview
```

The smoke command writes screenshots under `/tmp/mclone-asset-lab/` by default.
The exported figure JSON is also written under `/tmp` unless `--out` is passed.
The sheet command writes a larger review image with front, side,
three-quarter, side animation, and three-quarter animation captures.
The video command captures deterministic Playwright frames and uses `ffmpeg` to
write an MP4 animation review at `/tmp/mclone-asset-lab/piglet-walk.mp4`.

Asset files should use the DSL from `src/dsl.ts`. Three.js is an implementation
detail of the preview, not the source format.

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
from the part's unrotated local center, so a vertical capsule leg with
`length: 0.42` uses `pivot: [0, 0.21, 0]` to swing from its top.

```ts
part("leg_fl", capsule({
  parent: "body",
  at: [-0.42, -0.53, -0.26],
  radius: 0.09,
  length: 0.42,
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
not runtime procedural animation.

```ts
quadrupedWalk("walk", {
  legs: {
    frontLeft: "leg_fl",
    frontRight: "leg_fr",
    backLeft: "leg_bl",
    backRight: "leg_br",
  },
  body: "body",
  gait: "trot",
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

The video command accepts the same figure path plus timing options:

```sh
pnpm --dir tools/asset-lab exec tsx src/video.ts examples/piglet/figure.ts \
  --out /tmp/mclone-asset-lab/piglet-walk.mp4 \
  --clip walk \
  --fps 24 \
  --seconds 2
```
