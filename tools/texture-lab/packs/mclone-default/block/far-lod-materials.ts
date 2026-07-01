import type { TextureCatalogRotation, TextureLabApi, TextureLayerSpec, TextureSpec } from "../../../src/dsl";

interface NoisyMaterialOptions {
  lightMix?: number;
  shadowMix?: number;
  darkMix?: number;
  frequency?: number;
  macroOpacity?: number;
  speckleDensity?: number;
  speckleOpacity?: number;
  rotation?: TextureCatalogRotation;
}

interface TopSideMaterialOptions {
  top: string;
  side: string;
  bottom?: string;
  topOptions?: NoisyMaterialOptions;
  sideOptions?: NoisyMaterialOptions;
  bottomOptions?: NoisyMaterialOptions;
}

export function defineFarLodTerrainMaterialTextures(api: TextureLabApi): void {
  for (const [name, base, options] of SIMPLE_TERRAIN_MATERIALS) {
    defineNoisyCube(api, name, base, options);
  }

  defineTopSideCube(api, "podzol", {
    top: "#5a4025",
    side: "#6d4f34",
    bottom: "dirt",
    topOptions: { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.2 },
    sideOptions: { frequency: 4, macroOpacity: 0.24, speckleDensity: 0.18 },
  });
  defineTopSideCube(api, "mycelium", {
    top: "#817887",
    side: "#6b513c",
    bottom: "dirt",
    topOptions: { frequency: 5, macroOpacity: 0.22, speckleDensity: 0.18 },
    sideOptions: { frequency: 4, macroOpacity: 0.24, speckleDensity: 0.18 },
  });

  defineLog(api, "oak_log", "#735936", "#9c7a48");
  defineLog(api, "birch_log", "#d5d0bc", "#c09a5b");
  defineLog(api, "spruce_log", "#4e3824", "#765735");

  defineNoisyCube(api, "oak_leaves", "#3e7f2d", {
    frequency: 6,
    macroOpacity: 0.28,
    speckleDensity: 0.16,
  });
  defineNoisyCube(api, "birch_leaves", "#5f9340", {
    frequency: 6,
    macroOpacity: 0.25,
    speckleDensity: 0.16,
  });
  defineNoisyCube(api, "spruce_leaves", "#244f32", {
    frequency: 7,
    macroOpacity: 0.3,
    speckleDensity: 0.16,
  });

  defineNoisyCube(api, "grass", "#4f8e2f", { macroOpacity: 0.24, speckleDensity: 0.12 });
  defineNoisyCube(api, "fern", "#3f7b31", { macroOpacity: 0.24, speckleDensity: 0.12 });
  defineNoisyCube(api, "large_fern", "#3a7330", {
    macroOpacity: 0.24,
    speckleDensity: 0.12,
  });
  defineNoisyCube(api, "dead_bush", "#8d6a3d", { macroOpacity: 0.18, speckleDensity: 0.1 });
  defineNoisyCube(api, "dandelion", "#d8b629", {
    lightMix: 0.24,
    shadowMix: 0.16,
    macroOpacity: 0.18,
    speckleDensity: 0.1,
  });
  defineNoisyCube(api, "poppy", "#b9342f", {
    lightMix: 0.18,
    shadowMix: 0.2,
    macroOpacity: 0.18,
    speckleDensity: 0.1,
  });
}

