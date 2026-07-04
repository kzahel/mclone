import type { TextureLabApi } from "../../../src/dsl";

const empty16 = "................";

export function definePartialShapeTextures({ palette, texture, block, mask, speckles }: TextureLabApi): void {
  palette("partial_shape_cutout", {
    transparent: "#00000000",
    glass: "#b9edf088",
    glass_edge: "#e0fbfccc",
    rail_dark: "#3b2b1e",
    rail_wood: "#725133",
    rail_metal: "#b9b8ad",
    wood_dark: "#5d3a1f",
    wood: "#8a5b2e",
    wood_light: "#b98247",
    torch_wood: "#6a4325",
    torch_tip: "#b54a1c",
    flame: "#f1a726",
    flame_light: "#ffe36a",
  });

  texture("glass", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/glass.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "pane", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          g: "glass",
          e: "glass_edge",
        },
        opacity: 1,
        upscale: "smooth",
        authoring: {
          role: "structure",
          label: "GLASS PANE FACE",
        },
        pixels: [
          "eeeeeeeeeeeeeeee",
          "egggggggggggggge",
          "eg............ge",
          "eg............ge",
          "eg...g....g...ge",
          "eg............ge",
          "eg............ge",
          "egggg....gggggge",
          "egggg....gggggge",
          "eg............ge",
          "eg............ge",
          "eg...g....g...ge",
          "eg............ge",
          "eg............ge",
          "egggggggggggggge",
          "eeeeeeeeeeeeeeee",
        ],
      }),
    ],
  });

  texture("glass_pane_top", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/glass_pane_top.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "pane", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          g: "glass",
          e: "glass_edge",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "GLASS PANE EDGE",
        },
        pixels: [
          empty16,
          empty16,
          "....eeeeeeee....",
          "...eeeeeeeeee...",
          "..eeeeggggeeee..",
          "..eeeggggggeee..",
          "..eeeggggggeee..",
          "..eeeeggggeeee..",
          "..eeeeggggeeee..",
          "..eeeggggggeee..",
          "..eeeggggggeee..",
          "..eeeeggggeeee..",
          "...eeeeeeeeee...",
          "....eeeeeeee....",
          empty16,
          empty16,
        ],
      }),
    ],
  });

  texture("rail", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/rail.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "ground", "model:rail"],
    },
    layers: [
      mask({
        colors: {
          d: "rail_dark",
          w: "rail_wood",
          m: "rail_metal",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "RAIL SPRITE",
        },
        pixels: [
          empty16,
          "..m..........m..",
          "..m..........m..",
          ".wwwwwwwwwwwwww.",
          "..m..........m..",
          "..m..........m..",
          "....dddddddd....",
          "..m..........m..",
          "..m..........m..",
          ".wwwwwwwwwwwwww.",
          "..m..........m..",
          "..m..........m..",
          "....dddddddd....",
          "..m..........m..",
          "..m..........m..",
          empty16,
        ],
      }),
    ],
  });

  texture("torch", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/torch.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "torch", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          d: "wood_dark",
          w: "torch_wood",
          t: "torch_tip",
          f: "flame",
          l: "flame_light",
        },
        opacity: 1,
        upscale: "smooth",
        authoring: {
          role: "structure",
          label: "TORCH SPRITE",
        },
        pixels: [
          empty16,
          ".......ll.......",
          "......lffl......",
          "......lffl......",
          ".......tt.......",
          ".......ww.......",
          ".......ww.......",
          ".......ww.......",
          ".......ww.......",
          ".......ww.......",
          ".......ww.......",
          ".......ww.......",
          ".......dw.......",
          ".......dd.......",
          empty16,
          empty16,
        ],
      }),
      speckles({
        seed: "torch-flame-flecks",
        density: 0.025,
        colors: ["flame_light"],
        opacity: 0.55,
      }),
    ],
  });

  texture("oak_door_top", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/oak_door_top.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "door", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          d: "wood_dark",
          w: "wood",
          l: "wood_light",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "OAK DOOR TOP",
        },
        pixels: [
          "dddddddddddddddd",
          "dlllllllllllllld",
          "dlwwwwwwwwwwwwld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlww....ww....ld",
          "dlwwwwwwwwwwwwld",
          "dlllllllllllllld",
          "dddddddddddddddd",
        ],
      }),
    ],
  });

  texture("oak_door_bottom", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/oak_door_bottom.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "door", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          d: "wood_dark",
          w: "wood",
          l: "wood_light",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "OAK DOOR BOTTOM",
        },
        pixels: [
          "dddddddddddddddd",
          "dlllllllllllllld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwddwwwwwwld",
          "dlwwwwddwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlwwwwwwwwwwwwld",
          "dlllllllllllllld",
          "dddddddddddddddd",
        ],
      }),
    ],
  });

  texture("oak_trapdoor", {
    size: 32,
    source: "final-color",
    palette: "partial_shape_cutout",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/oak_trapdoor.png",
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "none",
    },
    catalog: {
      tiling: "none",
      rotation: "model-driven",
      tags: ["cutout", "trapdoor", "model:partial"],
    },
    layers: [
      mask({
        colors: {
          d: "wood_dark",
          w: "wood",
          l: "wood_light",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "OAK TRAPDOOR",
        },
        pixels: [
          "dddddddddddddddd",
          "dlllllllllllllld",
          "dlwwwwwwwwwwwwld",
          "dlww..ww..wwwwld",
          "dlww..ww..wwwwld",
          "dlwwwwwwwwwwwwld",
          "dlllllllllllllld",
          "dddddddddddddddd",
          "dddddddddddddddd",
          "dlllllllllllllld",
          "dlwwwwwwwwwwwwld",
          "dlwwww..ww..wwld",
          "dlwwww..ww..wwld",
          "dlwwwwwwwwwwwwld",
          "dlllllllllllllld",
          "dddddddddddddddd",
        ],
      }),
    ],
  });

  block("glass-pane", {
    kind: "pane",
    faces: {
      side: "glass",
      top: "glass_pane_top",
    },
  });

  block("rail", {
    kind: "rail",
    faces: {
      top: "rail",
    },
  });

  block("torch", {
    kind: "torch",
    faces: {
      side: "torch",
    },
  });

  block("oak-door", {
    kind: "door",
    faces: {
      top: "oak_door_top",
      bottom: "oak_door_bottom",
    },
  });

  block("oak-trapdoor", {
    kind: "trapdoor",
    faces: {
      top: "oak_trapdoor",
    },
  });
}
