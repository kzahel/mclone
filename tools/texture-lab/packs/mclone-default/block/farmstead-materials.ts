import type { TextureLabApi, TextureLayerSpec } from "../../../src/dsl";

const empty16 = "................";

export function defineFarmsteadMaterialTextures(api: TextureLabApi): void {
  definePlaster(api);
  defineCobblestone(api, "cobblestone", false);
  defineCobblestone(api, "mossy_cobblestone", true);
  defineLog(api, "oak_log", "#725036", "#9a7350", "#4b3427", "#36251e");
  defineLog(api, "spruce_log", "#503622", "#765439", "#35241a", "#271a14");
  definePlanks(api, "oak_planks", "#a97942", "#6a4327", "#d1a468");
  definePlanks(api, "spruce_planks", "#684428", "#3e281b", "#95663d");
  defineBricks(api);
  defineStoneBricks(api);
  defineRedTerracotta(api);
  defineHay(api);
  defineFlower(api, "poppy", "#b92f2b", "#ee5a40", "#f4b13d");
  defineFlower(api, "cornflower", "#4168bd", "#7199ed", "#d7d8a9");
  defineWallTorch(api);
}

function definePlaster(api: TextureLabApi): void {
  api.palette("farmstead_plaster", {
    base: "#c8b9a3",
    light: "#ded3c1",
    shadow: "#a99984",
    grain: "#887963",
  });
  api.texture("farmstead_white_terracotta", {
    size: 32,
    source: "final-color",
    palette: "farmstead_plaster",
    base: "base",
    exportPath: "assets/mclone/textures/block/white_terracotta.png",
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog("plaster", "y180-safe"),
    layers: [
      api.macroNoise({
        seed: "farmstead-plaster-clouds",
        frequency: 5,
        octaves: 3,
        colors: ["shadow", "base", "light"],
        opacity: 0.16,
        contrast: 0.9,
      }),
      api.speckles({
        seed: "farmstead-plaster-grain",
        density: 0.055,
        colors: ["light", "grain"],
        opacity: 0.18,
      }),
    ],
  });
  api.block("farmstead-white-terracotta", cube("farmstead_white_terracotta"));
}

function defineCobblestone(api: TextureLabApi, name: string, mossy: boolean): void {
  const paletteName = `farmstead_${name}`;
  const textureName = `farmstead_${name}`;
  api.palette(paletteName, {
    base: mossy ? "#727369" : "#777873",
    light: mossy ? "#999985" : "#9b9c94",
    shadow: mossy ? "#555b50" : "#5a5c58",
    joint: "#424640",
    moss: "#657548",
    moss_light: "#87945d",
  });

  const layers: TextureLayerSpec[] = [
    api.macroNoise({
      seed: `farmstead-${name}-stone`,
      frequency: 7,
      octaves: 2,
      colors: ["shadow", "base", "light"],
      opacity: 0.24,
      contrast: 1.15,
    }),
    api.mask({
      colors: { j: "joint", l: "light", s: "shadow" },
      skip: ".",
      upscale: "nearest",
      opacity: 0.78,
      authoring: { role: "structure", label: "IRREGULAR STONE JOINTS" },
      pixels: [
        "jjjjjjjjjjjjjjjj",
        "j.....j........j",
        "j.l...j..l.....j",
        "j.....j.......sj",
        "jjjjjjjjjjjjjjjj",
        "j........j.....j",
        "j...l....j.l...j",
        "j........j.....j",
        "jjjjjjjjjjjjjjjj",
        "j....j.........j",
        "j.l..j....l....j",
        "j....j.........j",
        "jjjjjjjjjjjjjjjj",
        "j.......j......j",
        "j..s....j...l..j",
        "j.......j......j",
      ],
    }),
  ];
  if (mossy) {
    layers.push(
      api.mask({
        colors: { m: "moss", M: "moss_light" },
        skip: ".",
        upscale: "smooth",
        opacity: 0.78,
        authoring: { role: "structure", label: "MOSS IN STONE JOINTS" },
        pixels: [
          "mmmmm...........",
          "m...............",
          "m...............",
          "mm..............",
          "mmmmmmm.........",
          "........m.......",
          "........m.......",
          "........m.......",
          "....mmmmmmmmm...",
          "....m...........",
          "....m...........",
          "mmmmmm........M.",
          "mmmmmmmmmmm.....",
          "........m.......",
          "........m.......",
          "........m.......",
        ],
      }),
    );
  }

  api.texture(textureName, {
    size: 32,
    source: "final-color",
    palette: paletteName,
    base: "base",
    exportPath: `assets/mclone/textures/block/${name}.png`,
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog(mossy ? "mossy-stone" : "stone", "y90-safe"),
    layers,
  });
  api.block(`farmstead-${name.replaceAll("_", "-")}`, cube(textureName));
}

