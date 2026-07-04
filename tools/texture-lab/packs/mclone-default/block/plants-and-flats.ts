import type { TextureLabApi } from "../../../src/dsl";

const empty16 = "................";

export function definePlantAndFlatTextures({ palette, texture, block, mask, speckles }: TextureLabApi): void {
  palette("plant_cutout", {
    transparent: "#00000000",
    grass_dark: "#2f5c25",
    grass: "#4f8e2f",
    grass_light: "#78ae4c",
    fern_dark: "#2b5a29",
    fern: "#4d873b",
    fern_light: "#75a55a",
  });

  palette("redstone_dust", {
    transparent: "#00000000",
    dark: "#4b0807cc",
    red: "#8e1713dd",
    light: "#d33023ee",
  });

  texture("grass_cross", {
    size: 32,
    source: "final-color",
    palette: "plant_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/grass.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "plant", "model:cross"],
    },
    layers: [
      mask({
        colors: {
          d: "grass_dark",
          g: "grass",
          l: "grass_light",
        },
        opacity: 1,
        upscale: "smooth",
        authoring: {
          role: "structure",
          label: "CROSS PLANT MASK",
        },
        pixels: [
          empty16,
          ".......l........",
          "......ll........",
          "...g..ll..g.....",
          "..gg..ll..gg....",
          "..ggg.ll.ggg....",
          ".gggggllggggg...",
          ".ggglgllglggg...",
          "ggglggllgglggg..",
          "ggggdgllgdgggg..",
          "dgggdgllgdgggd..",
          "ddggdgllgdggdd..",
          ".ddgdgllgdgdd...",
          "..dddgddgddd....",
          "...dddddddd.....",
          empty16,
        ],
      }),
      speckles({
        seed: "grass-cross-light-flecks",
        density: 0.035,
        colors: ["grass_light"],
        opacity: 0.45,
      }),
    ],
  });

  texture("fern_cross", {
    size: 32,
    source: "final-color",
    palette: "plant_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/fern.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "plant", "model:cross"],
    },
    layers: [
      mask({
        colors: {
          d: "fern_dark",
          f: "fern",
          l: "fern_light",
          s: "fern_dark",
        },
        opacity: 1,
        upscale: "smooth",
        authoring: {
          role: "structure",
          label: "FERN CROSS MASK",
        },
        pixels: [
          empty16,
          ".......l........",
          "......ll........",
          ".....lfl........",
          "....lfffl.......",
          "...lfffffl......",
          "..lflfffll......",
          ".lffflfffl......",
          "lfllflfffl......",
          ".ffflfllffl.....",
          "..ffflfffll.....",
          "...fflfffl......",
          "....fflff.......",
          ".....ffdf.......",
          "......dd........",
          empty16,
        ],
      }),
    ],
  });

  texture("redstone_dust_dot", {
    size: 32,
    source: "final-color",
    palette: "redstone_dust",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/redstone_dust_dot.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "ground", "model:flat"],
    },
    layers: [
      mask({
        colors: {
          d: "dark",
          r: "red",
          l: "light",
        },
        opacity: 1,
        upscale: "smooth",
        authoring: {
          role: "structure",
          label: "FLAT GROUND SPRITE",
        },
        pixels: [
          empty16,
          empty16,
          "......rrrr......",
          "....rrrrrrrr....",
          "...rrrllllrrr...",
          "..rrrllllllrrr..",
          "..rrllllllllrr..",
          "..rrllllllllrr..",
          "..rrllllllllrr..",
          "..rrrllllllrrr..",
          "...rrrllllrrr...",
          "....rrrrrrrr....",
          "......rrrr......",
          ".......dd.......",
          empty16,
          empty16,
        ],
      }),
    ],
  });

  block("grass", {
    kind: "cross",
    faces: {
      all: "grass_cross",
    },
  });

  block("fern", {
    kind: "cross",
    faces: {
      all: "fern_cross",
    },
  });

  block("redstone-dust-dot", {
    kind: "flat",
    faces: {
      top: "redstone_dust_dot",
    },
  });
}
