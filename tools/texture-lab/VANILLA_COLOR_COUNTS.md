# Vanilla Texture Color Counts

This note records a local measurement of Minecraft Java 1.17.1 block texture
color counts. It is research context for mclone texture authors, not source
art. Do not copy, trace, recolor, upscale, or mechanically transform Mojang
textures into checked-in mclone assets.

## Measurement

Measured source:

```text
reference/minecraft-1.17.1/extracted/assets/minecraft/textures/block/*.png
```

Method:

- decode each local PNG as RGBA
- count exact unique non-transparent RGBA values
- focus on 16x16 block textures, since that is the vanilla baseline for many
  common terrain blocks

Exact color counts are not perceptual palette counts. They are still useful for
understanding the typical size of vanilla texture palettes.

## Related Tools

For a specific authored texture, use the texture analyzer instead of updating
this note by hand. It renders the mclone pack, loads the local vanilla
counterpart when available, downscales both to a shared comparison grid, and
reports color, luminance, blob, seam, and structure features:

```sh
pnpm texture-lab:analyze --texture stone
pnpm texture-lab:analyze --texture stone --json
```

The analyzer is implemented in:

```text
tools/texture-lab/src/analyze.ts
tools/texture-lab/src/analysis.ts
```

Use compare mode when judging two generated PNG candidates against each other:

```sh
pnpm texture-lab:analyze --compare /tmp/a.png /tmp/b.png --reference-name stone
```

For atlas-wide authoring constraints, use the texture catalog. It answers
questions about vanilla texture usage, alpha/cutout behavior, tint indexes,
render layers, model families, tiling, rotation, and authored overlay coverage:

```sh
pnpm texture-lab:catalog
```

The catalog writes:

```text
generated-assets/texture-lab/mclone-default-texture-catalog.md
generated-assets/texture-lab/mclone-default-texture-catalog.json
```

The catalog does not currently emit this document's color-count distribution.
If this table needs regular refreshes, add a generated vanilla-summary mode to
`texture-lab:analyze` or a small companion script that reuses
`tools/texture-lab/src/png.ts`.

## Common Textures

| Texture | Size | Exact non-transparent colors |
|---|---:|---:|
| `stone` | 16x16 | 4 |
| `dirt` | 16x16 | 7 |
| `cobblestone` | 16x16 | 6 |
| `oak_planks` | 16x16 | 7 |
| `oak_log` | 16x16 | 6 |
| `sand` | 16x16 | 6 |
| `gravel` | 16x16 | 8 |
| `granite` | 16x16 | 10 |
| `andesite` | 16x16 | 6 |
| `diorite` | 16x16 | 6 |
| `deepslate` | 16x16 | 5 |
| `tuff` | 16x16 | 5 |
| `netherrack` | 16x16 | 7 |
| `stone_bricks` | 16x16 | 7 |
| `coal_ore` | 16x16 | 10 |
| `iron_ore` | 16x16 | 9 |
| `copper_ore` | 16x16 | 14 |
| `diamond_ore` | 16x16 | 10 |
| `grass_block_side` | 16x16 | 40 |
| `grass_block_top` | 16x16 | 66 |

Grass texture counts are higher than many rock/dirt textures. Also remember
that grass and foliage rendering involves tint semantics, so raw source PNG
color count is not the whole visual model.

## Distribution

Across opaque 16x16 vanilla block textures in the local 1.17.1 extraction:

| Statistic | Exact non-transparent colors |
|---|---:|
| Count | 443 textures |
| Minimum | 1 |
| 25th percentile | 6 |
| Median | 9 |
| 75th percentile | 15 |
| 90th percentile | 40 |
| Maximum | 219 |

Opaque 16x16 bucket counts:

| Color count bucket | Texture count |
|---|---:|
| 1-4 | 22 |
| 5-8 | 191 |
| 9-16 | 130 |
| 17-32 | 42 |
| 33-64 | 31 |
| 65+ | 27 |

High-count outliers are often noisy materials such as concrete powder, or
debug/special textures. They are not the norm for ordinary rock, dirt, wood,
and sand materials.

## Authoring Takeaways

Small palettes are normal. Vanilla stone using only four exact colors is the
important example: it looks richer than a flat noisy texture because the values
are placed with strong authored structure.

For mclone rock and ground textures:

- Start with a small structural palette, usually 4-10 source roles.
- Spend effort on value placement, connected planes, chunks, and edge pairs.
- Use speckles as final grain, not as the main material identity.
- Judge the 16x16 view, mip strip, cube view, and local vanilla reference panel.
- Use the `AUTHOR STRUCTURE MASK` sheet panel to inspect the low-resolution
  composition directly.

Generated 32x32 mclone PNGs may contain many more exact colors after opacity
blending and procedural noise. That is acceptable, but it is not a substitute
for a clear source mask. The source mask should still be understandable as a
small, intentional composition.
