# Deno1: Render Pipeline Smoke

Status: done.

## Goal

Extend the Chrome-free Deno WebGPU lane from a raw clear/readback smoke to a real renderer-pipeline smoke. The test must compile a repo shader through `RenderPipelineCache`, bind uniforms through `ShaderProgramDefinition`, draw known geometry into an offscreen target, read pixels back, and write a PNG under `/tmp`.

This is still a headless renderer validation slice. It intentionally avoids DOM, browser canvas, asset loading, chunk/world rendering, Rust/wgpu, and OpenXR.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `scripts/deno-render-pipeline-smoke.ts` | done - Deno script that draws a `POSITION_COLOR` triangle through `RenderPipelineCache` and writes `/tmp/mclone-deno-render-pipeline-smoke.png` |
| 2 | `package.json` | done - pinned `pnpm smoke:deno:pipeline` command |
| 3 | `scripts/png-rgba.ts` | done - shared PNG encoder used by both Deno smoke scripts |
| 4 | `docs/deno-wgpu-native-spike.md` / tactical README | done - document the new pipeline smoke lane and next target |

Do not add:

- DOM or canvas emulation
- browser asset decode
- texture atlas loading
- chunk/world rendering
- Playwright/Chrome dependencies
- Rust/wgpu or OpenXR host code

## Command

```bash
pnpm smoke:deno:pipeline
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-render-pipeline-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-render-pipeline-smoke.png
```

Expected center pixel:

```text
[51, 204, 77, 255]
```

## Implementation Notes

The smoke creates a custom render type:

- format: `DefaultVertexFormat.POSITION_COLOR`
- mode: `VertexFormat.Mode.TRIANGLES`
- shader: `RenderStateShards.POSITION_COLOR_SHADER`
- culling: disabled
- write mask: color only

The command then uses `RenderPipelineCache.getOrCreate(...)`, `ShaderProgramDefinition.createUniformBufferBytes(...)`, and `ShaderProgramDefinition.createBindGroup(...)`. This validates the repo shader JSON import path, WGSL source generation, vertex layout lowering, pipeline creation, uniform binding, draw submission, readback, and PNG artifact generation in Deno.

## Validation

Completed:

- `pnpm smoke:deno:webgpu` - passed after granting network access for the pinned Deno binary fetch through `npx`
- `pnpm smoke:deno:pipeline` - passed after granting network access for the pinned Deno binary fetch through `npx`
- inspected `/tmp/mclone-deno-render-pipeline-smoke.png`: black background with a green triangle; center pixel matched `[51, 204, 77, 255]`
- `pnpm typecheck`
- `pnpm test:browser` - required because this work touches package scripts/docs around the renderer lane and keeps Deno0 healthy alongside browser validation
- `git diff --check`

## Done When

- Done: a repo-owned command draws known geometry through a real renderer pipeline without Chrome.
- Done: the smoke validates a known readback pixel and fails on mismatch.
- Done: the PNG artifact is written to `/tmp` and visually inspectable.
- Done: Deno0 still passes after sharing the PNG encoder.

## Follow-Up

Next likely tactical: `Deno2` should introduce a non-browser asset/image decode seam so the Deno lane can load a tiny texture or atlas input without browser `createImageBitmap`, `HTMLCanvasElement`, or `OffscreenCanvas`.
