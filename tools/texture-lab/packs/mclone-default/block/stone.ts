import type { TextureLabApi, TextureLayerSpec } from "../../../src/dsl";

export function defineStoneTextures(api: TextureLabApi): void {
  const { palette, texture, block, mask } = api;

  // Value structure follows vanilla 1.17.1 stone (studied locally, not copied):
  // four near-neutral grays in a deliberately narrow luminance band, a dominant
  // mid, a second mid only a hair darker, sparse light, sparse dark pits. The
  // faint cool-green tint is our own; the arrangement below is original.
  palette("stone", {
    pit: "#646967",
    mid: "#717672",
    base: "#7c817c",
    light: "#8d928b",
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
    catalog: {
      tiling: "xy",
      rotation: "y180-safe",
    },
    layers: stoneBaseLayers(api),
  });

  texture("coal_ore", {
    size: 32,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/coal_ore.png",
    catalog: {
      tiling: "xy",
      rotation: "fixed",
    },
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
    catalog: {
      tiling: "xy",
      rotation: "fixed",
    },
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

function stoneBaseLayers({ macroNoise, mask, speckles }: TextureLabApi): TextureLayerSpec[] {
  return [
    // The texture's whole identity: an original 16x16 arrangement that obeys
    // vanilla stone's value rules. Mostly the dominant `base` mid, a second mid
    // (`mid`) only ~11 luminance darker, short mostly-horizontal `light` runs
    // (the faint "brick-like" highlight banding), and sparse single-cell `pit`
    // darks. No large pits, no high-contrast plates. Top/bottom rows match and
    // all edge cells are mid-tone, so the nearest-upscaled tile wraps cleanly.
    mask({
      colors: {
        p: "pit",
        m: "mid",
        b: "base",
        h: "light",
      },
      opacity: 1,
      upscale: "nearest",
      authoring: {
        role: "structure",
        label: "AUTHOR STRUCTURE MASK",
      },
      pixels: [
        "bmbbhhbmbbmbbhbm",
        "mbpbbmbbmbpbbmbb",
        "bbmbhhhbbmbbhhbm",
        "bhhbbbmbhhhbbmbb",
        "mbbmbbbmbbmbbbmb",
        "bbbbmpbbmbbhhbbm",
        "bmmbbbmbpbmbbmmb",
        "bbbmbhbmbbbmpbbb",
        "bhhhbbmbhhbbmbhb",
        "mbbmbbbmbbmbbmbb",
        "bpbbmmbbbmbbhhbm",
        "mbbmbbpbmmbbbmbb",
        "bmhhhbbmbbhhbbmb",
        "bbbmbbmbpbbmbbbm",
        "mbbmbhbbmbbmpbbb",
        "bmbbhhbmbbmbbhbm",
      ],
    }),
    // Barely-there low-frequency drift so the tiled field is not a crisp 2x
    // grid of the mask. Kept very low so total contrast stays vanilla-narrow.
    macroNoise({
      seed: "stone-broad-drift",
      frequency: 2,
      octaves: 2,
      colors: ["mid", "base", "light"],
      opacity: 0.07,
      contrast: 0.5,
    }),
    // A whisper of grain only — never the main form.
    speckles({
      seed: "stone-grain-dark",
      density: 0.04,
      colors: ["pit", "mid"],
      opacity: 0.12,
    }),
    speckles({
      seed: "stone-grain-light",
      density: 0.03,
      colors: ["light"],
      opacity: 0.1,
    }),
  ];
}
