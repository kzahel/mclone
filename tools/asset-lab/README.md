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
pnpm asset-lab:preview
```

The smoke command writes screenshots under `/tmp/mclone-asset-lab/` by default.
The exported figure JSON is also written under `/tmp` unless `--out` is passed.
The sheet command writes a larger review image with front, side,
three-quarter, side animation, and three-quarter animation captures.

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