function defineLog(
  api: TextureLabApi,
  name: "oak_log" | "spruce_log",
  base: string,
  light: string,
  shadow: string,
  dark: string,
): void {
  const textureName = `farmstead_${name}`;
  const paletteName = `${textureName}_bark`;
  api.palette(paletteName, {
    base,
    light,
    shadow,
    dark,
  });
  api.texture(textureName, {
    size: 32,
    source: "final-color",
    palette: paletteName,
    base: "base",
    exportPath: `assets/mclone/textures/block/${name}.png`,
    preview: { cube: true, tiling: "x" },
    catalog: farmsteadCatalog("timber", "y180-safe", "x"),
    layers: [
      api.macroNoise({
        seed: `farmstead-${name}-bark`,
        frequency: 5,
        octaves: 2,
        colors: ["shadow", "base", "light"],
        opacity: 0.25,
        contrast: 1.15,
      }),
      api.mask({
        colors: { d: "dark", s: "shadow", l: "light" },
        skip: ".",
        upscale: "smooth",
        opacity: 0.62,
        authoring: { role: "structure", label: "VERTICAL BARK FISSURES" },
        pixels: [
          ".d....s....d....",
          ".d....s....d..l.",
          "..d...s...d...l.",
          "..d...s...d.....",
          "..d..s....d.....",
          "..d..s.....d....",
          "...d.s.....d....",
          "...d..s....d....",
          "...d..s...d.....",
          "..d...s...d.....",
          "..d....s..d.....",
          ".d.....s...d..l.",
          ".d.....s....d.l.",
          ".d....s.....d...",
          "..d...s.....d...",
          "..d...s....d....",
        ],
      }),
    ],
  });
  api.block(`farmstead-${name.replaceAll("_", "-")}`, cube(textureName));
}

function definePlanks(
  api: TextureLabApi,
  name: "oak_planks" | "spruce_planks",
  base: string,
  dark: string,
  light: string,
): void {
  const textureName = `farmstead_${name}`;
  api.palette(textureName, { base, dark, light, seam: dark });
  api.texture(textureName, {
    size: 32,
    source: "final-color",
    palette: textureName,
    base: "base",
    exportPath: `assets/mclone/textures/block/${name}.png`,
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog("wood-planks", "y180-safe"),
    layers: [
      api.macroNoise({
        seed: `farmstead-${name}-grain`,
        frequency: 8,
        octaves: 2,
        colors: ["dark", "base", "light"],
        opacity: 0.16,
        contrast: 1.12,
      }),
      api.mask({
        colors: { s: "seam", l: "light", d: "dark" },
        skip: ".",
        upscale: "nearest",
        opacity: 0.72,
        authoring: { role: "structure", label: "PLANK SEAMS AND KNOTS" },
        pixels: [
          "ssssssssssssssss",
          "................",
          "....l...........",
          ".........dd.....",
          "ssssssssssssssss",
          "................",
          ".dd.............",
          ".d...........l..",
          "ssssssssssssssss",
          "................",
          ".......l........",
          "............dd..",
          "ssssssssssssssss",
          "................",
          "..l.............",
          ".........d......",
        ],
      }),
    ],
  });
  api.block(`farmstead-${name.replaceAll("_", "-")}`, cube(textureName));
}

