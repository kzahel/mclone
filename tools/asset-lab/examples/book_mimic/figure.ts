import { figure } from "../../src/dsl";

// An enchanted grimoire mimic with a readable open-book silhouette, ragged
// card pages, a cover eye, two tooth rows, and a fast snap-shut action.
export default figure("book_mimic", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  plane,
}) => {
  metadata({
    bodyPlans: ["other"],
    disposition: "hostile",
    groups: ["fantasy", "monster"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["archive", "book", "enchanted", "mimic", "scary"],
  });

  mat("leather", "#653548");
  mat("leather_light", "#8c4b5e");
  mat("leather_dark", "#352331");
  mat("gold", "#c59b45");
  mat("page", { color: "#d7c99d", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("page_dark", "#9d8b67");
  mat("mouth", "#25151d");
  mat("gum", "#7e3448");
  mat("tooth", "#eee3ba");
  mat("tongue", "#a94b69");
  mat("eye", "#d9e95e");

  asciiTexture("cover_runes", {
    palette: {
      ".": "#653548",
      "l": "#8c4b5e",
      "d": "#352331",
      "g": "#c59b45",
    },
    pixels: [
      "gggggggggggg",
      "g..........g",
      "g..g....g..g",
      "g...g..g...g",
      "g....gg....g",
      "g...g..g...g",
      "g..g....g..g",
      "g..........g",
      "gggggggggggg",
    ],
  });
  asciiTexture("watching_eye", {
    palette: {
      ".": "#352331",
      "g": "#c59b45",
      "e": "#d9e95e",
      "p": "#15131a",
    },
    pixels: [
      "....gg....",
      "..ggeegg..",
      ".geeeeeeg.",
      "geeeppppeg",
      "geeeppppeg",
      ".geeeeeeg.",
      "..ggeegg..",
      "....gg....",
    ],
  });
  asciiTexture("ragged_page", {
    palette: {
      ".": "transparent",
      "p": "#d7c99d",
      "d": "#9d8b67",
      "i": "#51452f",
      "r": "#8b493f",
    },
    pixels: [
      ".pppppppppp.",
      "pppppppppppp",
      "ppiiipppiiip",
      "pppppppppppp",
      "ppiiipppiiip",
      "ppppprrppppp",
      "ppiiiipppipp",
      "pppppppppppp",
      ".dppppppppd.",
    ],
  });

  part("bottom_cover", box({
    at: [0, 0.1, 0],
    size: [1.62, 0.16, 1.04],
    material: "leather",
    faces: { up: { texture: "cover_runes" }, down: { texture: "cover_runes" } },
  }));
  part("page_block", box({
    parent: "bottom_cover",
    at: [0, 0.2, 0],
    size: [1.44, 0.32, 0.86],
    material: "page_dark",
  }));
  part("mouth_cavity", box({
    parent: "page_block",
    at: [0, 0.01, -0.45],
    size: [1.14, 0.22, 0.1],
    material: "mouth",
  }));
  part("gum", box({
    parent: "mouth_cavity",
    at: [0, -0.06, -0.04],
    size: [0.98, 0.09, 0.1],
    material: "gum",
  }));
  part("tongue", box({
    parent: "page_block",
    at: [0.08, -0.05, -0.61],
    rot: [3, 0, -5],
    size: [0.34, 0.1, 0.56],
    material: "tongue",
    joint: { pivot: [0, 0, 0.25], axis: [0, 1, 0] },
  }));

  for (const [index, x] of [-0.42, -0.14, 0.14, 0.42].entries()) {
    part(`lower_tooth_${index + 1}`, box({
      parent: "page_block",
      at: [x, 0.17, -0.39],
      rot: [0, 0, index % 2 === 0 ? -5 : 5],
      size: [0.13, index % 2 === 0 ? 0.22 : 0.17, 0.14],
      material: "tooth",
    }));
  }

  part("spine", box({
    parent: "bottom_cover",
    at: [0, 0.3, 0.43],
    size: [1.5, 0.36, 0.18],
    material: "leather_dark",
  }));
  part("spine_band_l", box({
    parent: "spine",
    at: [-0.53, 0, 0],
    size: [0.15, 0.4, 0.22],
    material: "gold",
  }));
  part("spine_band_r", box({
    parent: "spine",
    at: [0.53, 0, 0],
    size: [0.15, 0.4, 0.22],
    material: "gold",
  }));

  for (const [index, [height, tilt]] of ([
    [0.09, 84],
    [0.12, 90],
    [0.15, 96],
  ] as const).entries()) {
    part(`loose_page_${index + 1}`, plane({
      parent: "spine",
      at: [0, height, -0.42],
      rot: [tilt, 0, index === 1 ? 2 : index === 0 ? -3 : 4],
      size: [1.32 - index * 0.05, 0.88 - index * 0.04],
      sidedness: "double",
      material: "page",
      texture: "ragged_page",
      joint: { pivot: [0, -0.4, 0], axis: [1, 0, 0] },
    }));
  }

  part("top_cover", box({
    parent: "spine",
    at: [0, 0.2, -0.43],
    rot: [20, 0, 0],
    size: [1.64, 0.16, 1.06],
    material: "leather_light",
    faces: { up: { texture: "cover_runes" } },
    joint: { pivot: [0, 0, 0.48], axis: [1, 0, 0] },
  }));
  part("eye_panel", box({
    parent: "top_cover",
    at: [0, 0.1, -0.02],
    size: [0.72, 0.1, 0.62],
    material: "gold",
    faces: { up: { texture: "watching_eye" } },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`cover_corner_${side}`, box({
      parent: "top_cover",
      at: [sign * 0.68, 0.04, -0.42],
      size: [0.2, 0.18, 0.2],
      material: "gold",
    }));
  }
  for (const [index, x] of [-0.42, -0.14, 0.14, 0.42].entries()) {
    part(`upper_tooth_${index + 1}`, box({
      parent: "top_cover",
      at: [x, -0.12, -0.4],
      rot: [0, 0, index % 2 === 0 ? 5 : -5],
      size: [0.13, index % 2 === 0 ? 0.22 : 0.17, 0.14],
      material: "tooth",
    }));
  }

  clip("page_whisper", {
    label: "Page whisper",
    role: "idle",
    fps: 24,
    loop: true,
    keys: [
      ["top_cover", 0, { rot: [0, 0, 0] }],
      ["top_cover", 0.55, { rot: [2.5, 0, 0] }],
      ["top_cover", 1.1, { rot: [-1, 0, 0] }],
      ["top_cover", 1.65, { rot: [1.5, 0, 0] }],
      ["top_cover", 2.2, { rot: [0, 0, 0] }],
      ["loose_page_1", 0, { rot: [0, 0, 0] }],
      ["loose_page_1", 0.55, { rot: [4, 0, -3] }],
      ["loose_page_1", 1.1, { rot: [-3, 0, 2] }],
      ["loose_page_1", 1.65, { rot: [2, 0, -2] }],
      ["loose_page_1", 2.2, { rot: [0, 0, 0] }],
      ["loose_page_3", 0, { rot: [0, 0, 0] }],
      ["loose_page_3", 0.55, { rot: [-3, 0, 2] }],
      ["loose_page_3", 1.1, { rot: [4, 0, -2] }],
      ["loose_page_3", 1.65, { rot: [-2, 0, 3] }],
      ["loose_page_3", 2.2, { rot: [0, 0, 0] }],
      ["tongue", 0, { rot: [0, 0, 0] }],
      ["tongue", 0.55, { rot: [0, 5, 5] }],
      ["tongue", 1.1, { rot: [0, -5, -4] }],
      ["tongue", 1.65, { rot: [0, 4, 3] }],
      ["tongue", 2.2, { rot: [0, 0, 0] }],
    ],
  });
  clip("snap_shut", {
    label: "Snap shut",
    role: "action",
    nextClip: "page_whisper",
    fps: 30,
    loop: false,
    keys: [
      ["top_cover", 0, { rot: [0, 0, 0] }],
      ["top_cover", 0.2, { rot: [13, 0, 0] }],
      ["top_cover", 0.42, { rot: [-20, 0, 0] }],
      ["top_cover", 0.55, { rot: [-16, 0, 0] }],
      ["top_cover", 0.72, { rot: [5, 0, 0] }],
      ["top_cover", 1.14, { rot: [0, 0, 0] }],
      ["loose_page_1", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["loose_page_1", 0.2, { at: [0, 0.05, 0], rot: [10, 0, 0] }],
      ["loose_page_1", 0.42, { at: [0, -0.05, 0], rot: [-6, 0, 0] }],
      ["loose_page_1", 1.14, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["loose_page_2", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["loose_page_2", 0.2, { at: [0, 0.08, 0], rot: [13, 0, 0] }],
      ["loose_page_2", 0.42, { at: [0, -0.07, 0], rot: [-8, 0, 0] }],
      ["loose_page_2", 1.14, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["loose_page_3", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["loose_page_3", 0.2, { at: [0, 0.06, 0], rot: [8, 0, 0] }],
      ["loose_page_3", 0.42, { at: [0, -0.06, 0], rot: [-5, 0, 0] }],
      ["loose_page_3", 1.14, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["tongue", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["tongue", 0.2, { at: [0, 0, -0.22], scale: [1, 1, 1.25] }],
      ["tongue", 0.42, { at: [0, 0, 0.25], scale: [1, 1, 0.3] }],
      ["tongue", 0.72, { at: [0, 0, -0.08], scale: [1, 1, 0.82] }],
      ["tongue", 1.14, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ],
  });
  defaultClip("page_whisper");
});
