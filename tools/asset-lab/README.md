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
three-quarter, and fixed-frame animation-strip captures.

Asset files should use the DSL from `src/dsl.ts`. Three.js is an implementation
detail of the preview, not the source format.