function defineBricks(api: TextureLabApi): void {
  api.palette("farmstead_bricks", {
    base: "#9e513e",
    light: "#c17659",
    dark: "#71372f",
    mortar: "#c4b79f",
  });
  api.texture("farmstead_bricks", {
    size: 32,
    source: "final-color",
    palette: "farmstead_bricks",
    base: "base",
    exportPath: "assets/mclone/textures/block/bricks.png",
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog("brick", "y180-safe"),
    layers: [
      api.macroNoise({
        seed: "farmstead-brick-clay",
        frequency: 7,
        octaves: 2,
        colors: ["dark", "base", "light"],
        opacity: 0.18,
      }),
      api.mask({
        colors: { m: "mortar", d: "dark", l: "light" },
        skip: ".",
        upscale: "nearest",
        opacity: 0.86,
        authoring: { role: "structure", label: "STAGGERED BRICK BONDS" },
        pixels: [
          "mmmmmmmmmmmmmmmm",
          ".......m........",
          "..l....m...d....",
          ".......m........",
          "mmmmmmmmmmmmmmmm",
          "...m.......m....",
          "...m..d....m.l..",
          "...m.......m....",
          "mmmmmmmmmmmmmmmm",
          ".......m........",
          ".d.....m....l...",
          ".......m........",
          "mmmmmmmmmmmmmmmm",
          "...m.......m....",
          "...m...l...m..d.",
          "...m.......m....",
        ],
      }),
    ],
  });
  api.block("farmstead-bricks", cube("farmstead_bricks"));
}

function defineStoneBricks(api: TextureLabApi): void {
  api.palette("farmstead_stone_bricks", {
    base: "#7d8178",
    light: "#a5a99d",
    dark: "#555b54",
    mortar: "#444b46",
  });
  api.texture("farmstead_stone_bricks", {
    size: 32,
    source: "final-color",
    palette: "farmstead_stone_bricks",
    base: "base",
    exportPath: "assets/mclone/textures/block/stone_bricks.png",
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog("dressed-stone", "y180-safe"),
    layers: [
      api.macroNoise({
        seed: "farmstead-stone-brick-grain",
        frequency: 6,
        octaves: 2,
        colors: ["dark", "base", "light"],
        opacity: 0.16,
      }),
      api.mask({
        colors: { m: "mortar", l: "light", d: "dark" },
        skip: ".",
        upscale: "nearest",
        opacity: 0.78,
        authoring: { role: "structure", label: "STAGGERED STONE COURSES" },
        pixels: [
          "mmmmmmmmmmmmmmmm",
          ".......m........",
          "..l....m....d...",
          ".......m........",
          "mmmmmmmmmmmmmmmm",
          "...m.......m....",
          "...m.d.....m..l.",
          "...m.......m....",
          "mmmmmmmmmmmmmmmm",
          ".......m........",
          ".d.....m...l....",
          ".......m........",
          "mmmmmmmmmmmmmmmm",
          "...m.......m....",
          "...m..l....m.d..",
          "...m.......m....",
        ],
      }),
    ],
  });
  api.block("farmstead-stone-bricks", cube("farmstead_stone_bricks"));
}

function defineRedTerracotta(api: TextureLabApi): void {
  api.palette("farmstead_red_terracotta", {
    base: "#974535",
    light: "#b96048",
    shadow: "#713329",
    dark: "#58261f",
  });
  api.texture("farmstead_red_terracotta", {
    size: 32,
    source: "final-color",
    palette: "farmstead_red_terracotta",
    base: "base",
    exportPath: "assets/mclone/textures/block/red_terracotta.png",
    preview: { cube: true, tiling: "xy" },
    catalog: farmsteadCatalog("painted-clay", "y180-safe"),
    layers: [
      api.macroNoise({
        seed: "farmstead-red-terracotta-clouds",
        frequency: 5,
        octaves: 3,
        colors: ["shadow", "base", "light"],
        opacity: 0.2,
        contrast: 0.95,
      }),
      api.speckles({
        seed: "farmstead-red-terracotta-grain",
        density: 0.06,
        colors: ["dark", "light"],
        opacity: 0.18,
      }),
    ],
  });
  api.block("farmstead-red-terracotta", cube("farmstead_red_terracotta"));
}

