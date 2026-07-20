import { figure } from "../../src/dsl";

// A box-only adult donkey with a compact gray body, large four-box ears,
// upright dark mane, dorsal stripe, pale muzzle, and tasseled tail.
export default figure("donkey", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#7b7369");
  mat("coat_light", "#a39b8f");
  mat("coat_dark", "#3c3935");
  mat("coat_shadow", "#5d5751");
  mat("cream", "#d1c7b6");
  mat("muzzle", "#b1a28f");
  mat("point", "#2d2a27");
  mat("hoof", "#1c1b1a");
  mat("ear_inner", "#8d716b");

  asciiTexture("face", {
    palette: { ".": "#7b7369", "d": "#3c3935", "e": "#151311", "c": "#d1c7b6" },
    pixels: [
      "dd....dd",
      ".e....e.",
      "..c..c..",
      "..cccc..",
      "...cc...",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#b1a28f", "n": "#29231f", "c": "#d1c7b6" },
    pixels: [
      "..cccc..",
      ".cccccc.",
      ".nn..nn.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#7b7369", "l": "#a39b8f", "d": "#3c3935", "c": "#d1c7b6" },
    pixels: [
      "dddddddddddddd",
      "d............d",
      "..llllllllll..",
      "..............",
      "...cccccccc...",
      "..cccccccccc..",
    ],
  });
  asciiTexture("dorsal_stripe", {
    palette: { ".": "#7b7369", "d": "#3c3935", "l": "#a39b8f" },
    pixels: [
      "....dd....",
      "....dd....",
      "lll.dd.lll",
      "....dd....",
      "....dd....",
    ],
  });
  asciiTexture("leg_stocking", {
    palette: { ".": "#7b7369", "d": "#3c3935" },
    pixels: [
      "....",
      "....",
      "....",
      "dddd",
      "dddd",
      "dddd",
      "dddd",
      "dddd",
    ],
  });

  part("body", box({
    at: [0, 1.06, 0.02],
    size: [0.8, 0.68, 1.48],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
      up: { texture: "dorsal_stripe" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, 0.03],
    size: [0.62, 0.1, 1.06],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.34, -0.69],
    rot: [-38, 0, 0],
    size: [0.42, 0.76, 0.44],
    material: "coat_shadow",
    joint: { pivot: [0, -0.37, 0.04], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.46, -0.18],
    rot: [28, 0, 0],
    size: [0.42, 0.46, 0.58],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.43],
    size: [0.34, 0.28, 0.32],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x, lean] of [
    ["l", -0.15, -8],
    ["r", 0.15, 8],
  ] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.42, 0.1],
      rot: [-7, 0, lean],
      size: [0.16, 0.48, 0.12],
      material: "coat_dark",
      joint: { pivot: [0, -0.22, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.07],
      size: [0.09, 0.34, 0.035],
      material: "ear_inner",
    }));
  }

  for (const [index, y, height] of [
    [1, 0.25, 0.23],
    [2, 0.05, 0.25],
    [3, -0.16, 0.25],
  ] as const) {
    part(`mane_${index}`, box({
      parent: "neck",
      at: [0, y, 0.24],
      rot: [5, 0, 0],
      size: [0.13, height, 0.14],
      material: "point",
    }));
  }
  part("forelock", box({
    parent: "head",
    at: [0, 0.23, 0.21],
    rot: [20, 0, 0],
    size: [0.14, 0.18, 0.12],
    material: "point",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.28, -0.49],
    ["fr", 0.28, -0.49],
    ["bl", -0.28, 0.51],
    ["br", 0.28, 0.51],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.57, z],
      size: [0.18, 0.64, 0.19],
      material: "coat",
      faces: {
        north: { texture: "leg_stocking" },
        south: { texture: "leg_stocking" },
        east: { texture: "leg_stocking" },
        west: { texture: "leg_stocking" },
      },
      joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.38, -0.015],
      size: [0.22, 0.12, 0.25],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.15, 0.77],
    rot: [17, 0, 0],
    size: [0.11, 0.48, 0.12],
    material: "coat_dark",
    joint: { pivot: [0, 0.23, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.34, -0.02],
    size: [0.22, 0.3, 0.21],
    material: "point",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.02,
    cycleDistance: 0.84,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.12 }),
      followThrough("mane_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.6, lag: 0.13 }),
      followThrough("mane_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.65, lag: 0.15 }),
      followThrough("mane_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.17 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.75, lag: 0.19 }),
    ],
  });
});
