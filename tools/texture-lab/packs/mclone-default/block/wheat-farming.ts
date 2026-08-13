import type { TextureLabApi } from "../../../src/dsl";

const empty = "................";

export function defineWheatFarmingTextures(api: TextureLabApi): void {
  defineFarmland(api, "farmland", false);
  defineFarmland(api, "farmland_moist", true);

  api.palette("wheat_crop", {
    transparent: "#00000000",
    green_dark: "#3d6327",
    green: "#64883b",
    green_light: "#8ca64d",
    straw_dark: "#8b652b",
    straw: "#c0923e",
    grain: "#e1b957",
    highlight: "#f2d47a",
  });

  const stages: string[][] = [
    [
      empty, empty, empty, empty, empty, empty, empty, empty,
      empty, empty, empty, empty, empty,
      ".......g........",
      "......ggg.......",
      ".......d........",
    ],
    [
      empty, empty, empty, empty, empty, empty, empty, empty,
      empty, empty, empty,
      ".....l.g.l......",
      ".....ggggg......",
      "......ggg.......",
      "......ddd.......",
      ".......d........",
    ],
    [
      empty, empty, empty, empty, empty, empty, empty, empty,
      "....l..g..l.....",
      "....ggggggg.....",
      ".....ggggg......",
      ".....g.g.g......",
      ".....g.g.g......",
      ".....d.d.d......",
      ".....d.d.d......",
      ".....d.d.d......",
    ],
    [
      empty, empty, empty, empty, empty, empty,
      "...l...g...l....",
      "...ggggggggg....",
      "....ggggggg.....",
      "....g.g.g.g.....",
      "....g.g.g.g.....",
      "....d.d.d.d.....",
      "....d.d.d.d.....",
      "....d.d.d.d.....",
      "....d.d.d.d.....",
      "....d.d.d.d.....",
    ],
    [
      empty, empty, empty, empty,
      "..l...l.g.l.....",
      "..gggggggggg....",
      "...gggggggg.....",
      "...g.g.g.g.g....",
      "...g.g.g.g.g....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
    ],
    [
      empty, empty,
      "..s.s.s.s.s.s...",
      ".slslsssslsls...",
      ".slglgssglgls...",
      "..gggggggggg....",
      "...g.g.g.g.g....",
      "...s.s.s.s.s....",
      "...s.s.s.s.s....",
      "...s.s.s.s.s....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
      "...d.d.d.d.d....",
    ],
    [
      empty,
      "..h.h.h.h.h.h...",
      ".hahahahahahah..",
      ".asasasasasasa..",
      "..sasasasasa....",
      "..s.s.s.s.s.....",
      "..s.s.s.s.s.....",
      "..s.s.s.s.s.....",
      "..s.s.s.s.s.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
      "..d.d.d.d.d.....",
    ],
    [
      ".h.h.h.h.h.h.h...",
      ".hahhahahahhah...",
      ".haahahahahaha...",
      ".hahahahahahah...",
      "..asasasasasa....",
      "..s.s.s.s.s......",
      "..s.s.s.s.s......",
      "..s.s.s.s.s......",
      "..s.s.s.s.s......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
      "..d.d.d.d.d......",
    ],
  ];

  for (const [age, pixels] of stages.entries()) {
    api.texture(`wheat_stage${age}`, {
      size: 32,
      source: "final-color",
      palette: "wheat_crop",
      base: "transparent",
      exportPath: `assets/mclone/textures/block/wheat_stage${age}.png`,
      preview: { checkerboard: true, cube: false, rotation: false, tiling: "none" },
      catalog: {
        tiling: "none",
        rotation: "model-driven",
        tags: ["cutout", "crop", `age:${age}`],
      },
      layers: [
        api.mask({
          colors: {
            d: "green_dark",
            g: "green",
            l: "green_light",
            s: "straw_dark",
            a: "straw",
            h: "highlight",
          },
          opacity: 1,
          upscale: "nearest",
          authoring: { role: "structure", label: `WHEAT AGE ${age}` },
          pixels,
        }),
      ],
    });
  }
}

function defineFarmland(api: TextureLabApi, name: string, moist: boolean): void {
  const paletteName = `wheat_${name}`;
  api.palette(paletteName, {
    base: moist ? "#3c2b22" : "#755039",
    ridge: moist ? "#574033" : "#91694a",
    light: moist ? "#6b4b39" : "#aa805c",
    furrow: moist ? "#241c19" : "#503728",
    grit: moist ? "#7b5943" : "#c0956c",
  });
  api.texture(name, {
    size: 32,
    source: "final-color",
    palette: paletteName,
    base: "base",
    exportPath: `assets/mclone/textures/block/${name}.png`,
    preview: { cube: true, rotation: true, tiling: "xy" },
    catalog: {
      tiling: "xy",
      rotation: "fixed",
      tags: ["soil", "furrowed", moist ? "hydrated" : "dry"],
    },
    layers: [
      api.macroNoise({
        seed: `wheat-${name}-clods`,
        frequency: 6,
        octaves: 2,
        colors: ["furrow", "base", "ridge", "light"],
        opacity: 0.26,
        contrast: 1.1,
      }),
      api.mask({
        colors: { f: "furrow", r: "ridge", l: "light" },
        opacity: 0.82,
        upscale: "smooth",
        authoring: { role: "structure", label: "PARALLEL PLOUGH FURROWS" },
        pixels: [
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
          "ffrrrffrrrffrrrf",
        ],
      }),
      api.speckles({
        seed: `wheat-${name}-grit`,
        density: 0.08,
        colors: ["grit", "furrow"],
        opacity: 0.38,
      }),
    ],
  });
}
