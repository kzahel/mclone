import { figure } from "../../src/dsl";

// A rooted carnivorous orchid with four jaw-like petals, a luminous lure,
// searching tendrils, and a fast open-snap-recoil attack.
export default figure("maw_orchid", ({
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
    bodyPlans: ["rooted"],
    disposition: "hostile",
    groups: ["plant", "monster"],
    habitats: ["land"],
    scale: "large",
    themes: ["carnivorous", "living-growth", "orchid", "scary", "tropical"],
  });

  mat("root", "#4d5330");
  mat("stem", "#4f7a3c");
  mat("stem_light", "#72a34e");
  mat("leaf", { color: "#376839", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("petal", { color: "#a94678", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("petal_light", { color: "#df6e9a", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("petal_dark", "#6d315c");
  mat("mouth", "#421d31");
  mat("tooth", "#eee2bd");
  mat("lure", { color: "#d7ff72", alphaMode: "additive", opacity: 0.88 });

  asciiTexture("leaf_cutout", {
    palette: { ".": "transparent", "g": "#376839", "l": "#72a34e", "d": "#244b2d" },
    pixels: [
      "...gggg...",
      ".gggglggg.",
      "ggglllgggg",
      "gglllllggg",
      ".gglllggg.",
      "..gglgg...",
      "...ggg....",
    ],
  });
  asciiTexture("petal_spots", {
    palette: {
      ".": "transparent",
      "p": "#a94678",
      "l": "#df6e9a",
      "d": "#6d315c",
      "s": "#f1a1b4",
    },
    pixels: [
      "..pppppp..",
      ".pplllspp.",
      "pplpsplppp",
      "pppllppppp",
      "ppdppppdpp",
      ".pppddppp.",
      "..pppppp..",
      "...pppp...",
    ],
  });
  asciiTexture("maw", {
    palette: { ".": "#421d31", "d": "#24131f", "t": "#eee2bd", "l": "#7d3152" },
    pixels: [
      "tt.tt..tt.tt",
      "t..t....t..t",
      ".ll......ll.",
      "..dddddddd..",
      "..d......d..",
      ".ll......ll.",
      "t..t....t..t",
      "tt.tt..tt.tt",
    ],
  });
  asciiTexture("lure_rune", {
    palette: { ".": "#d7ff72", "l": "#f0ffb0", "d": "#83b84a" },
    pixels: [".llll.", "ll..ll", "l.dd.l", "l.dd.l", "ll..ll", ".llll."],
  });

  part("root_bulb", box({
    at: [0, 0.32, 0.06],
    size: [0.76, 0.48, 0.72],
    material: "root",
  }));
  for (const [name, x, z, rot] of [
    ["l", -0.48, 0, -8],
    ["r", 0.48, 0, 8],
    ["front", 0, -0.46, 0],
    ["back", 0, 0.46, 0],
  ] as const) {
    part(`ground_root_${name}`, box({
      parent: "root_bulb",
      at: [x, -0.18, z],
      rot: [0, name === "front" || name === "back" ? 0 : rot, name === "front" ? 90 : name === "back" ? -90 : rot],
      size: [name === "front" || name === "back" ? 0.18 : 0.82, 0.12, name === "front" || name === "back" ? 0.8 : 0.2],
      material: "root",
    }));
  }

  part("stem_lower", box({
    parent: "root_bulb",
    at: [0, 0.62, 0],
    rot: [-3, 0, 2],
    size: [0.38, 1.02, 0.38],
    material: "stem",
    joint: { pivot: [0, -0.47, 0], axis: [0, 0, 1] },
  }));
  part("stem_upper", box({
    parent: "stem_lower",
    at: [0.04, 0.66, -0.05],
    rot: [5, 0, -4],
    size: [0.26, 0.54, 0.26],
    material: "stem_light",
    joint: { pivot: [0, -0.24, 0.05], axis: [0, 0, 1] },
  }));

  const leafFaces = {
    north: { texture: "leaf_cutout" },
    south: { texture: "leaf_cutout" },
    up: { texture: "leaf_cutout" },
    down: { texture: "leaf_cutout" },
  } as const;
  part("leaf_l", box({
    parent: "stem_lower",
    at: [-0.48, -0.12, 0.02],
    rot: [3, -8, -18],
    size: [0.92, 0.14, 0.5],
    material: "leaf",
    faces: leafFaces,
    joint: { pivot: [0.42, 0, 0], axis: [0, 0, 1] },
  }));
  part("leaf_r", box({
    parent: "stem_lower",
    at: [0.5, 0.06, 0.03],
    rot: [-4, 7, 16],
    size: [0.96, 0.14, 0.52],
    material: "leaf",
    faces: leafFaces,
    joint: { pivot: [-0.44, 0, 0], axis: [0, 0, 1] },
  }));

  part("flower_core", box({
    parent: "stem_upper",
    at: [0, 0.38, -0.12],
    rot: [4, 0, 0],
    size: [0.74, 0.66, 0.48],
    material: "petal_dark",
    joint: { pivot: [0, -0.28, 0.12], axis: [0, 1, 0] },
  }));
  part("mouth", box({
    parent: "flower_core",
    at: [0, 0, -0.3],
    size: [0.6, 0.52, 0.18],
    material: "mouth",
    faces: { north: { texture: "maw" } },
  }));

  part("petal_top", plane({
    parent: "flower_core",
    at: [0, 0.5, -0.08],
    rot: [8, 0, 0],
    size: [0.68, 0.62],
    sidedness: "double",
    material: "petal_light",
    texture: "petal_spots",
    joint: { pivot: [0, -0.27, 0.05], axis: [1, 0, 0] },
  }));
  part("petal_bottom", plane({
    parent: "flower_core",
    at: [0, -0.48, -0.06],
    rot: [-8, 0, 0],
    size: [0.64, 0.58],
    sidedness: "double",
    material: "petal",
    texture: "petal_spots",
    joint: { pivot: [0, 0.26, 0.05], axis: [1, 0, 0] },
  }));
  part("petal_l", plane({
    parent: "flower_core",
    at: [-0.52, 0, -0.07],
    rot: [0, 0, -8],
    size: [0.62, 0.64],
    sidedness: "double",
    material: "petal",
    texture: "petal_spots",
    joint: { pivot: [0.27, 0, 0.05], axis: [0, 0, 1] },
  }));
  part("petal_r", plane({
    parent: "flower_core",
    at: [0.52, 0, -0.07],
    rot: [0, 0, 8],
    size: [0.62, 0.64],
    sidedness: "double",
    material: "petal_light",
    texture: "petal_spots",
    joint: { pivot: [-0.27, 0, 0.05], axis: [0, 0, 1] },
  }));

  part("lure_stalk", box({
    parent: "mouth",
    at: [0, 0.02, -0.3],
    rot: [0, 0, -5],
    size: [0.08, 0.08, 0.5],
    material: "stem_light",
    joint: { pivot: [0, 0, 0.22], axis: [0, 1, 0] },
  }));
  part("lure_bulb", box({
    parent: "lure_stalk",
    at: [0, 0, -0.31],
    rot: [0, 0, 45],
    size: [0.25, 0.25, 0.22],
    material: "lure",
    faces: { north: { texture: "lure_rune" }, south: { texture: "lure_rune" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`tendril_${side}`, box({
      parent: "root_bulb",
      at: [sign * 0.35, 0.34, -0.1],
      rot: [-12, sign * 8, sign * -24],
      size: [0.12, 0.74, 0.12],
      material: "stem_light",
      joint: { pivot: [0, -0.34, 0], axis: [0, 0, 1] },
    }));
    part(`tendril_tip_${side}`, box({
      parent: `tendril_${side}`,
      at: [sign * 0.08, 0.45, -0.03],
      rot: [0, 0, sign * -18],
      size: [0.09, 0.3, 0.09],
      material: "stem",
    }));
  }

  clip("tendril_watch", {
    label: "Tendril watch",
    role: "idle",
    fps: 24,
    loop: true,
    keys: [
      ["stem_lower", 0, { rot: [0, 0, 0] }],
      ["stem_lower", 0.7, { rot: [1, 0, 3] }],
      ["stem_lower", 1.4, { rot: [-1, 0, -3] }],
      ["stem_lower", 2.1, { rot: [0, 0, 0] }],
      ["flower_core", 0, { rot: [0, 0, 0] }],
      ["flower_core", 0.7, { rot: [0, 7, -2] }],
      ["flower_core", 1.4, { rot: [0, -7, 2] }],
      ["flower_core", 2.1, { rot: [0, 0, 0] }],
      ["tendril_l", 0, { rot: [0, 0, 0] }],
      ["tendril_l", 0.7, { rot: [0, 0, 12] }],
      ["tendril_l", 1.4, { rot: [0, 0, -9] }],
      ["tendril_l", 2.1, { rot: [0, 0, 0] }],
      ["tendril_r", 0, { rot: [0, 0, 0] }],
      ["tendril_r", 0.7, { rot: [0, 0, -10] }],
      ["tendril_r", 1.4, { rot: [0, 0, 13] }],
      ["tendril_r", 2.1, { rot: [0, 0, 0] }],
      ["lure_stalk", 0, { rot: [0, 0, 0] }],
      ["lure_stalk", 0.7, { rot: [0, 8, 4] }],
      ["lure_stalk", 1.4, { rot: [0, -8, -4] }],
      ["lure_stalk", 2.1, { rot: [0, 0, 0] }],
    ],
  });
  clip("snap_trap", {
    label: "Snap trap",
    role: "action",
    nextClip: "tendril_watch",
    fps: 30,
    loop: false,
    keys: [
      ["stem_upper", 0, { rot: [0, 0, 0] }],
      ["stem_upper", 0.28, { rot: [-8, 0, 4] }],
      ["stem_upper", 0.5, { rot: [16, 0, -5] }],
      ["stem_upper", 0.72, { rot: [9, 0, -2] }],
      ["stem_upper", 1.18, { rot: [0, 0, 0] }],
      ["petal_l", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_l", 0.28, { at: [-0.08, 0, 0], rot: [0, 0, -24] }],
      ["petal_l", 0.5, { at: [0.2, 0, 0], rot: [0, 0, 8] }],
      ["petal_l", 0.72, { at: [-0.03, 0, 0], rot: [0, 0, -10] }],
      ["petal_l", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_r", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_r", 0.28, { at: [0.08, 0, 0], rot: [0, 0, 24] }],
      ["petal_r", 0.5, { at: [-0.2, 0, 0], rot: [0, 0, -8] }],
      ["petal_r", 0.72, { at: [0.03, 0, 0], rot: [0, 0, 10] }],
      ["petal_r", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_top", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_top", 0.28, { at: [0, 0.08, 0], rot: [26, 0, 0] }],
      ["petal_top", 0.5, { at: [0, -0.2, 0], rot: [-8, 0, 0] }],
      ["petal_top", 0.72, { at: [0, 0.03, 0], rot: [11, 0, 0] }],
      ["petal_top", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_bottom", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["petal_bottom", 0.28, { at: [0, -0.08, 0], rot: [-26, 0, 0] }],
      ["petal_bottom", 0.5, { at: [0, 0.2, 0], rot: [8, 0, 0] }],
      ["petal_bottom", 0.72, { at: [0, -0.03, 0], rot: [-11, 0, 0] }],
      ["petal_bottom", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["lure_stalk", 0, { scale: [1, 1, 1] }],
      ["lure_stalk", 0.28, { scale: [1, 1, 1.18] }],
      ["lure_stalk", 0.5, { scale: [1, 1, 0.28] }],
      ["lure_stalk", 0.72, { scale: [1, 1, 0.65] }],
      ["lure_stalk", 1.18, { scale: [1, 1, 1] }],
    ],
  });
  defaultClip("tendril_watch");
});
