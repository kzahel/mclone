import { figure } from "../../src/dsl";

// A box-only adult Indian peacock with an iridescent neck, small crest,
// patterned folded wings, and a seven-feather display fan. The fan is parented
// to one tail root so its subtle walk follow-through cannot separate feathers.
export default figure("peacock", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("blue", "#176b87");
  mat("blue_light", "#2c91a2");
  mat("blue_dark", "#103f63");
  mat("green", "#267052");
  mat("green_light", "#58a263");
  mat("green_dark", "#174c3e");
  mat("bronze", "#9a7040");
  mat("cream", "#dfd5ae");
  mat("eye", "#171513");
  mat("beak", "#d2b67b");
  mat("leg", "#8a8069");

  asciiTexture("face", {
    palette: { ".": "#176b87", "e": "#171513", "c": "#dfd5ae", "l": "#2c91a2" },
    pixels: [
      "ll....ll",
      ".ce..ec.",
      ".ce..ec.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("wing_pattern", {
    palette: { ".": "#9a7040", "g": "#267052", "d": "#174c3e", "c": "#dfd5ae" },
    pixels: [
      "cccccccccc",
      "c........c",
      ".gg..gg...",
      "ggdgggdggg",
      "g.gggg.ggg",
      "dddddddddd",
    ],
  });
  asciiTexture("tail_eye", {
    palette: { "g": "#267052", "l": "#58a263", "b": "#176b87", "d": "#103f63", "c": "#d2b67b" },
    pixels: [
      "gggggggg",
      "gllllllg",
      "glcccclg",
      "glcbbclg",
      "glbddblg",
      "glcbbclg",
      "glcccclg",
      "gllllllg",
      "gggggggg",
      "gggllggg",
      "gggggggg",
      "gggggggg",
    ],
  });
  asciiTexture("foot_front", {
    palette: { ".": "#8a8069", "d": "#174c3e" },
    pixels: [
      "........",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.96, 0.04],
    size: [0.66, 0.68, 0.92],
    material: "green_dark",
  }));
  part("breast", box({
    parent: "body",
    at: [0, 0.04, -0.51],
    size: [0.52, 0.54, 0.18],
    material: "blue",
  }));
  part("wing_l", box({
    parent: "body",
    at: [-0.37, 0.02, 0.08],
    size: [0.09, 0.5, 0.7],
    material: "bronze",
    faces: { west: { texture: "wing_pattern" } },
    joint: { pivot: [0.04, 0.16, -0.2], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.37, 0.02, 0.08],
    size: [0.09, 0.5, 0.7],
    material: "bronze",
    faces: { east: { texture: "wing_pattern" } },
    joint: { pivot: [-0.04, 0.16, -0.2], axis: [0, 0, 1] },
  }));

  part("neck", box({
    parent: "body",
    at: [0, 0.52, -0.33],
    rot: [-8, 0, 0],
    size: [0.34, 0.82, 0.34],
    material: "blue",
    joint: { pivot: [0, -0.39, 0.06], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.5, -0.08],
    rot: [7, 0, 0],
    size: [0.42, 0.38, 0.42],
    material: "blue_light",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.07, -0.31],
    size: [0.28, 0.16, 0.22],
    material: "beak",
  }));
  for (const [index, x, lean] of [
    [1, -0.12, -8],
    [2, 0, 0],
    [3, 0.12, 8],
  ] as const) {
    part(`crest_${index}`, box({
      parent: "head",
      at: [x, 0.32, 0.03],
      rot: [0, 0, lean],
      size: [0.055, 0.3, 0.055],
      material: "blue_dark",
    }));
  }

  part("tail_root", box({
    parent: "body",
    at: [0, 0.1, 0.58],
    rot: [3, 0, 0],
    size: [0.56, 0.28, 0.22],
    material: "green_dark",
    joint: { pivot: [0, -0.12, -0.08], axis: [1, 0, 0] },
  }));
  for (const [index, x, y, height, lean, material] of [
    [1, -0.68, 0.48, 1.12, -18, "green"],
    [2, -0.47, 0.57, 1.34, -13, "green_light"],
    [3, -0.24, 0.64, 1.5, -7, "green"],
    [4, 0, 0.68, 1.58, 0, "green_light"],
    [5, 0.24, 0.64, 1.5, 7, "green"],
    [6, 0.47, 0.57, 1.34, 13, "green_light"],
    [7, 0.68, 0.48, 1.12, 18, "green"],
  ] as const) {
    part(`tail_feather_${index}`, box({
      parent: "tail_root",
      at: [x, y, 0.14],
      rot: [0, 0, lean],
      size: [0.2, height, 0.08],
      material,
      faces: {
        north: { texture: "tail_eye" },
        south: { texture: "tail_eye" },
      },
    }));
  }

  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.55, -0.02],
      size: [0.11, 0.5, 0.12],
      material: "leg",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.32, -0.08],
      size: [0.26, 0.1, 0.42],
      material: "leg",
      faces: { north: { texture: "foot_front" } },
    }));
  }

  bipedWalk("walk", {
    fps: 18,
    duration: 1.12,
    cycleDistance: 0.58,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.015,
    head: "head",
    headSwingDegrees: 2,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.66,
    swingDegrees: 18,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 4, center: -1, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -4, center: 1, frequency: 2 }),
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.1 }),
      followThrough("tail_root", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.14 }),
    ],
  });
});
