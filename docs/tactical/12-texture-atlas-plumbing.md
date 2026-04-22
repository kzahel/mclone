# Tactical 12 — Texture atlas plumbing

Port the CPU-side texture ingestion stack that sits under Minecraft's block renderer: `NativeImage`, `MipmapGenerator`, `Stitcher`, `TextureAtlasSprite`, `TextureAtlas`, and the browser-side glue that loads extracted PNGs into a WebGPU atlas. This slice replaces the placeholder white texture from tactical 11 with a real stitched block atlas.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/com/mojang/blaze3d/platform/NativeImage.java` | `src/renderer/texture/native-image.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/MipmapGenerator.java` | `src/renderer/texture/mipmap-generator.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/Stitcher.java` | `src/renderer/texture/stitcher.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/StitcherException.java` | `src/renderer/texture/stitcher-exception.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/TextureAtlasSprite.java` | `src/renderer/texture/texture-atlas-sprite.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/MissingTextureAtlasSprite.java` | `src/renderer/texture/missing-texture-atlas-sprite.ts` |
| `reference/.../src/net/minecraft/client/renderer/texture/TextureAtlas.java` | `src/renderer/texture/texture-atlas.ts` |
| `reference/.../src/net/minecraft/client/resources/metadata/animation/AnimationMetadataSection.java` | `src/renderer/texture/animation-metadata-section.ts` |
| `reference/.../src/net/minecraft/client/resources/metadata/animation/AnimationFrame.java` | `src/renderer/texture/animation-frame.ts` |
| `reference/.../src/net/minecraft/resources/ResourceLocation.java` | `src/core/resource-location.ts` |
| `reference/.../extracted/assets/minecraft/textures/**/*.png` | browser atlas load path via `src/renderer/texture/browser-native-image-loader.ts` |

## What to port and what to skip

### Direct translation (no divergence)

- `NativeImage` RGBA pixel packing and helpers: `getPixelRGBA`, `setPixelRGBA`, `blendPixel`, `copyFrom`, `fillRect`, `copyRect`, `makePixelArray`, and the ABGR-int helper methods.
- `MipmapGenerator.generateMipLevels(...)`, including the gamma-correct blending path and alpha-cutout handling.
- `Stitcher`, `Holder`, `Region`, and `StitcherException`, including the power-of-two growth logic and the holder sort order.
- `TextureAtlasSprite` UV math (`u0/u1/v0/v1`), mip generation, animated-frame bookkeeping, `uploadFirstFrame`, `uvShrinkRatio`, and `isTransparent`.
- `MissingTextureAtlasSprite`'s 16×16 magenta/black checkerboard.
- `TextureAtlas.prepareToStitch(...)`, missing-sprite registration, mip-level reduction, sprite lookup fallback, and `Preparations`.
- `AnimationMetadataSection` / `AnimationFrame` and the minimal `ResourceLocation` leaf dependency that the texture classes already use in Java.

### WebGPU divergences (callout required)

- PNG decoding uses browser image APIs (`fetch` + `createImageBitmap` + canvas readback) instead of STB/LWJGL.
- `TextureAtlas.reload(...)` creates a `GPUTexture` and uploads sprite mip levels into it instead of allocating and binding a GL texture object.
- `NativeImage.upload(...)` writes into an explicit `GPUTexture` via `GPUQueue.writeTexture(...)` instead of the currently bound GL texture.

## Oracle / done-when

**Unit (Vitest, no browser):**

- `NativeImage` preserves Minecraft's RGBA byte packing (`combine/getR/getG/getB/getA` round-trip).
- `MipmapGenerator` produces the expected mip texel for a hand-built 2×2 source image.
- `Stitcher` packs two equal 16×16 sprites into a 32×16 atlas row.
- `TextureAtlas.prepareToStitch(...)` returns non-trivial UVs for a stitched sprite once the missing texture is added.

**Smoke (Playwright / WebGPU):**

- `src/renderer/main.ts` builds a real `TextureAtlas` from extracted MC block textures, binds the atlas view as `Sampler0`, and renders textured quads through the tactical-11 pipeline layer.
- The browser smoke still passes.
- A screenshot is captured and visually inspected before moving on.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `pnpm test:browser` passes
- The screenshot shows textured quads sampled from the stitched atlas, not a placeholder solid-color texture
- `docs/tactical/README.md` links this slice and marks tactical 11 as done

## Next

Tactical 13: block/state/property groundwork for model baking. `ResourceLocation` already landed here as a leaf dependency, so 13 now starts at `Block`, `BlockState`, `Property`, `BlockStateDefinition`, and a minimal `BlockGetter`.
