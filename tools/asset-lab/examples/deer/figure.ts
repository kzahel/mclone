import { figure } from "../../src/dsl";

// A box-only white-tailed buck built on the reviewed long-legged quadruped
// grammar. The narrow body, large ears, white tail, and forked antlers keep it
// distinct from the heavier horse even at review-sheet scale.
export default figure("deer", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#9a6338");
  mat("coat_light", "#c28a58");
  mat("coat_dark", "#5a3823");
  mat("cream", "#ead7b4");
  mat("hoof", "#241a16");
  mat("antler", "#b99a70");
  mat("ear_inner", "#d59a82");

  asciiTexture("face", {
    palette: { ".": "#9a6338", "e": "#17100d", "c": "#ead7b4", "d": "#5a3823" },
    pixels: [
      "dd....dd",
      ".e....e.",
      "..c..c..",
      ".cccccc.",
      "..cccc..",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#ead7b4", "n": "#211611" },
    pixels: [
      "........",
      ".nn..nn.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#9a6338", "l": "#c28a58", "c": "#ead7b4" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "............",
      "............",
      "..cccccccc..",
      ".cccccccccc.",
    ],
  });

  part("body", box({
    at: [0, 1.18, 0.04],
    size: [0.68, 0.6, 1.48],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.18, -0.7],
    size: [0.54, 0.42, 0.2],
    material: "coat_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, 0.02],
    size: [0.52, 0.1, 1.02],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.35, -0.69],
    rot: [-37, 0, 0],
    size: [0.4, 0.78, 0.4],
    material: "coat_light",
    joint: { pivot: [0, -0.39, 0.08], axis: [1, 0, 0] },
  }));
  part("throat", box({
    parent: "neck",
    at: [0, -0.02, -0.22],
    size: [0.27, 0.54, 0.08],
    material: "cream",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.48, -0.18],
    rot: [29, 0, 0],
    size: [0.48, 0.42, 0.52],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.4],
    size: [0.3, 0.24, 0.3],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));

  for (const [side, x, lean] of [
    ["l", -0.24, -18],
    ["r", 0.24, 18],
  ] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.27, 0.06],
      rot: [-7, 0, lean],
      size: [0.16, 0.3, 0.09],
      material: "ear_inner",
      joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
    }));
  }

  // Three sparse beams per side make a forked buck silhouette without tiny
  // cylindrical tines or a dense ornamental rack.
  for (const [side, x, lean] of [
    ["l", -0.15, -15],
    ["r", 0.15, 15],
  ] as const) {
    part(`antler_root_${side}`, box({
      parent: "head",
      at: [x, 0.38, 0.08],
      rot: [-12, 0, lean],
      size: [0.075, 0.42, 0.075],
      material: "antler",
      joint: { pivot: [0, -0.19, 0], axis: [0, 0, 1] },
    }));
    part(`antler_outer_${side}`, box({
      parent: `antler_root_${side}`,
      at: [0, 0.34, 0],
      rot: [-8, 0, lean * 0.65],
      size: [0.065, 0.34, 0.065],
      material: "antler",
    }));
    part(`antler_tine_${side}`, box({
      parent: `antler_root_${side}`,
      at: [0, 0.16, -0.03],
      rot: [-28, 0, -lean * 1.7],
      size: [0.06, 0.25, 0.06],
      material: "antler",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.23, -0.5],
    ["fr", 0.23, -0.5],
    ["bl", -0.23, 0.51],
    ["br", 0.23, 0.51],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.59, z],
      size: [0.15, 0.77, 0.16],
      material: suffix.startsWith("b") ? "coat_dark" : "coat",
      joint: { pivot: [0, 0.385, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.44, -0.025],
      size: [0.18, 0.12, 0.21],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.17, 0.79],
    rot: [42, 0, 0],
    size: [0.2, 0.38, 0.18],
    material: "coat_dark",
    joint: { pivot: [0, 0.18, 0], axis: [0, 0, 1] },
  }));
  part("tail_flag", box({
    parent: "tail",
    at: [0, -0.23, -0.025],
    size: [0.16, 0.22, 0.14],
    material: "cream",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.08,
    cycleDistance: 0.98,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.12 }),
      followThrough("antler_root_l", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.1 }),
      followThrough("antler_root_r", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.1 }),
    ],
  });
});
