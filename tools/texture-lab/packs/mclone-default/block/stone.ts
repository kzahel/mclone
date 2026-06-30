import type { TextureLabApi, TextureLayerSpec } from "../../../src/dsl";

export function defineStoneTextures(api: TextureLabApi): void {
  const { palette, texture, block, mask } = api;

  palette("stone", {
    base: "#787d78",
    warm: "#85857c",
    light: "#92978f",
    cool: "#68706e",
    cool_light: "#828a86",
    shadow: "#565d5b",
    dark: "#424847",
    deep: "#353b3a",
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
      frequency: 2,
      octaves: 2,
      colors: ["shadow", "cool", "base", "warm", "light"],
      opacity: 0.12,
      contrast: 0.58,
    }),
    mask({
      colors: {
        d: "deep",
        c: "cool",
        h: "cool_light",
        w: "warm",
        s: "shadow",
        l: "light",
        b: "base",
      },
      opacity: 0.66,
      upscale: "nearest",
      authoring: {
        role: "structure",
        label: "AUTHOR STRUCTURE MASK",
      },
      pixels: [
        "bbccbbbllbbbccbb",
        "bcccccblbbbbbccb",
        "bcccbbbbbssbbbbb",
        "bbccbbbssssbbbbb",
        "bbbblbbbssbbllbb",
        "bbbllllbbbbllbbb",
        "bsbbbllbbbbbccbb",
        "bsbbbbbbbbccccbb",
        "bbbwwbbbbbbcccbb",
        "bbwwwwbbbssbbbcb",
        "bwwllwbbbsssbbbb",
        "bbbwwbbbbsbbbllb",
        "bbbbbccbbbllbbbb",
        "bbcccccbllllbbcb",
        "bbbccbbbbllbbbcb",
        "bbccbbbllbbbccbb",
      ],
    }),
    ascii({
      colors: {
        d: "deep",
        s: "shadow",
        c: "cool",
        l: "light",
        h: "cool_light",
      },
      opacity: 0.46,
      pixels: [
        "................................",
        "....sss...............hh........",
        "...s...................h........",
        "..s.............................",
        ".............ss.................",
        "...............s................",
        ".......................hh.......",
        "........................h.......",
        ".ss.............................",
        "...s............................",
        "..........h.....................",
        "..........hh....................",
        "..................ss............",
        "....................s...........",
        "....dd..........................",
        ".....d................h.........",
        ".....................hh.........",
        "........sss.....................",
        "..........s................d....",
        ".........................dd.....",
        ".hh.............................",
        "..h...............ss............",
        "...................s............",
        "............hh..................",
        ".............h..................",
        ".........................ss.....",
        "...........................s....",
        "....s...........................",
        ".....ss...........hh............",
        "...................h............",
        "............................s...",
        "................................",
      ],
    }),
    speckles({
      seed: "stone-fine-cool-grain",
      density: 0.045,
      colors: ["cool", "shadow"],
      opacity: 0.28,
    }),
    speckles({
      seed: "stone-fine-light-grain",
      density: 0.024,
      colors: ["warm", "light"],
      opacity: 0.18,
    }),
    ascii({
      colors: {
        d: "dark",
        s: "shadow",
        l: "light",
      },
      opacity: 0.28,
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