const SIMPLE_TERRAIN_MATERIALS: Array<[string, string, NoisyMaterialOptions?]> = [
  ["coarse_dirt", "#74543a", { frequency: 4, speckleDensity: 0.22 }],
  ["sand", "#cfc28a", { frequency: 6, macroOpacity: 0.18, speckleDensity: 0.12 }],
  ["red_sand", "#a9572a", { frequency: 6, macroOpacity: 0.2, speckleDensity: 0.12 }],
  ["sandstone", "#c7b982", { frequency: 4, macroOpacity: 0.18, speckleDensity: 0.1 }],
  ["red_sandstone", "#a75a2f", { frequency: 4, macroOpacity: 0.18, speckleDensity: 0.1 }],
  ["gravel", "#75746f", { frequency: 8, macroOpacity: 0.28, speckleDensity: 0.3 }],
  ["snow", "#e6ebe6", { frequency: 5, macroOpacity: 0.12, speckleDensity: 0.08, shadowMix: 0.08 }],
  ["snow_block", "#e6ebe6", { frequency: 5, macroOpacity: 0.14, speckleDensity: 0.1 }],
  ["ice", "#94c6da", { frequency: 5, macroOpacity: 0.16, speckleDensity: 0.08, lightMix: 0.28 }],
  [
    "packed_ice",
    "#75a9c7",
    { frequency: 5, macroOpacity: 0.2, speckleDensity: 0.12, lightMix: 0.22 },
  ],
  ["clay", "#818990", { frequency: 4, macroOpacity: 0.18, speckleDensity: 0.1 }],
  ["granite", "#956b5d", { frequency: 5, macroOpacity: 0.25, speckleDensity: 0.22 }],
  ["diorite", "#b8b8b2", { frequency: 5, macroOpacity: 0.25, speckleDensity: 0.2, shadowMix: 0.12 }],
  ["andesite", "#858884", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.18 }],
  ["copper_ore", "#897d69", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["gold_ore", "#8e8460", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["redstone_ore", "#766463", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["diamond_ore", "#6f8b8b", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["lapis_ore", "#66708c", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["bedrock", "#565656", { frequency: 7, macroOpacity: 0.32, speckleDensity: 0.28, shadowMix: 0.32 }],
  ["tuff", "#6d7068", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.18 }],
  ["deepslate", "#555a5b", { frequency: 4, macroOpacity: 0.24, speckleDensity: 0.16 }],
  ["deepslate_coal_ore", "#4e5353", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_copper_ore", "#665f56", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_iron_ore", "#675d54", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_gold_ore", "#6b654f", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_redstone_ore", "#5e5052", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_diamond_ore", "#566c70", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["deepslate_lapis_ore", "#535a71", { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.14 }],
  ["dripstone_block", "#8c6b4e", { frequency: 5, macroOpacity: 0.26 }],
  ["pointed_dripstone", "#8b6b52", { frequency: 5, macroOpacity: 0.22, speckleDensity: 0.1 }],
  ["obsidian", "#2a263d", { frequency: 5, macroOpacity: 0.28, speckleDensity: 0.16, lightMix: 0.12 }],
  ["magma_block", "#824126", { frequency: 5, macroOpacity: 0.32, speckleDensity: 0.18 }],
  ["glow_lichen", "#6d8b64", { frequency: 6, macroOpacity: 0.18, speckleDensity: 0.08, lightMix: 0.2 }],
  ["water", "#2b5797", { frequency: 4, macroOpacity: 0.14, speckleDensity: 0.04, lightMix: 0.2, shadowMix: 0.1 }],
  ["lava", "#df5620", { frequency: 4, macroOpacity: 0.24, speckleDensity: 0.1, lightMix: 0.28, shadowMix: 0.16 }],
  ["terracotta", "#8c4d35", { frequency: 4, macroOpacity: 0.18, speckleDensity: 0.08 }],
  ["white_terracotta", "#bca996", { frequency: 4, macroOpacity: 0.16 }],
  ["orange_terracotta", "#a85328", { frequency: 4, macroOpacity: 0.18 }],
  ["magenta_terracotta", "#95526d", { frequency: 4, macroOpacity: 0.18 }],
  ["light_blue_terracotta", "#6d7893", { frequency: 4, macroOpacity: 0.18 }],
  ["yellow_terracotta", "#b58a3b", { frequency: 4, macroOpacity: 0.18 }],
  ["lime_terracotta", "#7d8d43", { frequency: 4, macroOpacity: 0.18 }],
  ["pink_terracotta", "#a25f5c", { frequency: 4, macroOpacity: 0.18 }],
  ["gray_terracotta", "#5b504b", { frequency: 4, macroOpacity: 0.18 }],
  ["light_gray_terracotta", "#867a72", { frequency: 4, macroOpacity: 0.18 }],
  ["cyan_terracotta", "#566363", { frequency: 4, macroOpacity: 0.18 }],
  ["purple_terracotta", "#754b6a", { frequency: 4, macroOpacity: 0.18 }],
  ["blue_terracotta", "#514e70", { frequency: 4, macroOpacity: 0.18 }],
  ["brown_terracotta", "#604026", { frequency: 4, macroOpacity: 0.18 }],
  ["green_terracotta", "#586034", { frequency: 4, macroOpacity: 0.18 }],
  ["red_terracotta", "#8f3e30", { frequency: 4, macroOpacity: 0.18 }],
  ["black_terracotta", "#302826", { frequency: 4, macroOpacity: 0.18 }],
  ["bricks", "#9b4f3e", { frequency: 4, macroOpacity: 0.2, speckleDensity: 0.12 }],
];

function defineTopSideCube(api: TextureLabApi, name: string, options: TopSideMaterialOptions): void {
  const topTexture = `${name}_top_lod`;
  const sideTexture = `${name}_side_lod`;
  defineNoisyTexture(api, topTexture, options.top, options.topOptions);
  defineNoisyTexture(api, sideTexture, options.side, options.sideOptions);

  let bottomTexture = options.bottom;
  if (!bottomTexture) {
    bottomTexture = `${name}_bottom_lod`;
    defineNoisyTexture(api, bottomTexture, options.side, options.bottomOptions ?? options.sideOptions);
  }

  api.block(name.replaceAll("_", "-"), {
    kind: "cube",
    faces: {
      top: topTexture,
      side: sideTexture,
      bottom: bottomTexture,
    },
  });
}

function defineLog(api: TextureLabApi, name: string, side: string, top: string): void {
  const sideTexture = `${name}_side_lod`;
  const topTexture = `${name}_top_lod`;
  defineNoisyTexture(api, sideTexture, side, { frequency: 4, macroOpacity: 0.28, speckleDensity: 0.14 });
  defineNoisyTexture(api, topTexture, top, { frequency: 5, macroOpacity: 0.24, speckleDensity: 0.12 });
  api.block(name.replaceAll("_", "-"), {
    kind: "cube",
    faces: {
      top: topTexture,
      bottom: topTexture,
      side: sideTexture,
    },
  });
}

function defineNoisyCube(api: TextureLabApi, name: string, base: string, options: NoisyMaterialOptions = {}): void {
  defineNoisyTexture(api, name, base, options);
}

function defineNoisyTexture(api: TextureLabApi, name: string, base: string, options: NoisyMaterialOptions = {}): void {
  const paletteName = `lod_${name}`;
  api.palette(paletteName, materialPalette(base, options));
  const layers = noisyLayers(api, name, options);
  const spec: TextureSpec = {
    size: 32,
    source: "final-color",
    palette: paletteName,
    base: "base",
    exportPath: `assets/mclone/lod/textures/block/${name}.png`,
    catalog: {
      tiling: "xy",
      rotation: options.rotation ?? "y180-safe",
      tags: ["far-lod-material"],
    },
    layers,
  };
  api.texture(name, spec);
}

function noisyLayers(api: TextureLabApi, name: string, options: NoisyMaterialOptions): TextureLayerSpec[] {
  const layers: TextureLayerSpec[] = [];
  layers.push(
    api.macroNoise({
      seed: `lod-${name}-macro`,
      frequency: options.frequency ?? 5,
      octaves: 2,
      colors: ["shadow", "base", "light"],
      opacity: options.macroOpacity ?? 0.22,
      contrast: 1.08,
    }),
  );
  layers.push(
    api.speckles({
      seed: `lod-${name}-grain`,
      density: options.speckleDensity ?? 0.14,
      colors: ["dark", "shadow", "light"],
      opacity: options.speckleOpacity ?? 0.38,
    }),
  );
  return layers;
}

function materialPalette(base: string, options: NoisyMaterialOptions): Record<string, string> {
  return {
    base,
    light: mixHex(base, "#ffffff", options.lightMix ?? 0.16),
    shadow: mixHex(base, "#000000", options.shadowMix ?? 0.18),
    dark: mixHex(base, "#000000", options.darkMix ?? 0.3),
  };
}

function mixHex(a: string, b: string, alpha: number): string {
  const ca = parseHex(a);
  const cb = parseHex(b);
  return hexColor([
    Math.round(ca[0] + (cb[0] - ca[0]) * alpha),
    Math.round(ca[1] + (cb[1] - ca[1]) * alpha),
    Math.round(ca[2] + (cb[2] - ca[2]) * alpha),
  ]);
}

function parseHex(hex: string): [number, number, number] {
  const normalized = hex.startsWith("#") ? hex.slice(1) : hex;
  return [
    Number.parseInt(normalized.slice(0, 2), 16),
    Number.parseInt(normalized.slice(2, 4), 16),
    Number.parseInt(normalized.slice(4, 6), 16),
  ];
}

function hexColor([r, g, b]: [number, number, number]): string {
  return `#${hexByte(r)}${hexByte(g)}${hexByte(b)}`;
}

function hexByte(value: number): string {
  return value.toString(16).padStart(2, "0");
}
