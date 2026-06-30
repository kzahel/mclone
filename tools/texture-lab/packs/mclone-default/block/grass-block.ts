import type { TextureLabApi } from "../../../src/dsl";

const empty16 = "................";

export function defineGrassBlockTextures({
  tint,
  palette,
  texture,
  block,
  macroNoise,
  mask,
  speckles,
}: TextureLabApi): void {
  tint("grass", {
    normal: "#79b34e",
    alternates: ["#5fa343", "#98b85e", "#6fa35b"],
  });

  palette("grass_top", {
    base: "#d8dbc5",
    light: "#f1f0cf",
    blade: "#c5cca2",
    shadow: "#969d75",
    dark: "#777f5f",
  });

  palette("grass_side", {
    dirt: "#76533a",
    dirt_warm: "#8c6747",
    dirt_light: "#a37b55",
    dirt_cool: "#604631",
    dirt_dark: "#3b2b20",
    grass: "#cfd8a8",
    grass_shadow: "#98a175",
  });

  palette("grass_overlay", {
    transparent: "#00000000",
    grass: "#f2f5d2cc",
    grass_shadow: "#b9c18a99",
  });

  texture("grass_block_top", {
    size: 32,
    source: "tintable",
    tintRole: "grass",
    palette: "grass_top",
    base: "base",
    exportPath: "assets/mclone/textures/block/grass_block_top.png",
    layers: [
      macroNoise({
        seed: "grass-top-broad-tufts",
        frequency: 4,
        octaves: 2,
        colors: ["shadow", "base", "blade", "light"],
        opacity: 0.34,
        contrast: 1.12,
      }),
      mask({
        colors: {
          b: "blade",
          s: "shadow",
          l: "light",
        },
        opacity: 0.18,
        upscale: "smooth",
        pixels: [
          "....b.......l...",
          "..s....b........",
          ".....l....s.....",
          ".b...........b..",
          "....ss..........",
          "..........l.....",
          "..l....b........",
          ".......s....b...",
          "...b...........l",
          "........ss......",
          ".l...........b..",
          ".....b.....l....",
          ".........s......",
          "..b.........ss..",
          "......l.........",
          "....s......b....",
        ],
      }),
      speckles({
        seed: "grass-top-soft-blades",
        density: 0.26,
        colors: ["blade", "shadow"],
        opacity: 0.48,
      }),
      speckles({
        seed: "grass-top-highlights",
        density: 0.1,
        colors: ["light"],
        opacity: 0.32,
      }),
      speckles({
        seed: "grass-top-dark-tufts",
        density: 0.032,
        colors: ["dark", "shadow"],
        opacity: 0.45,
        radius: 1,
      }),
    ],
  });

  texture("grass_block_side", {
    size: 32,
    source: "final-color",
    palette: "grass_side",
    base: "dirt",
    exportPath: "assets/mclone/textures/block/grass_block_side.png",
    preview: {
      cube: false,
      rotation: false,
      tiling: "x",
    },
    layers: [
      macroNoise({
        seed: "grass-side-dirt-broad-clods",
        frequency: 4,
        octaves: 2,
        colors: ["dirt_dark", "dirt_cool", "dirt", "dirt_warm", "dirt_light"],
        opacity: 0.28,
        contrast: 1.12,
      }),
      speckles({
        seed: "grass-side-dirt-grain",
        density: 0.2,
        colors: ["dirt_cool", "dirt_dark", "dirt_warm"],
        opacity: 0.56,
      }),
      speckles({
        seed: "grass-side-dirt-light",
        density: 0.08,
        colors: ["dirt_light"],
        opacity: 0.42,
      }),
      mask({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        opacity: 0.82,
        upscale: "nearest",
        pixels: [
          "gggggggggggggggg",
          "gggsgggsgggsgggs",
          ".gss..gss..gss..",
          "..s....s....s...",
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
        ],
      }),
    ],
  });

  texture("grass_block_side_overlay", {
    size: 32,
    source: "tintable",
    tintRole: "grass",
    palette: "grass_overlay",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/grass_block_side_overlay.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "x",
    },
    layers: [
      mask({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        upscale: "nearest",
        pixels: [
          "gggggggggggggggg",
          "gggsgggsgggsgggs",
          ".gss..gss..gss..",
          "..s....s....s...",
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
        ],
      }),
    ],
  });

  texture("grass_block_bottom", {
    size: 32,
    source: "final-color",
    palette: "grass_side",
    base: "dirt",
    exportPath: "assets/mclone/textures/block/grass_block_bottom.png",
    layers: [
      macroNoise({
        seed: "grass-bottom-dirt-broad-clods",
        frequency: 4,
        octaves: 2,
        colors: ["dirt_dark", "dirt_cool", "dirt", "dirt_warm", "dirt_light"],
        opacity: 0.32,
        contrast: 1.16,
      }),
      mask({
        colors: {
          c: "dirt_cool",
          w: "dirt_warm",
          s: "dirt_dark",
          l: "dirt_light",
        },
        opacity: 0.2,
        upscale: "smooth",
        pixels: [
          "..c.....w.......",
          ".....s......l...",
          ".w..............",
          ".......cc....s..",
          "...l.......w....",
          "..........s.....",
          "c....w..........",
          "......l.....c...",
          "...s...........w",
          "........cc......",
          ".l.........s....",
          ".....w.........c",
          ".........l......",
          "..c........w....",
          "......s.........",
          "....w......c....",
        ],
      }),
      speckles({
        seed: "grass-bottom-dirt-dark",
        density: 0.22,
        colors: ["dirt_cool", "dirt_dark"],
        opacity: 0.62,
      }),
      speckles({
        seed: "grass-bottom-dirt-warm",
        density: 0.12,
        colors: ["dirt_warm", "dirt_light"],
        opacity: 0.44,
      }),
    ],
  });

  block("grass-block", {
    kind: "cube",
    faces: {
      top: "grass_block_top",
      side: "grass_block_side",
      bottom: "grass_block_bottom",
      overlay: "grass_block_side_overlay",
    },
  });
}
