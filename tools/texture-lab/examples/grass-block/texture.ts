import { texturePack } from "../../src/dsl";

const empty = "................................";

export default texturePack("mclone-grass-block-starter", ({ tint, palette, texture, block, speckles, ascii }) => {
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
      speckles({
        seed: "grass-top-soft-blades",
        density: 0.4,
        colors: ["blade", "shadow"],
        opacity: 0.58,
      }),
      speckles({
        seed: "grass-top-highlights",
        density: 0.18,
        colors: ["light"],
        opacity: 0.38,
      }),
      speckles({
        seed: "grass-top-dark-tufts",
        density: 0.055,
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
      speckles({
        seed: "grass-side-dirt-grain",
        density: 0.32,
        colors: ["dirt_cool", "dirt_dark", "dirt_warm"],
        opacity: 0.64,
      }),
      speckles({
        seed: "grass-side-dirt-light",
        density: 0.13,
        colors: ["dirt_light"],
        opacity: 0.5,
      }),
      ascii({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        opacity: 0.82,
        pixels: [
          "gggggggggggggggggggggggggggggggg",
          "gggggggggggggggggggggggggggggggg",
          "gggsgggggggsgggggggsggggggggsggg",
          "ggssggggggssggggggssggggggssgggg",
          ".gssg...gssg....gssg...gssg.....",
          "..s.....s.......s......s........",
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
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
      ascii({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        pixels: [
          "gggggggggggggggggggggggggggggggg",
          "gggggggggggggggggggggggggggggggg",
          "gggsgggggggsgggggggsggggggggsggg",
          "ggssggggggssggggggssggggggssgggg",
          ".gssg...gssg....gssg...gssg.....",
          "..s.....s.......s......s........",
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
          empty,
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
      speckles({
        seed: "grass-bottom-dirt-dark",
        density: 0.34,
        colors: ["dirt_cool", "dirt_dark"],
        opacity: 0.7,
      }),
      speckles({
        seed: "grass-bottom-dirt-warm",
        density: 0.2,
        colors: ["dirt_warm", "dirt_light"],
        opacity: 0.5,
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
});