function defineHay(api: TextureLabApi): void {
  api.palette("farmstead_hay", {
    base: "#c99a35",
    light: "#e2bd59",
    shadow: "#936926",
    tie: "#725025",
  });
  api.texture("farmstead_hay", {
    size: 32,
    source: "final-color",
    palette: "farmstead_hay",
    base: "base",
    exportPath: "assets/mclone/textures/block/hay_block.png",
    preview: { cube: true, tiling: "x" },
    catalog: farmsteadCatalog("hay", "y180-safe", "x"),
    layers: [
      api.macroNoise({
        seed: "farmstead-hay-fibers",
        frequency: 8,
        octaves: 2,
        colors: ["shadow", "base", "light"],
        opacity: 0.2,
      }),
      api.mask({
        colors: { s: "shadow", l: "light", t: "tie" },
        skip: ".",
        upscale: "smooth",
        opacity: 0.7,
        authoring: { role: "structure", label: "BOUND HAY FIBERS" },
        pixels: [
          "..s..l...s..l...",
          "..s...l..s...l..",
          "...s..l...s..l..",
          "...s...l..s...l.",
          "tttttttttttttttt",
          "..s..l...s..l...",
          "..s...l..s...l..",
          "...s..l...s..l..",
          "...s...l..s...l.",
          "..s..l...s..l...",
          "..s...l..s...l..",
          "tttttttttttttttt",
          "...s...l..s...l.",
          "..s..l...s..l...",
          "..s...l..s...l..",
          "...s..l...s..l..",
        ],
      }),
    ],
  });
  api.block("farmstead-hay", cube("farmstead_hay"));
}

function defineFlower(
  api: TextureLabApi,
  name: "poppy" | "cornflower",
  petal: string,
  highlight: string,
  center: string,
): void {
  const paletteName = `farmstead_${name}`;
  api.palette(paletteName, {
    transparent: "#00000000",
    stem: "#477136",
    leaf: "#5d8c42",
    petal,
    highlight,
    center,
  });
  api.texture(`farmstead_${name}`, {
    size: 32,
    source: "final-color",
    palette: paletteName,
    base: "transparent",
    exportPath: `assets/mclone/textures/block/${name}.png`,
    preview: { checkerboard: true, cube: false, tiling: "none" },
    catalog: farmsteadCatalog("flower", "model-driven", "none"),
    layers: [
      api.mask({
        colors: { s: "stem", g: "leaf", p: "petal", h: "highlight", c: "center" },
        skip: ".",
        upscale: "smooth",
        opacity: 1,
        authoring: { role: "structure", label: `${name.toUpperCase()} SILHOUETTE` },
        pixels: [
          empty16,
          ".....pphpp......",
          "....pppcppp.....",
          ".....ppcpp......",
          ".......s........",
          ".......s..g.....",
          ".......s.gg.....",
          "....g..sgg......",
          "....gg.s........",
          ".....ggs........",
          ".......s........",
          ".......s........",
          "......ss........",
          "......ss........",
          "......ss........",
          empty16,
        ],
      }),
    ],
  });
  api.block(name, { kind: "cross", faces: { all: `farmstead_${name}` } });
}

function defineWallTorch(api: TextureLabApi): void {
  api.palette("farmstead_wall_torch", {
    base: "#6b4529",
    wood: "#8d6037",
    shadow: "#432a1d",
    ember: "#c14d21",
    flame: "#f2a62e",
    glow: "#ffe277",
  });
  api.texture("farmstead_wall_torch", {
    size: 32,
    source: "final-color",
    palette: "farmstead_wall_torch",
    base: "base",
    exportPath: "assets/mclone/textures/block/wall_torch.png",
    preview: { cube: false, tiling: "none" },
    catalog: farmsteadCatalog("torch", "model-driven", "none"),
    layers: [
      api.macroNoise({
        seed: "farmstead-wall-torch-wood",
        frequency: 4,
        colors: ["shadow", "base", "wood"],
        opacity: 0.22,
      }),
      api.mask({
        colors: { e: "ember", f: "flame", g: "glow", s: "shadow" },
        skip: ".",
        upscale: "smooth",
        opacity: 0.92,
        authoring: { role: "structure", label: "TORCH FLAME ACCENT" },
        pixels: [
          "......gggg......",
          ".....gffffg.....",
          ".....gffffg.....",
          "......feef......",
          ".......ee.......",
          "................",
          "................",
          "................",
          "................",
          "................",
          "................",
          "................",
          "................",
          "................",
          ".......ss.......",
          "......ssss......",
        ],
      }),
    ],
  });
  api.block("farmstead-wall-torch", cube("farmstead_wall_torch"));
}

function cube(texture: string) {
  return { kind: "cube" as const, faces: { all: texture } };
}

function farmsteadCatalog(
  material: string,
  rotation: "y90-safe" | "y180-safe" | "model-driven",
  tiling: "xy" | "x" | "none" = "xy",
) {
  return {
    status: "draft" as const,
    tiling,
    rotation,
    tags: ["farmstead", material, "structure-lab"],
    notes: ["Original first-party material authored for the farmstead structure family."],
  };
}
