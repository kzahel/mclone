import type { TextureLabApi, TextureLayerSpec } from "../../../src/dsl";

export function defineStoneTextures(api: TextureLabApi): void {
  const { palette, texture, block, mask } = api;

  palette("stone", {
    base: "#777b76",
    warm: "#858276",
    light: "#969a91",
    cool: "#666d6b",
    shadow: "#555c5a",
    dark: "#3f4645",
    coal: "#303231",
    coal_light: "#4a4b45",
    coal_dark: "#181a19",
    iron: "#b66d49",
    iron_light: "#d09365",
    iron_shadow: "#7b4e39",
  });

  texture("stone", {
    size: 32,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/stone.png",
    layers: stoneBaseLayers(api),
  });

  texture("coal_ore", {
    size: 32,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/coal_ore.png",
    layers: [
      ...stoneBaseLayers(api),
      mask({
        colors: {
          c: "coal",
          l: "coal_light",
          d: "coal_dark",
        },
        opacity: 0.92,
        upscale: "nearest",
        pixels: [
          "................",
          "................",
          "....c...........",
          "...cdc..........",
          "....d...........",
          "..........cl....",
          ".........cdc....",
          "................",
          "..cl............",
          "..dc............",
          "...........c....",
          "..........cd....",
          ".....cc.........",
          "......d.........",
          "................",
          "................",
        ],
      }),
    ],
  });

  texture("iron_ore", {
    size: 32,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/iron_ore.png",
    layers: [
      ...stoneBaseLayers(api),
      mask({
        colors: {
          i: "iron",
          l: "iron_light",
          s: "iron_shadow",
        },
        opacity: 0.9,
        upscale: "nearest",
        pixels: [
          "................",
          "...i............",
          "..sli...........",
          "...i............",
          ".........il.....",
          "........isli....",
          ".........si.....",
          "................",
          ".il.............",
          ".sli............",
          "..s........i....",
          "..........ili...",
          "...........s....",
          ".....il.........",
          ".....s..........",
          "................",
        ],
      }),
    ],
  });

  block("stone", {
    kind: "cube",
    faces: {
      all: "stone",
    },
  });

  block("coal-ore", {
    kind: "cube",
    faces: {
      all: "coal_ore",
    },
  });

  block("iron-ore", {
    kind: "cube",
    faces: {
      all: "iron_ore",
    },
  });
}

function stoneBaseLayers({ macroNoise, mask, speckles, ascii }: TextureLabApi): TextureLayerSpec[] {
  return [
    macroNoise({
      seed: "stone-broad-planes",
      frequency: 3,
      octaves: 3,
      colors: ["shadow", "cool", "base", "warm", "light"],
      opacity: 0.28,
      contrast: 0.95,
    }),
    mask({
      colors: {
        c: "cool",
        w: "warm",
        s: "shadow",
        l: "light",
      },
      opacity: 0.14,
      upscale: "smooth",
      pixels: [
        "..c.........w...",
        "......s.........",
        ".w.........l....",
        "........cc......",
        "...l...........s",
        "..........w.....",
        "c...............",
        ".....l.....c....",
        ".........s......",
        "..w.........l...",
        "......cc........",
        "...........w....",
        ".s..............",
        ".....l......c...",
        ".........w......",
        "...c.........s..",
      ],
    }),
    speckles({
      seed: "stone-fine-cool-grain",
      density: 0.18,
      colors: ["cool", "shadow", "dark"],
      opacity: 0.52,
    }),
    speckles({
      seed: "stone-fine-light-grain",
      density: 0.09,
      colors: ["warm", "light"],
      opacity: 0.34,
    }),
    ascii({
      colors: {
        d: "dark",
        s: "shadow",
        l: "light",
      },
      opacity: 0.42,
      pixels: [
        "................................",
        "................................",
        ".......s........................",
        "........s.......................",
        "........................l.......",
        ".......................l........",
        "................................",
        "....d...........................",
        ".....d..........................",
        "................................",
        ".................s..............",
        "..................s.............",
        "................................",
        "............................d...",
        ".............................d..",
        "................................",
        "............l...................",
        ".............l..................",
        "................................",
        ".....................s..........",
        "......................s.........",
        "................................",
        "..d.............................",
        "...d............................",
        "................................",
        "........................l.......",
        ".........................l......",
        "................................",
        "..........s.....................",
        "...........s....................",
        "................................",
        "................................",
      ],
    }),
  ];
}
